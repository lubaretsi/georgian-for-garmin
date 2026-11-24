//! A fake watch built from a watch report fixture (`tests/reports/*.json`), for `replay.rs` and
//! `ui_fixtures.rs`. `tests/reports/README.md` lists the watch behaviors it models, each with
//! the report that showed it. Besides the operations' own calls it records what they wrote and
//! every way they broke a rule (`violations`).
//!
//! Opt-in, for `ui_fixtures.rs`: [`Watch::fail_writes`] makes a write fail, as a watch that
//! stops responding does.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use georgian_for_garmin_core::Error;
use georgian_for_garmin_core::fonts::{Content, Earlier, Font, FontSet, Kind, sha256_hex};
use georgian_for_garmin_core::mtp::{CreateError, Mtp, MtpError, NewObject};
use georgian_for_garmin_core::names::{self, Role};
use georgian_for_garmin_core::ops;
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use serde_json::{Value, json};

pub const STORAGE: u32 = 0x0001_0001;
pub const GARMIN: u32 = 0x0100_0001;
pub const XML: u32 = 0x0100_0002;
pub const EXTDATA: u32 = 0x0100_0003;

/// Where a file's bytes come from, for the safety checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Garmin's original of a part.
    Stock,
    /// One of this program's builds: this build's, or an earlier one's.
    Ours,
    /// Anything else, which the operations must never write over.
    Other,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub info: ObjectInfo,
    pub data: Vec<u8>,
    /// Left out of the normal listing; only Garmin's `GetObjectIds` (0x9002) shows it.
    pub hidden: bool,
    pub origin: Origin,
}

pub struct State {
    pub model: String,
    pub model_part: String,
    pub software: String,
    /// `Garmin/ExtData` update files: (file name, part, version).
    pub declared: Vec<(String, String, Option<String>)>,
    pub objects: BTreeMap<u32, Object>,
    pub next: u32,
    pub get_object_ids: bool,
    pub get_object_ids_fails: bool,
    /// Whether the watch keeps `.bin` fonts out of the normal listing (imports them hidden).
    pub hides_bins: bool,
    /// Every way the operations broke a rule.
    pub violations: Vec<String>,
    /// Handles the operations wrote.
    pub written: Vec<u32>,
    /// Opt-in ([`Watch::fail_writes`]): the write that fails, and whether deletes fail too.
    pub failure: Option<WriteFailure>,
}

/// A watch that stops taking writes: after `after` more files are written, the next write's
/// data fails; with `deletes_fail`, every delete fails from then on, so undoing the call can't
/// remove what it wrote.
#[derive(Debug, Clone, Copy)]
pub struct WriteFailure {
    pub after: usize,
    pub deletes_fail: bool,
    fired: bool,
}

#[derive(Clone)]
pub struct Watch(pub Rc<RefCell<State>>);

fn version_xml(version: &Option<String>) -> String {
    match version.as_deref().and_then(|v| v.split_once('.')) {
        Some((major, minor)) => {
            format!("<Version><Major>{major}</Major><Minor>{minor}</Minor></Version>")
        }
        None => String::new(),
    }
}

impl State {
    pub fn device_xml(&self) -> Vec<u8> {
        let files: String = self
            .declared
            .iter()
            .map(|(file, part, version)| {
                format!(
                    "<UpdateFile><PartNumber>{part}</PartNumber>{}<Path>Garmin/ExtData</Path>\
                     <FileName>{file}</FileName></UpdateFile>",
                    version_xml(version)
                )
            })
            .collect();
        let software = self.software.replace('.', "");
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
             <Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\"><Model>\
             <PartNumber>{}</PartNumber><SoftwareVersion>{software}</SoftwareVersion>\
             <Description>{}</Description></Model><MassStorageMode>{files}</MassStorageMode>\
             </Device>",
            self.model_part,
            xml_escape(&self.model)
        )
        .into_bytes()
    }

    pub fn add(
        &mut self,
        name: &str,
        data: Vec<u8>,
        size: u32,
        hidden: bool,
        origin: Origin,
    ) -> u32 {
        let handle = self.next;
        self.next += 1;
        let info = ObjectInfo {
            storage_id: STORAGE,
            format: format::UNDEFINED,
            compressed_size: size,
            parent: EXTDATA,
            filename: name.into(),
            protection_status: u16::from(hidden),
            ..ObjectInfo::default()
        };
        self.objects.insert(
            handle,
            Object {
                info,
                data,
                hidden,
                origin,
            },
        );
        handle
    }

    pub fn refresh_xml(&mut self) {
        let xml = self.device_xml();
        let o = self.objects.get_mut(&XML).unwrap();
        o.info.compressed_size = xml.len() as u32;
        o.data = xml;
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;")
}

/// A file name without its extension, in the name form it has: `D894567.tmp` → `D894567`.
pub fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// The font `name` is for: a part number, or a part's file name in either name form ([`names`]):
/// `006-D9971-09`, `006-D9971-09.bin`, `D997109.bin`.
pub fn font_for<'a>(fonts: &'a FontSet, name: &str) -> Option<&'a Font> {
    let part = names::resolve(name).map_or_else(|| name.to_owned(), |f| f.part);
    fonts
        .fonts()
        .map(|(_, f)| f)
        .find(|f| f.part.eq_ignore_ascii_case(&part))
}

/// What the watch imports `font`'s file as: a bitmap part's `.fnt`, a TrueType part's `.bin`.
pub fn imported_role(font: &Font) -> Role {
    match font.kind {
        Kind::Bitmap => Role::Imported,
        Kind::Truetype => Role::Bin,
    }
}

/// Bytes that are none of the build's: `len` long, or short when the size is only listed.
fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len.min(4096))
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// A trailer-less earlier build of `font`, `len` bytes long, registered in `fonts` as an earlier
/// Georgian build so the operations recognize it, as the registry's `earlier_georgian` makes
/// them recognize 0.3.0-beta.1 and beta.2.
fn earlier_build(fonts: &mut FontSet, part: &str, len: usize) -> Vec<u8> {
    let data: Vec<u8> = (0..len)
        .map(|i| (i as u8).wrapping_mul(7).wrapping_add(0x5a))
        .collect();
    for pack in &mut fonts.packs {
        for font in &mut pack.fonts {
            if font.part == part {
                font.earlier.push(Earlier {
                    build: "replay".into(),
                    sha256: sha256_hex(&data),
                    bytes: data.len() as u64,
                });
            }
        }
    }
    data
}

/// A file's bytes and origin from what the report says it held (`file`) or its SHA-256.
fn contents(
    fonts: &mut FontSet,
    name: &str,
    bytes: u64,
    sha256: Option<&str>,
    file: Option<&str>,
) -> (Vec<u8>, u32, Origin) {
    let font = font_for(fonts, name).cloned();
    let by_hash = |font: &Font| {
        let hash = sha256?;
        if sha256_hex(&font.stock) == hash {
            Some("stock")
        } else if sha256_hex(&font.georgian) == hash {
            Some("georgian")
        } else if font.earlier.iter().any(|e| e.sha256 == hash) {
            Some("georgian_beta")
        } else {
            Some("other")
        }
    };
    let state = file.or_else(|| font.as_ref().and_then(by_hash));
    match (font, state) {
        (Some(font), Some("stock")) => {
            let data = font.stock.to_vec();
            let len = data.len() as u32;
            (data, len, Origin::Stock)
        }
        (Some(font), Some("georgian")) => {
            let data = font.georgian.to_vec();
            let len = data.len() as u32;
            (data, len, Origin::Ours)
        }
        (Some(font), Some("georgian_beta")) => {
            let data = earlier_build(fonts, &font.part, bytes as usize);
            (data, bytes as u32, Origin::Ours)
        }
        _ => (
            filler(bytes as usize, bytes as u8),
            bytes as u32,
            Origin::Other,
        ),
    }
}

impl Watch {
    /// The watch a report describes, and the build's fonts with the report's earlier builds
    /// registered.
    pub fn from_fixture(fixture: &Value) -> (Watch, FontSet) {
        Watch::from_fixture_with(fixture, FontSet::bundled())
    }

    /// As [`Watch::from_fixture`], with `fonts` in place of the build's.
    pub fn from_fixture_with(fixture: &Value, mut fonts: FontSet) -> (Watch, FontSet) {
        let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let declared = fixture["declared"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                (
                    text(&d["file"]),
                    text(&d["part"]),
                    d["version"].as_str().map(str::to_owned),
                )
            })
            .collect::<Vec<_>>();
        let hidden = &fixture["hidden"];
        let mut state = State {
            model: text(&fixture["model"]),
            model_part: text(&fixture["model_part"]),
            software: text(&fixture["software"]),
            declared,
            objects: BTreeMap::new(),
            next: 0x0100_0100,
            get_object_ids: matches!(hidden["status"].as_str(), Some("listed" | "error")),
            get_object_ids_fails: hidden["status"] == "error",
            hides_bins: true,
            violations: Vec::new(),
            written: Vec::new(),
            failure: None,
        };
        let folder = |name: &str, parent: u32| Object {
            info: ObjectInfo {
                storage_id: STORAGE,
                format: format::ASSOCIATION,
                parent,
                filename: name.into(),
                ..ObjectInfo::default()
            },
            data: Vec::new(),
            hidden: false,
            origin: Origin::Other,
        };
        state.objects.insert(GARMIN, folder("GARMIN", 0));
        state.objects.insert(
            XML,
            Object {
                info: ObjectInfo {
                    storage_id: STORAGE,
                    format: format::UNDEFINED,
                    parent: GARMIN,
                    filename: "GarminDevice.xml".into(),
                    ..ObjectInfo::default()
                },
                data: Vec::new(),
                hidden: false,
                origin: Origin::Other,
            },
        );
        if fixture["extdata"] != "not_found" {
            state.objects.insert(EXTDATA, folder("ExtData", GARMIN));
        }
        let listed = fixture["extdata"].as_array().cloned().unwrap_or_default();
        if listed.iter().any(|f| text(&f["name"]).ends_with(".bin")) {
            state.hides_bins = false;
        }
        for f in &listed {
            let name = text(&f["name"]);
            let (data, size, origin) = contents(
                &mut fonts,
                &name,
                f["bytes"].as_u64().unwrap(),
                f["sha256"].as_str(),
                f["file"].as_str(),
            );
            state.add(&name, data, size, false, origin);
        }
        for f in hidden["files"].as_array().into_iter().flatten() {
            let name = text(&f["name"]);
            let (data, size, origin) = contents(
                &mut fonts,
                &name,
                f["bytes"].as_u64().unwrap(),
                None,
                f["file"].as_str(),
            );
            state.add(&name, data, size, true, origin);
        }
        state.refresh_xml();
        (Watch(Rc::new(RefCell::new(state))), fonts)
    }

    pub fn call(&self, name: &str, fonts: &FontSet) -> Result<Value, Error> {
        let watch = self.clone();
        ops::call(name, json!({}), fonts, &mut || Ok(Box::new(watch.clone())))
    }

    /// Opt-in: after `after` more files are written, the next write fails; with
    /// `deletes_fail`, so do deletes from then on ([`WriteFailure`]).
    pub fn fail_writes(&self, after: usize, deletes_fail: bool) {
        self.0.borrow_mut().failure = Some(WriteFailure {
            after,
            deletes_fail,
            fired: false,
        });
    }

    /// The watch restarts, as watches were seen to:
    /// - a staged `.tmp` is imported: it leaves the normal listing and becomes a hidden
    ///   read-only `.fnt` under the same name form (`006-D9035-03.fnt`, `D894567.fnt`),
    ///   replacing the part's earlier import;
    /// - on a watch that keeps `.bin` fonts out of the normal listing, a written `<part>.bin`
    ///   becomes the hidden read-only one, replacing the old import;
    /// - a `.bin`'s declared version is read from its trailer: Garmin's version for Garmin's
    ///   originals and this program's builds since 0.3.0, which keep it; a trailer-less earlier
    ///   build keeps the version the report showed (0.00 or 0.01).
    ///
    /// An import replaced that is neither Garmin's original nor ours is a violation: the file
    /// written before the restart overwrote it.
    pub fn restart(&self, fonts: &FontSet) {
        let mut s = self.0.borrow_mut();
        let handles: Vec<u32> = s.objects.keys().copied().collect();
        for handle in handles {
            let o = s.objects[&handle].clone();
            if o.hidden || o.info.parent != EXTDATA {
                continue;
            }
            let lower = o.info.filename.to_ascii_lowercase();
            let file = names::resolve(&o.info.filename);
            // The part's earlier imports with `role`, in either name form.
            let earlier = |s: &State, role: Role, name: &str| -> Vec<u32> {
                s.objects
                    .iter()
                    .filter(|&(&h, x)| {
                        h != handle
                            && x.hidden
                            && match &file {
                                Some(f) => names::is(&x.info.filename, &f.part, role),
                                None => x.info.filename.eq_ignore_ascii_case(name),
                            }
                    })
                    .map(|(&h, _)| h)
                    .collect()
            };
            let replace = |s: &mut State, old: Vec<u32>| {
                for h in old {
                    let x = s.objects.remove(&h).unwrap();
                    if x.origin == Origin::Other {
                        s.violations.push(format!(
                            "{} was imported over {}, which is neither Garmin's nor ours",
                            o.info.filename, x.info.filename
                        ));
                    }
                }
            };
            if lower.ends_with(".tmp") {
                s.objects.remove(&handle);
                let name = format!("{}.fnt", stem(&o.info.filename));
                let old = earlier(&s, Role::Imported, &name);
                replace(&mut s, old);
                let size = o.data.len() as u32;
                s.add(&name, o.data, size, true, o.origin);
            } else if lower.ends_with(".bin") && s.hides_bins {
                let old = earlier(&s, Role::Bin, &lower);
                replace(&mut s, old);
                let o = s.objects.get_mut(&handle).unwrap();
                o.hidden = true;
                o.info.protection_status = 1;
            }
        }
        // Versions from the trailers.
        let current: Vec<(String, Vec<u8>)> = s
            .objects
            .values()
            .filter(|o| o.info.parent == EXTDATA)
            .filter_map(|o| {
                let file = names::resolve(&o.info.filename)?;
                (file.role == Role::Bin).then(|| (file.part, o.data.clone()))
            })
            .collect();
        for (part, data) in current {
            let Some(font) = fonts.fonts().map(|(_, f)| f).find(|f| f.part == part) else {
                continue;
            };
            if matches!(font.identify(&data), Content::Stock | Content::Georgian)
                && let Some(d) = s.declared.iter_mut().find(|d| d.1 == part)
            {
                d.2 = Some(font.version.clone());
            }
        }
        s.refresh_xml();
    }
}

impl Mtp for Watch {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        let mut operations = vec![
            op::GET_DEVICE_INFO,
            op::OPEN_SESSION,
            op::CLOSE_SESSION,
            op::GET_STORAGE_IDS,
            op::GET_OBJECT_HANDLES,
            op::GET_OBJECT_INFO,
            op::GET_OBJECT,
            op::DELETE_OBJECT,
            op::SEND_OBJECT_INFO,
            op::SEND_OBJECT,
        ];
        if self.0.borrow().get_object_ids {
            operations.push(op::GARMIN_GET_OBJECT_IDS);
        }
        Ok(DeviceInfo {
            operations_supported: operations,
            ..DeviceInfo::default()
        })
    }

    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError> {
        Ok(vec![STORAGE])
    }

    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let s = self.0.borrow();
        Ok(s.objects
            .iter()
            .filter(|(_, o)| {
                !o.hidden
                    && o.info.storage_id == storage
                    && match parent {
                        ALL_OBJECTS => true,
                        ROOT => o.info.parent == 0,
                        p => o.info.parent == p,
                    }
            })
            .map(|(&h, _)| h)
            .collect())
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError> {
        self.0
            .borrow()
            .objects
            .get(&handle)
            .map(|o| o.info.clone())
            .ok_or(MtpError::Response {
                operation: op::GET_OBJECT_INFO,
                code: 0x2009,
            })
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        self.0
            .borrow()
            .objects
            .get(&handle)
            .map(|o| o.data.clone())
            .ok_or(MtpError::Response {
                operation: op::GET_OBJECT,
                code: 0x2009,
            })
    }

    fn create_object(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
        data: &[u8],
    ) -> Result<NewObject, CreateError> {
        // What the operations write is Garmin's original of a part or one of this build's.
        let stock = FontSet::bundled().fonts().any(|(_, f)| *data == *f.stock);
        let origin = if stock { Origin::Stock } else { Origin::Ours };
        let mut s = self.0.borrow_mut();
        let handle = s.add(
            &info.filename,
            data.to_vec(),
            data.len() as u32,
            false,
            origin,
        );
        let o = s.objects.get_mut(&handle).unwrap();
        o.info.storage_id = storage;
        o.info.parent = parent;
        s.written.push(handle);
        if let Some(failure) = s.failure.as_mut().filter(|f| !f.fired) {
            if failure.after == 0 {
                // The object exists, but its data didn't arrive: a general error on SendObject.
                failure.fired = true;
                return Err(CreateError::Data {
                    handle,
                    error: MtpError::Response {
                        operation: op::SEND_OBJECT,
                        code: 0x2002,
                    },
                });
            }
            failure.after -= 1;
        }
        Ok(NewObject {
            storage,
            parent,
            handle,
        })
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        let mut s = self.0.borrow_mut();
        if s.failure.is_some_and(|f| f.fired && f.deletes_fail) {
            return Err(MtpError::Response {
                operation: op::DELETE_OBJECT,
                code: 0x2002,
            });
        }
        let Some(o) = s.objects.remove(&handle) else {
            return Err(MtpError::Response {
                operation: op::DELETE_OBJECT,
                code: 0x2009,
            });
        };
        if o.hidden {
            s.violations
                .push(format!("deleted the hidden object {}", o.info.filename));
        }
        if o.origin == Origin::Other {
            s.violations.push(format!(
                "deleted {}, which is neither Garmin's nor ours",
                o.info.filename
            ));
        }
        Ok(())
    }

    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let s = self.0.borrow();
        if s.get_object_ids_fails {
            return Err(MtpError::Response {
                operation: op::GARMIN_GET_OBJECT_IDS,
                code: 0x2002,
            });
        }
        Ok(s.objects
            .iter()
            .filter(|(_, o)| o.info.storage_id == storage && o.info.parent == parent)
            .map(|(&h, _)| h)
            .collect())
    }
}

pub fn fixture(name: &str) -> Value {
    fixtures()
        .into_iter()
        .find(|(p, _)| p.ends_with(name))
        .unwrap_or_else(|| panic!("fixture {name}"))
        .1
}

pub fn fixtures() -> Vec<(PathBuf, Value)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reports");
    let mut out: Vec<(PathBuf, Value)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| {
            let value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
            (p, value)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// `fixture` with the hidden imports of `parts` read as Garmin's originals. The 0.3.x reports
/// list a hidden import's size but not its hash, so a test that installs on such a watch has to
/// say what the import holds; this says Garmin's original, and checks first that the size is
/// the pinned original's, as it is for the Lily 2 Active's `-67/-68` and #33's `-07`.
pub fn imports_as_garmins(mut fixture: Value, fonts: &FontSet, parts: &[&str]) -> Value {
    for file in fixture["hidden"]["files"]
        .as_array_mut()
        .into_iter()
        .flatten()
    {
        let Some(font) = names::resolve(file["name"].as_str().unwrap())
            .filter(|f| parts.contains(&f.part.as_str()))
            .and_then(|f| font_for(fonts, &f.part))
        else {
            continue;
        };
        assert_eq!(
            file["bytes"].as_u64(),
            Some(font.stock.len() as u64),
            "{}: the import isn't the pinned original's size",
            file["name"]
        );
        file["file"] = json!("stock");
    }
    fixture
}
