//! Replays every watch report posted in the repository's issues against a fake watch built from
//! it: inspect, install, a restart, install again, a restart, restore.
//!
//! The fixtures (`tests/reports/*.json`) come from the reports' text through
//! `scripts/report_fixture.py`; `tests/reports/README.md` lists them and the watch behaviors the
//! fake models, each with the report that showed it. Whatever the watch held, the real
//! operations must never write over a file that is neither Garmin's original nor one of this
//! program's builds, never delete a hidden (imported, read-only) object, put Garmin's original
//! back wherever restore writes, and never refuse a state this program's own builds produced
//! with `unsupported_device`; install may ask to restore first where it can't read a file the
//! watch declares below 1.00, as it did after this program's trailer-less builds.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use georgian_for_garmin_core::fonts::{Content, Earlier, Font, FontSet, Kind, sha256_hex};
use georgian_for_garmin_core::mtp::{CreateError, Mtp, MtpError, NewObject};
use georgian_for_garmin_core::ops;
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use georgian_for_garmin_core::{Error, ErrorCode};
use serde_json::{Value, json};

const STORAGE: u32 = 0x0001_0001;
const GARMIN: u32 = 0x0100_0001;
const XML: u32 = 0x0100_0002;
const EXTDATA: u32 = 0x0100_0003;

/// Where a file's bytes come from, for the safety checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// Garmin's original of a part.
    Stock,
    /// One of this program's builds: this build's, or an earlier one's.
    Ours,
    /// Anything else, which the operations must never write over.
    Other,
}

#[derive(Debug, Clone)]
struct Object {
    info: ObjectInfo,
    data: Vec<u8>,
    /// Left out of the normal listing; only Garmin's `GetObjectIds` (0x9002) shows it.
    hidden: bool,
    origin: Origin,
}

struct State {
    model: String,
    model_part: String,
    software: String,
    /// `Garmin/ExtData` update files: (file name, part, version).
    declared: Vec<(String, String, Option<String>)>,
    objects: BTreeMap<u32, Object>,
    next: u32,
    get_object_ids: bool,
    get_object_ids_fails: bool,
    /// Whether the watch keeps `.bin` fonts out of the normal listing (imports them hidden).
    hides_bins: bool,
    /// Every way the operations broke a rule.
    violations: Vec<String>,
    /// Handles the operations wrote.
    written: Vec<u32>,
}

#[derive(Clone)]
struct Watch(Rc<RefCell<State>>);

fn version_xml(version: &Option<String>) -> String {
    match version.as_deref().and_then(|v| v.split_once('.')) {
        Some((major, minor)) => {
            format!("<Version><Major>{major}</Major><Minor>{minor}</Minor></Version>")
        }
        None => String::new(),
    }
}

impl State {
    fn device_xml(&self) -> Vec<u8> {
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

    fn add(&mut self, name: &str, data: Vec<u8>, size: u32, hidden: bool, origin: Origin) -> u32 {
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

    fn refresh_xml(&mut self) {
        let xml = self.device_xml();
        let o = self.objects.get_mut(&XML).unwrap();
        o.info.compressed_size = xml.len() as u32;
        o.data = xml;
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;")
}

/// The part a file is named for: `006-D9971-09.bin` → `006-D9971-09`.
fn part_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

fn font_for<'a>(fonts: &'a FontSet, name: &str) -> Option<&'a Font> {
    fonts
        .fonts()
        .map(|(_, f)| f)
        .find(|f| f.part.eq_ignore_ascii_case(part_of(name)))
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
    fn from_fixture(fixture: &Value) -> (Watch, FontSet) {
        let mut fonts = FontSet::bundled();
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

    fn call(&self, name: &str, fonts: &FontSet) -> Result<Value, Error> {
        let watch = self.clone();
        ops::call(name, json!({}), fonts, &mut || Ok(Box::new(watch.clone())))
    }

    /// The watch restarts, as watches were seen to:
    /// - a staged `<part>.tmp` is imported: it leaves the normal listing and becomes a hidden
    ///   read-only `<part>.fnt`;
    /// - on a watch that keeps `.bin` fonts out of the normal listing, a written `<part>.bin`
    ///   becomes the hidden read-only one, replacing the old import;
    /// - a `.bin`'s declared version is read from its trailer: Garmin's version for Garmin's
    ///   originals and this program's builds since 0.3.0, which keep it; a trailer-less earlier
    ///   build keeps the version the report showed (0.00 or 0.01).
    fn restart(&self, fonts: &FontSet) {
        let mut s = self.0.borrow_mut();
        let handles: Vec<u32> = s.objects.keys().copied().collect();
        for handle in handles {
            let o = s.objects[&handle].clone();
            if o.hidden || o.info.parent != EXTDATA {
                continue;
            }
            let lower = o.info.filename.to_ascii_lowercase();
            if lower.ends_with(".tmp") {
                s.objects.remove(&handle);
                let name = format!("{}.fnt", part_of(&o.info.filename));
                let old: Vec<u32> = s
                    .objects
                    .iter()
                    .filter(|(_, x)| x.hidden && x.info.filename.eq_ignore_ascii_case(&name))
                    .map(|(&h, _)| h)
                    .collect();
                for h in old {
                    s.objects.remove(&h);
                }
                let size = o.data.len() as u32;
                s.add(&name, o.data, size, true, o.origin);
            } else if lower.ends_with(".bin") && s.hides_bins {
                let old: Vec<u32> = s
                    .objects
                    .iter()
                    .filter(|&(&h, x)| {
                        h != handle && x.hidden && x.info.filename.eq_ignore_ascii_case(&lower)
                    })
                    .map(|(&h, _)| h)
                    .collect();
                for h in old {
                    s.objects.remove(&h);
                }
                let o = s.objects.get_mut(&handle).unwrap();
                o.hidden = true;
                o.info.protection_status = 1;
            }
        }
        // Versions from the trailers.
        let current: Vec<(String, Vec<u8>)> = s
            .objects
            .values()
            .filter(|o| {
                o.info.parent == EXTDATA && o.info.filename.to_ascii_lowercase().ends_with(".bin")
            })
            .map(|o| (part_of(&o.info.filename).to_owned(), o.data.clone()))
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
        Ok(NewObject {
            storage,
            parent,
            handle,
        })
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        let mut s = self.0.borrow_mut();
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

/// Where a fixture comes from: a report in the repository's issues, or the maintainer's own
/// watch.
fn known_source(source: &str) -> bool {
    source.starts_with("https://github.com/lubaretsi/georgian-for-garmin/issues/")
        || source.starts_with("maintainer's ")
}

fn fixture(name: &str) -> Value {
    fixtures()
        .into_iter()
        .find(|(p, _)| p.ends_with(name))
        .unwrap_or_else(|| panic!("fixture {name}"))
        .1
}

fn fixtures() -> Vec<(PathBuf, Value)> {
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

/// The parts an operation targets, from inspect's `fonts` (`target` or `restore`).
fn targets<'a>(inspect: &'a Value, key: &str) -> Vec<&'a str> {
    inspect["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f[key] == true)
        .map(|f| f["part"].as_str().unwrap())
        .collect()
}

/// Whether a file under one of `parts`' names is neither Garmin's original nor ours: in the
/// normal listing, or the watch's import (a bitmap part's hidden `<part>.fnt`, a TrueType part's
/// hidden `.bin`).
fn other_under(watch: &Watch, fonts: &FontSet, parts: &[&str]) -> bool {
    let s = watch.0.borrow();
    parts.iter().any(|part| {
        let Some(font) = font_for(fonts, part) else {
            return false;
        };
        let name = font.file_name();
        let imported = match font.kind {
            Kind::Bitmap => format!("{}.fnt", font.part),
            Kind::Truetype => name.clone(),
        };
        s.objects.values().any(|o| {
            o.info.parent == EXTDATA
                && o.origin == Origin::Other
                && if o.hidden {
                    o.info.filename.eq_ignore_ascii_case(&imported)
                } else {
                    o.info.filename.eq_ignore_ascii_case(&name)
                }
        })
    })
}

/// Whether one of `parts` is declared below 1.00 (this program's trailer-less builds).
fn declared_below_one(watch: &Watch, parts: &[&str]) -> bool {
    watch.0.borrow().declared.iter().any(|(_, part, version)| {
        parts.contains(&part.as_str())
            && version
                .as_deref()
                .is_some_and(|v| v.split('.').next() == Some("0"))
    })
}

/// Checks an operation's outcome against the rules; returns whether it succeeded.
fn check(
    source: &str,
    step: &str,
    result: &Result<Value, Error>,
    inspect: &Value,
    watch: &Watch,
    fonts: &FontSet,
) -> bool {
    let key = if step.starts_with("restore") {
        "restore"
    } else {
        "target"
    };
    let parts = targets(inspect, key);
    let err = match result {
        Ok(_) => return true,
        Err(e) => e,
    };
    let allowed = match err.code {
        // Nothing to write on this watch.
        ErrorCode::UnsupportedDevice => parts.is_empty(),
        ErrorCode::Conflict => other_under(watch, fonts, &parts),
        ErrorCode::RestoreFirst => step.starts_with("install") && declared_below_one(watch, &parts),
        _ => false,
    };
    assert!(
        allowed,
        "{source}: {step} failed with {}: {}",
        err.code.as_str(),
        err.message
    );
    false
}

#[test]
fn every_posted_report_replays_safely() {
    let fixtures = fixtures();
    assert!(fixtures.len() >= 20, "{} fixtures", fixtures.len());
    let mut installed = 0;
    let mut restored = 0;
    for (path, fixture) in &fixtures {
        let source = fixture["source"].as_str().unwrap();
        assert!(known_source(source), "{source}");
        let (watch, fonts) = Watch::from_fixture(fixture);
        if fixture["connection"] == "usb_drive" {
            let root = std::env::temp_dir().join(format!(
                "gfg-replay-{}-{}",
                std::process::id(),
                path.file_stem().unwrap().to_string_lossy()
            ));
            std::fs::create_dir_all(root.join("GARMIN")).unwrap();
            std::fs::write(
                root.join("GARMIN/GarminDevice.xml"),
                watch.0.borrow().device_xml(),
            )
            .unwrap();
            let out = ops::call("inspect_drive", json!({"path": root}), &fonts, &mut || {
                panic!("a drive is never opened over MTP")
            });
            std::fs::remove_dir_all(&root).unwrap();
            let out = out.unwrap_or_else(|e| panic!("{source}: inspect_drive: {e}"));
            assert!(out["support"]["status"].is_string(), "{source}");
            continue;
        }

        let before = watch
            .call("inspect", &fonts)
            .unwrap_or_else(|e| panic!("{source}: inspect: {e}"));
        let install = watch.call("install", &fonts);
        let first = check(source, "install", &install, &before, &watch, &fonts);
        installed += usize::from(first);
        watch.restart(&fonts);

        let again = watch
            .call("inspect", &fonts)
            .unwrap_or_else(|e| panic!("{source}: inspect after install: {e}"));
        let reinstall = watch.call("install", &fonts);
        // Where the watch shows its imports (0x9002), the font install wrote and the restart
        // imported reads as installed, as on the maintainer's Venu 2.
        if first && watch.0.borrow().get_object_ids && !watch.0.borrow().get_object_ids_fails {
            assert_eq!(
                again["state"], "installed",
                "{source}: after install and a restart"
            );
        }
        if first {
            let out = reinstall
                .as_ref()
                .unwrap_or_else(|e| panic!("{source}: install again failed: {e}"));
            assert!(
                matches!(
                    out["status"].as_str(),
                    Some("already_staged" | "staged_verified")
                ),
                "{source}: {out}"
            );
        } else {
            check(source, "install again", &reinstall, &again, &watch, &fonts);
        }
        watch.restart(&fonts);

        let ready = watch
            .call("inspect", &fonts)
            .unwrap_or_else(|e| panic!("{source}: inspect before restore: {e}"));
        let written_before = watch.0.borrow().written.len();
        let restore = watch.call("restore_stock", &fonts);
        if check(source, "restore", &restore, &ready, &watch, &fonts) {
            restored += 1;
            // Garmin's original wherever restore wrote.
            let out = restore.as_ref().unwrap();
            for pack in out["packs"].as_array().unwrap() {
                for part in pack["parts"].as_array().unwrap() {
                    let font = font_for(&fonts, part["part"].as_str().unwrap()).unwrap();
                    assert_eq!(
                        part["sha256"].as_str().unwrap(),
                        sha256_hex(&font.stock),
                        "{source}: restore left {} without Garmin's original",
                        font.part
                    );
                }
            }
            // And every file restore wrote holds it.
            let s = watch.0.borrow();
            for handle in &s.written[written_before..] {
                let o = &s.objects[handle];
                let font = font_for(&fonts, &o.info.filename).unwrap();
                assert_eq!(o.data, *font.stock, "{source}: {}", o.info.filename);
            }
        }
        let outcome = |r: &Result<Value, Error>| match r {
            Ok(v) => v["status"].as_str().unwrap_or("ok").to_owned(),
            Err(e) => e.code.as_str().to_owned(),
        };
        eprintln!(
            "{source}: install {}, again {}, restore {}",
            outcome(&install),
            outcome(&reinstall),
            outcome(&restore)
        );
        let violations = watch.0.borrow().violations.clone();
        assert!(violations.is_empty(), "{source}: {violations:?}");
    }
    // The suite exercises both paths: watches it installs on and ones it restores.
    assert!(
        installed > 0 && restored > 0,
        "{installed} installed, {restored} restored"
    );
}

/// The fixtures hold only allowlisted fields: no unit ID or serial number. A fixture composed by
/// hand (a report that isn't in a report format) names its sources in `composed_from`.
#[test]
fn fixtures_hold_no_identifiers() {
    fn scan(value: &Value, key: &str, source: &str) {
        match value {
            Value::Object(map) => map.iter().for_each(|(k, v)| scan(v, k, source)),
            Value::Array(items) => items.iter().for_each(|v| scan(v, key, source)),
            Value::String(s) if !matches!(key, "sha256" | "source" | "composed_from") => {
                let digits = s
                    .split(|c: char| !c.is_ascii_digit())
                    .map(str::len)
                    .max()
                    .unwrap_or(0);
                assert!(digits < 9, "{source}: {key} {s:?} looks like an identifier");
            }
            _ => {}
        }
    }
    let keys = [
        "source",
        "app_version",
        "connection",
        "model",
        "model_part",
        "software",
        "declared",
        "extdata",
        "hidden",
    ];
    for (_, fixture) in fixtures() {
        let source = fixture["source"].as_str().unwrap().to_owned();
        let mut found: Vec<&str> = fixture
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .filter(|&k| k != "composed_from")
            .collect();
        found.sort_unstable();
        let mut expected = keys.to_vec();
        expected.sort_unstable();
        assert_eq!(found, expected, "{source}");
        for from in fixture["composed_from"].as_array().into_iter().flatten() {
            assert!(known_source(from.as_str().unwrap()), "{source}: {from}");
        }
        scan(&fixture, "", &source);
    }
}

/// #5's Forerunner 165 after 0.3.0-beta.2's trailer-less install: the four TrueType parts
/// declared 0.01, no file the watch lets be read. Install refuses with `restore_first` and
/// writes nothing; restore writes Garmin's originals (they may go back over a file without
/// Garmin's version); after the restart the watch declares 2.00 again and install succeeds.
#[test]
fn the_0_01_state_restores_and_then_installs() {
    let (_, fixture) = fixtures()
        .into_iter()
        .find(|(p, _)| p.ends_with("issue-5-comment-7078836039.json"))
        .expect("the #5 restore failure fixture");
    let (watch, fonts) = Watch::from_fixture(&fixture);
    let parts = [
        "006-D9971-09",
        "006-D9971-08",
        "006-D1053-98",
        "006-D1053-99",
    ];

    let err = watch.call("install", &fonts).unwrap_err();
    assert_eq!(err.code, ErrorCode::RestoreFirst, "{}", err.message);
    assert!(
        watch.0.borrow().written.is_empty(),
        "install wrote something"
    );

    let restored = watch.call("restore_stock", &fonts).unwrap();
    let staged: Vec<&str> = restored["packs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["parts"].as_array().unwrap())
        .map(|p| {
            assert_eq!(p["status"], "staged_verified");
            p["part"].as_str().unwrap()
        })
        .collect();
    assert_eq!(staged, parts);
    for handle in watch.0.borrow().written.clone() {
        let s = watch.0.borrow();
        let o = &s.objects[&handle];
        assert_eq!(o.data, *font_for(&fonts, &o.info.filename).unwrap().stock);
    }

    watch.restart(&fonts);
    let declared = watch.0.borrow().declared.clone();
    for part in parts {
        let version = declared.iter().find(|d| d.1 == part).unwrap().2.clone();
        assert_eq!(version.as_deref(), Some("2.00"), "{part}");
    }
    let installed = watch.call("install", &fonts).unwrap();
    assert_eq!(installed["status"], "staged_verified");
    assert!(watch.0.borrow().violations.is_empty());
}

/// The maintainer's Venu 2 with the 0.3.1 CLI: after install and a restart its Georgian `.tmp`
/// files are the hidden read-only `.fnt` imports, and inspect says Georgian is installed (0.3.1
/// said `not_installed` with every part `file none`). After restore-stock and a restart the
/// imports are Garmin's, and Georgian isn't installed. Install then stages Georgian again
/// beside the imports, without touching them.
#[test]
fn the_venu_2_reads_its_imported_fonts() {
    let parts = [
        "006-D9035-01",
        "006-D9035-02",
        "006-D9035-03",
        "006-D9035-04",
    ];
    let targets = |out: &Value| -> Vec<(String, String, bool)> {
        out["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["target"] == true)
            .map(|f| {
                (
                    f["part"].as_str().unwrap().to_owned(),
                    f["file"].as_str().unwrap().to_owned(),
                    f["imported"] == true,
                )
            })
            .collect()
    };
    for (name, file, state) in [
        (
            "maintainer-venu2-georgian-imported.json",
            "georgian",
            "installed",
        ),
        (
            "maintainer-venu2-stock-imported.json",
            "stock",
            "not_installed",
        ),
    ] {
        let (watch, fonts) = Watch::from_fixture(&fixture(name));
        let out = watch.call("inspect", &fonts).unwrap();
        assert_eq!(out["state"], state, "{name}");
        assert_eq!(out["support"]["status"], "tested", "{name}");
        assert_eq!(
            targets(&out),
            parts.map(|p| (p.to_owned(), file.to_owned(), true)),
            "{name}"
        );
        let text = out["report"]["text"].as_str().unwrap();
        assert!(
            text.contains(&format!("\nstate {state}\n")),
            "{name}: {text}"
        );
        for part in parts {
            assert!(
                text.contains(&format!("\n{part} {file} yes tested -\n")),
                "{name}: {text}"
            );
        }

        let installed = watch.call("install", &fonts).unwrap();
        assert_eq!(installed["status"], "staged_verified", "{name}");
        assert_eq!(watch.call("inspect", &fonts).unwrap()["state"], "waiting");
        watch.restart(&fonts);
        assert_eq!(watch.call("inspect", &fonts).unwrap()["state"], "installed");
        assert!(watch.0.borrow().violations.is_empty(), "{name}");
    }
}

/// The Venu 2 with one imported `.fnt` this build doesn't recognise (a size none of its builds
/// has): `other`, a report wanted, and install and restore write nothing, since the next restart
/// would replace that font with whatever they staged.
#[test]
fn an_unrecognised_imported_font_is_never_written_over() {
    let mut fixture = fixture("maintainer-venu2-georgian-imported.json");
    for file in fixture["hidden"]["files"].as_array_mut().unwrap() {
        if file["name"] == "006-D9035-03.fnt" {
            file["bytes"] = json!(35000);
            file["file"] = Value::Null;
        }
    }
    let (watch, fonts) = Watch::from_fixture(&fixture);
    let out = watch.call("inspect", &fonts).unwrap();
    assert_eq!(out["state"], "other");
    assert_eq!(out["support"]["report_wanted"], true);
    for operation in ["install", "restore_stock"] {
        let err = watch.call(operation, &fonts).unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::Conflict,
            "{operation}: {}",
            err.message
        );
    }
    assert!(watch.0.borrow().written.is_empty());
    assert!(watch.0.borrow().violations.is_empty());
}
