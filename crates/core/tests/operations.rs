//! The real operations driven against an in-memory MTP device modeled on the Venu 2.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use georgian_for_garmin_core::fonts::{Font, FontSet, Kind, Mark, Pack, Tested};
use georgian_for_garmin_core::mtp::{CreateError, Mtp, MtpError, NewObject, Phase, TransportError};
use georgian_for_garmin_core::ops;
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use georgian_for_garmin_core::{Error, ErrorCode};
use serde_json::{Value, json};

const STORAGE: u32 = 0x0002_0001;
const OTHER_STORAGE: u32 = 0x0001_0001;
const GARMIN: u32 = 0x0100_0002;
const XML: u32 = 0x0100_0003;
const EXTDATA: u32 = 0x0100_0009;
/// The Armenian pack, first in the test registry: (part, size).
const ARMENIAN: [(&str, u16); 4] = [
    ("006-D9035-01", 21),
    ("006-D9035-02", 24),
    ("006-D9035-03", 27),
    ("006-D9035-04", 31),
];
/// A second pack, after the Armenian one.
const THAI: [(&str, u16); 2] = [("006-D8947-01", 21), ("006-D8947-02", 27)];
const ALL: [&str; 4] = [ARMENIAN[0].0, ARMENIAN[1].0, ARMENIAN[2].0, ARMENIAN[3].0];
const PART: &str = "006-D9035-03";
const STAGED: &str = "006-D9035-03.tmp";

/// The TrueType pack, after the Armenian and Thai ones in `fonts_tt`: Regular, then Bold.
const HEBREW: [&str; 2] = ["006-D0952-08", "006-D0952-07"];
const BIN: &str = "006-D0952-08.bin";

/// A `Garmin/ExtData` update file entry at version 2.`minor`.
fn update_file(part: &str, minor: &str, file_name: &str) -> String {
    format!(
        "<UpdateFile><PartNumber>{part}</PartNumber><Version><Major>2</Major><Minor>{minor}</Minor></Version><Path>Garmin/ExtData</Path><FileName>{file_name}</FileName></UpdateFile>"
    )
}

fn device_xml_with(model_part: &str, software: &str, files: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">\
         <Model><PartNumber>{model_part}</PartNumber><SoftwareVersion>{software}</SoftwareVersion><Description>Venu 2</Description></Model>\
         <MassStorageMode><UpdateFile><PartNumber>006-D8422-03</PartNumber><Path>Garmin/Text</Path><FileName>french.ln4</FileName></UpdateFile>{files}</MassStorageMode></Device>"
    )
    .into_bytes()
}

/// Every part at version 2.10, as the watches list them: TrueType parts as `.bin`, the rest
/// as `.tmp`.
fn device_xml_for(model_part: &str, software: &str, parts: &[&str]) -> Vec<u8> {
    let files: String = parts
        .iter()
        .map(|p| {
            let ext = if HEBREW.contains(p) { "bin" } else { "tmp" };
            update_file(p, "10", &format!("{p}.{ext}"))
        })
        .collect();
    device_xml_with(model_part, software, &files)
}

fn device_xml(parts: &[&str]) -> Vec<u8> {
    device_xml_for("006-B3703-00", "1905", parts)
}

#[derive(Default)]
struct State {
    objects: BTreeMap<u32, (ObjectInfo, Vec<u8>)>,
    /// Objects the device answers `GetObjectInfo` for but omits from every listing.
    hidden: BTreeSet<u32>,
    operations: Vec<u16>,
    next_handle: u32,
    awaiting_data: Option<u32>,
    /// The nth `SendObject` (1-based) stores half the data, then times out.
    fail_send_object_at: Option<usize>,
    sends: usize,
    /// `SendObjectInfo` times out; `Some(true)` if the watch created the object anyway.
    send_object_info_timeout: Option<bool>,
    /// `DeleteObject` times out.
    fail_delete: bool,
    /// The device stores different bytes than it received.
    corrupt_storage: bool,
    /// The nth `SendObject` (1-based) stores different bytes than it received.
    corrupt_send_at: Option<usize>,
    /// The watch puts new objects in this folder instead of the requested one.
    misplace_into: Option<u32>,
    writes: Vec<String>,
    connects: usize,
    /// Handles passed to `GetObject`, in order.
    reads: Vec<u32>,
    /// Handles Garmin's `GetObjectIds` reports beyond `parent`'s hidden children.
    object_ids_extra: Vec<u32>,
    /// Garmin's `GetObjectIds` answers with an error response.
    object_ids_fail: bool,
}

impl State {
    fn add(&mut self, handle: u32, storage: u32, parent: u32, name: &str, fmt: u16, data: Vec<u8>) {
        let info = ObjectInfo {
            storage_id: storage,
            format: fmt,
            compressed_size: data.len() as u32,
            parent,
            filename: name.into(),
            ..ObjectInfo::default()
        };
        self.objects.insert(handle, (info, data));
    }

    fn folder(&mut self, handle: u32, parent: u32, name: &str) {
        self.add(
            handle,
            STORAGE,
            parent,
            name,
            format::ASSOCIATION,
            Vec::new(),
        );
    }

    fn file(&mut self, handle: u32, parent: u32, name: &str, data: &[u8]) {
        self.add(
            handle,
            STORAGE,
            parent,
            name,
            format::UNDEFINED,
            data.to_vec(),
        );
    }

    fn named(&self, parent: u32, name: &str) -> Vec<(u32, Vec<u8>)> {
        self.objects
            .iter()
            .filter(|(_, (i, _))| i.parent == parent && i.filename == name)
            .map(|(&h, (_, d))| (h, d.clone()))
            .collect()
    }
}

/// The Venu 2 layout: `ExtData` exists but is hidden from listings.
fn venu2(declared: &[&str]) -> State {
    venu2_with(device_xml(declared))
}

fn venu2_with(xml: Vec<u8>) -> State {
    let mut s = State {
        operations: vec![
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
        ],
        next_handle: 0x0100_0100,
        ..State::default()
    };
    s.folder(0x0100_0001, 0, "Music");
    s.folder(GARMIN, 0, "GARMIN");
    s.file(XML, GARMIN, "GarminDevice.xml", &xml);
    s.folder(0x0100_0004, GARMIN, "TEXT");
    s.folder(EXTDATA, GARMIN, "ExtData");
    s.hidden.insert(EXTDATA);
    s
}

/// First handle past the probe window: `(GARMIN & 0xFF000000) + 33`.
const PAST_PROBE: u32 = 0x0100_0021;

/// The tactix 7 layout: `ExtData` hidden one handle past the probe window, reported only by
/// Garmin's `GetObjectIds`, which the watch lists among its operations.
fn tactix7(declared: &[&str]) -> State {
    let mut s = venu2(declared);
    s.objects.remove(&EXTDATA);
    s.hidden.clear();
    s.folder(PAST_PROBE, GARMIN, "ExtData");
    s.hidden.insert(PAST_PROBE);
    s.operations.push(op::GARMIN_GET_OBJECT_IDS);
    s
}

#[derive(Clone)]
struct Fake(Rc<RefCell<State>>);

impl Fake {
    fn new(state: State) -> Self {
        Fake(Rc::new(RefCell::new(state)))
    }

    fn call(&self, name: &str, input: Value, fonts: &FontSet) -> Result<Value, Error> {
        let mut connect = || -> Result<Box<dyn Mtp>, Error> {
            self.0.borrow_mut().connects += 1;
            Ok(Box::new(self.clone()))
        };
        ops::call(name, input, fonts, &mut connect)
    }

    fn writes(&self) -> Vec<String> {
        self.0.borrow().writes.clone()
    }
}

fn invalid_handle(operation: u16) -> MtpError {
    MtpError::Response {
        operation,
        code: 0x2009,
    }
}

fn timed_out(operation: u16) -> MtpError {
    MtpError::Transport {
        operation,
        phase: Phase::Reply,
        error: TransportError::TimedOut,
    }
}

/// The two MTP steps of creating an object, with the failures `State` scripts.
impl Fake {
    fn send_object_info(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
    ) -> Result<NewObject, MtpError> {
        let mut s = self.0.borrow_mut();
        s.writes
            .push(format!("send_object_info {parent:#x} {}", info.filename));
        let handle = s.next_handle;
        s.next_handle += 1;
        let info = ObjectInfo {
            storage_id: storage,
            parent,
            ..info.clone()
        };
        s.objects.insert(handle, (info, Vec::new()));
        match s.send_object_info_timeout {
            Some(created) => {
                if !created {
                    s.objects.remove(&handle);
                }
                Err(timed_out(op::SEND_OBJECT_INFO))
            }
            None => {
                s.awaiting_data = Some(handle);
                Ok(NewObject {
                    storage,
                    parent,
                    handle,
                })
            }
        }
    }

    fn send_object(&mut self, data: &[u8]) -> Result<(), MtpError> {
        let mut s = self.0.borrow_mut();
        s.writes.push("send_object".into());
        let handle = s.awaiting_data.take().ok_or(MtpError::Response {
            operation: op::SEND_OBJECT,
            code: 0x2015,
        })?;
        s.sends += 1;
        let fail = s.fail_send_object_at == Some(s.sends);
        let mut stored = data.to_vec();
        if fail {
            stored.truncate(data.len() / 2);
        } else if s.corrupt_storage || s.corrupt_send_at == Some(s.sends) {
            *stored.last_mut().unwrap() ^= 0xFF;
        }
        s.objects.get_mut(&handle).unwrap().1 = stored;
        if fail {
            return Err(timed_out(op::SEND_OBJECT));
        }
        Ok(())
    }
}

impl Mtp for Fake {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        Ok(DeviceInfo {
            operations_supported: self.0.borrow().operations.clone(),
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
            .filter(|(h, (i, _))| {
                !s.hidden.contains(h)
                    && i.storage_id == storage
                    && match parent {
                        ALL_OBJECTS => true,
                        ROOT => i.parent == 0,
                        p => i.parent == p,
                    }
            })
            .map(|(&h, _)| h)
            .collect())
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError> {
        let s = self.0.borrow();
        s.objects
            .get(&handle)
            .map(|(i, _)| i.clone())
            .ok_or(invalid_handle(op::GET_OBJECT_INFO))
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        let mut s = self.0.borrow_mut();
        s.reads.push(handle);
        s.objects
            .get(&handle)
            .map(|(_, d)| d.clone())
            .ok_or(invalid_handle(op::GET_OBJECT))
    }

    fn create_object(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
        data: &[u8],
    ) -> Result<NewObject, CreateError> {
        let new = self
            .send_object_info(storage, parent, info)
            .map_err(CreateError::Unconfirmed)?;
        let misplace_into = self.0.borrow().misplace_into;
        if let Some(other) = misplace_into {
            let mut s = self.0.borrow_mut();
            s.awaiting_data = None;
            s.objects.get_mut(&new.handle).unwrap().0.parent = other;
            return Err(CreateError::Misplaced(NewObject {
                parent: other,
                ..new
            }));
        }
        self.send_object(data).map_err(|error| CreateError::Data {
            handle: new.handle,
            error,
        })?;
        Ok(new)
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        let mut s = self.0.borrow_mut();
        s.writes.push(format!("delete_object {handle:#x}"));
        if s.fail_delete {
            return Err(timed_out(op::DELETE_OBJECT));
        }
        s.objects
            .remove(&handle)
            .map(drop)
            .ok_or(invalid_handle(op::DELETE_OBJECT))
    }

    /// As the tactix 7 answers it: only the children standard listings leave out.
    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let s = self.0.borrow();
        let refuse = |code| MtpError::Response {
            operation: op::GARMIN_GET_OBJECT_IDS,
            code,
        };
        if !s.operations.contains(&op::GARMIN_GET_OBJECT_IDS) {
            return Err(refuse(0x2005));
        }
        if s.object_ids_fail {
            return Err(refuse(0x2002));
        }
        Ok(s.hidden
            .iter()
            .copied()
            .filter(|h| {
                s.objects
                    .get(h)
                    .is_some_and(|(i, _)| i.storage_id == storage && i.parent == parent)
            })
            .chain(s.object_ids_extra.iter().copied())
            .collect())
    }
}

/// Distinct bytes per part, with Georgian and stock builds of different lengths.
fn bytes_for(part: &str, base: usize, step: u32) -> Vec<u8> {
    let seed: usize = part.bytes().map(usize::from).sum();
    (0..(base + seed % 97) as u32)
        .map(|i| ((i * step + seed as u32) % 251) as u8)
        .collect()
}

fn georgian(part: &str) -> Vec<u8> {
    bytes_for(part, 33_000, 7)
}

fn stock(part: &str) -> Vec<u8> {
    bytes_for(part, 20_700, 13)
}

fn pack(id: &str, parts: &[(&str, u16)], tested: Vec<Tested>) -> Pack {
    Pack {
        id: id.into(),
        name: format!("{id} pack"),
        kind: Kind::Bitmap,
        weight: 400,
        fonts: parts
            .iter()
            .map(|&(part, size_px)| Font {
                part: part.into(),
                kind: Kind::Bitmap,
                size_px: Some(size_px),
                version: "2.10".into(),
                stock: Cow::Owned(stock(part)),
                georgian: Cow::Owned(georgian(part)),
                mark: None,
            })
            .collect(),
        tested,
    }
}

/// A TrueType pack of `HEBREW`, version 2.10, untested.
fn truetype_pack() -> Pack {
    Pack {
        id: "hebrew".into(),
        name: "hebrew pack".into(),
        kind: Kind::Truetype,
        weight: 400,
        fonts: HEBREW
            .iter()
            .map(|&part| Font {
                part: part.into(),
                kind: Kind::Truetype,
                size_px: None,
                version: "2.10".into(),
                stock: Cow::Owned(stock(part)),
                georgian: Cow::Owned(georgian(part)),
                mark: None,
            })
            .collect(),
        tested: vec![],
    }
}

/// A `tested` entry for a Venu 2 (`006-B3703-00`).
fn venu2_tested(software: &str, parts: &[&str]) -> Tested {
    Tested {
        model: "Venu 2".into(),
        model_part: "006-B3703-00".into(),
        software: software.into(),
        parts: parts.iter().map(|&p| p.into()).collect(),
    }
}

/// A test registry: Armenian, then Thai, with these `tested` lists.
fn fonts_tested(armenian: Vec<Tested>, thai: Vec<Tested>) -> FontSet {
    FontSet {
        packs: vec![
            pack("armenian", &ARMENIAN, armenian),
            pack("thai", &THAI, thai),
        ],
    }
}

/// The test registry: Armenian (all four parts verified on a Venu 2 at 19.05), then Thai
/// (untested), which shares the 21 and 27 px sizes.
fn fonts() -> FontSet {
    fonts_tested(vec![venu2_tested("19.05", &ALL)], vec![])
}

/// The test registry with the TrueType pack after Armenian and Thai.
fn fonts_tt() -> FontSet {
    let mut fonts = fonts();
    fonts.packs.push(truetype_pack());
    fonts
}

fn error_code(result: Result<Value, Error>) -> ErrorCode {
    result.expect_err("operation should fail").code
}

fn staged(fake: &Fake, part: &str) -> Vec<(u32, Vec<u8>)> {
    fake.0.borrow().named(EXTDATA, &format!("{part}.tmp"))
}

/// Every staged part of a stage output, across its packs.
fn staged_parts(out: &Value) -> Vec<&Value> {
    out["packs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["parts"].as_array().unwrap())
        .collect()
}

fn statuses(out: &Value) -> Vec<(String, String)> {
    staged_parts(out)
        .iter()
        .map(|p| {
            (
                p["part"].as_str().unwrap().to_owned(),
                p["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|&(a, b)| (a.to_owned(), b.to_owned()))
        .collect()
}

// Discovery

#[test]
fn finds_extdata_listed_under_garmin() {
    let mut s = venu2(&[PART]);
    s.hidden.clear();
    let out = Fake::new(s).call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(out["extdata"]["handle"], "0x01000009");
    assert_eq!(out["extdata"]["listed_under_garmin"], true);
    assert_eq!(out["extdata"]["found_by"], "garmin_listing");
}

#[test]
fn finds_hidden_extdata_through_a_staged_font() {
    // A pack this build doesn't carry still points at ExtData.
    let mut s = venu2(&[PART]);
    s.file(0x0100_0050, EXTDATA, "006-D1234-20.tmp", b"cjk");
    let out = Fake::new(s).call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(out["extdata"]["found_by"], "font_parent");
    assert_eq!(out["extdata"]["listed_under_garmin"], false);
    assert_eq!(
        out["extdata"]["files"],
        // Not a part of this build, so listed but never read.
        json!([{"name": "006-D1234-20.tmp", "handle": "0x01000050", "bytes": 3,
                "sha256": null}])
    );
}

#[test]
fn finds_unlisted_empty_extdata_by_probe() {
    let out = Fake::new(venu2(&[PART]))
        .call("inspect", json!({}), &fonts())
        .unwrap();
    assert_eq!(
        out["device"],
        json!({"model": "Venu 2", "part_number": "006-B3703-00", "software_version": "19.05"})
    );
    assert_eq!(out["supported"], true);
    assert_eq!(
        out["extdata"],
        json!({"handle": "0x01000009", "storage_id": "0x00020001", "listed_under_garmin": false,
               "found_by": "handle_probe", "files": []})
    );
    assert_eq!(
        out["fonts"][2],
        json!({"pack": "armenian", "part": PART, "kind": "bitmap", "size_px": 27, "listed": true,
               "target": true, "staged": "none", "mark": null})
    );
    assert_eq!(out["fonts"].as_array().unwrap().len(), 6);
}

#[test]
fn probe_window_ends_at_32_handles() {
    for (handle, found) in [(0x0100_0020, true), (0x0100_0021, false)] {
        let mut s = venu2(&[PART]);
        s.objects.remove(&EXTDATA);
        s.hidden.clear();
        s.folder(handle, GARMIN, "ExtData");
        s.hidden.insert(handle);
        let out = Fake::new(s).call("inspect", json!({}), &fonts()).unwrap();
        assert_eq!(out["extdata"].is_null(), !found, "handle {handle:#x}");
    }
}

#[test]
fn probe_rejects_wrong_name_type_parent_and_storage() {
    let mut s = venu2(&[PART]);
    s.objects.remove(&EXTDATA);
    s.hidden.clear();
    s.folder(0x0100_0005, GARMIN, "extdata");
    s.add(
        0x0100_0006,
        STORAGE,
        GARMIN,
        "ExtData",
        format::UNDEFINED,
        Vec::new(),
    );
    s.folder(0x0100_0007, 0x0100_0004, "ExtData");
    s.add(
        0x0100_0008,
        OTHER_STORAGE,
        GARMIN,
        "ExtData",
        format::ASSOCIATION,
        Vec::new(),
    );
    s.hidden
        .extend([0x0100_0005, 0x0100_0006, 0x0100_0007, 0x0100_0008]);
    let fake = Fake::new(s);
    let out = fake.call("inspect", json!({}), &fonts()).unwrap();
    assert!(out["extdata"].is_null());
    assert_eq!(out["fonts"][0]["staged"], "unknown");
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn finds_extdata_past_the_probe_window_by_garmin_object_ids() {
    let out = Fake::new(tactix7(&[PART]))
        .call("inspect", json!({}), &fonts())
        .unwrap();
    assert_eq!(
        out["extdata"],
        json!({"handle": "0x01000021", "storage_id": "0x00020001", "listed_under_garmin": false,
               "found_by": "garmin_object_ids", "files": []})
    );
    assert_eq!(out["fonts"][2]["staged"], "none");
}

#[test]
fn installs_into_extdata_found_by_garmin_object_ids() {
    let fake = Fake::new(tactix7(&ALL));
    let out = fake.call("install", json!({}), &fonts()).unwrap();
    assert_eq!(out["status"], "staged_verified");
    assert_eq!(statuses(&out), pairs(&ALL.map(|p| (p, "staged_verified"))));
    for part in ALL {
        let files = fake.0.borrow().named(PAST_PROBE, &format!("{part}.tmp"));
        assert_eq!(files.len(), 1, "{part}");
        assert_eq!(files[0].1, georgian(part));
    }
}

#[test]
fn garmin_object_ids_is_used_only_when_the_watch_lists_it() {
    let mut s = tactix7(&[PART]);
    s.operations.retain(|&o| o != op::GARMIN_GET_OBJECT_IDS);
    let fake = Fake::new(s);
    assert!(fake.call("inspect", json!({}), &fonts()).unwrap()["extdata"].is_null());
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn a_failing_garmin_object_ids_means_extdata_not_found() {
    let mut s = tactix7(&[PART]);
    s.object_ids_fail = true;
    let fake = Fake::new(s);
    assert!(fake.call("inspect", json!({}), &fonts()).unwrap()["extdata"].is_null());
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn garmin_object_ids_candidates_are_checked() {
    let mut s = tactix7(&[PART]);
    s.objects.remove(&PAST_PROBE);
    s.hidden.clear();
    // Wrong name, not a folder, another folder's child, another storage, and a missing handle.
    s.folder(0x0100_0030, GARMIN, "extdata");
    s.add(
        0x0100_0031,
        STORAGE,
        GARMIN,
        "ExtData",
        format::UNDEFINED,
        Vec::new(),
    );
    s.folder(0x0100_0032, 0x0100_0004, "ExtData");
    s.add(
        0x0100_0033,
        OTHER_STORAGE,
        GARMIN,
        "ExtData",
        format::ASSOCIATION,
        Vec::new(),
    );
    s.hidden
        .extend([0x0100_0030, 0x0100_0031, 0x0100_0032, 0x0100_0033]);
    s.object_ids_extra = vec![0x0100_0032, 0x0100_0033, 0x0100_0099];
    let fake = Fake::new(s);
    assert!(fake.call("inspect", json!({}), &fonts()).unwrap()["extdata"].is_null());
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn two_extdata_folders_from_garmin_object_ids_are_refused() {
    let mut s = tactix7(&[PART]);
    s.folder(0x0100_0040, GARMIN, "ExtData");
    s.hidden.insert(0x0100_0040);
    let fake = Fake::new(s);
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn absent_extdata_blocks_writes() {
    let mut s = venu2(&ALL);
    s.objects.remove(&EXTDATA);
    let fake = Fake::new(s);
    assert!(fake.call("inspect", json!({}), &fonts()).unwrap()["extdata"].is_null());
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert_eq!(
        error_code(fake.call("restore_stock", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn two_extdata_folders_are_refused() {
    let mut s = venu2(&[PART]);
    s.hidden.clear();
    s.folder(0x0100_000A, GARMIN, "ExtData");
    let fake = Fake::new(s);
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::ExtdataNotFound
    );
    assert!(fake.writes().is_empty());
}

// Targets: every listed part of every pack

fn selected(declared: &[&str]) -> Value {
    Fake::new(venu2(declared))
        .call("inspect", json!({}), &fonts())
        .unwrap()
}

fn targets(out: &Value) -> Vec<&str> {
    out["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["target"] == true)
        .map(|f| f["part"].as_str().unwrap())
        .collect()
}

fn pack_ids(out: &Value) -> Vec<&str> {
    out["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect()
}

/// `(part, tested)` of every listed part, across packs.
fn part_tested(out: &Value) -> Vec<(String, String)> {
    out["packs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["parts"].as_array().unwrap())
        .map(|p| {
            (
                p["part"].as_str().unwrap().to_owned(),
                p["tested"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn one_listed_pack_targets_every_listed_size() {
    let out = selected(&ALL);
    assert_eq!(
        out["packs"],
        json!([{"id": "armenian", "name": "armenian pack", "kind": "bitmap", "weight": 400, "parts": [
            {"part": ALL[0], "size_px": 21, "tested": "tested"},
            {"part": ALL[1], "size_px": 24, "tested": "tested"},
            {"part": ALL[2], "size_px": 27, "tested": "tested"},
            {"part": ALL[3], "size_px": 31, "tested": "tested"},
        ]}])
    );
    assert_eq!(targets(&out), ALL);
    assert_eq!(out["supported"], true);
}

#[test]
fn every_listed_part_of_every_pack_is_a_target() {
    // The watch lists Thai first; packs come in registry order.
    let out = selected(&[THAI[0].0, THAI[1].0, ARMENIAN[1].0]);
    assert_eq!(pack_ids(&out), ["armenian", "thai"]);
    assert_eq!(
        out["packs"][0]["parts"],
        json!([{"part": ARMENIAN[1].0, "size_px": 24, "tested": "tested"}])
    );
    assert_eq!(
        out["packs"][1]["parts"],
        json!([{"part": THAI[0].0, "size_px": 21, "tested": "untested"},
               {"part": THAI[1].0, "size_px": 27, "tested": "untested"}])
    );
    assert_eq!(targets(&out), [ARMENIAN[1].0, THAI[0].0, THAI[1].0]);
    assert_eq!(out["report"]["packs"], json!(["armenian", "thai"]));

    let out = selected(&[THAI[1].0]);
    assert_eq!(pack_ids(&out), ["thai"]);
    assert_eq!(targets(&out), [THAI[1].0]);
}

#[test]
fn a_watch_listing_no_pack_has_no_targets() {
    let out = selected(&["006-D1234-01"]);
    assert_eq!(out["packs"], json!([]));
    assert_eq!(out["supported"], false);
    assert_eq!(
        out["tested"],
        json!({"status": "untested", "verified_software": []})
    );
    assert!(targets(&out).is_empty());
    assert_eq!(out["report"]["packs"], json!([]));
    assert_eq!(out["report"]["tested"], "untested");
}

#[test]
fn a_partial_list_targets_only_the_listed_sizes() {
    let out = selected(&[ARMENIAN[0].0, ARMENIAN[2].0]);
    assert_eq!(pack_ids(&out), ["armenian"]);
    assert_eq!(
        part_tested(&out),
        pairs(&[(ARMENIAN[0].0, "tested"), (ARMENIAN[2].0, "tested")])
    );
    assert_eq!(targets(&out), [ARMENIAN[0].0, ARMENIAN[2].0]);
}

// Tested status: per part, and overall the worst of the targets'

fn tested_with(fonts: &FontSet, model_part: &str, software: &str, declared: &[&str]) -> Value {
    Fake::new(venu2_with(device_xml_for(model_part, software, declared)))
        .call("inspect", json!({}), fonts)
        .unwrap()
}

fn tested(model_part: &str, software: &str) -> Value {
    tested_with(&fonts(), model_part, software, &ALL)
}

const ALL_AND_THAI: [&str; 6] = [ALL[0], ALL[1], ALL[2], ALL[3], THAI[0].0, THAI[1].0];

#[test]
fn every_target_tested_on_this_software_is_tested() {
    // The Venu 2 case: four parts of one pack, all verified at 19.05.
    let out = tested("006-B3703-00", "1905");
    assert_eq!(
        out["tested"],
        json!({"status": "tested", "verified_software": ["19.05"]})
    );
    assert_eq!(part_tested(&out), pairs(&ALL.map(|p| (p, "tested"))));
    assert_eq!(out["report"]["tested"], "tested");

    // Across two packs too, and an exact entry beats one for other software.
    let fonts = fonts_tested(
        vec![venu2_tested("18.00", &ALL), venu2_tested("19.05", &ALL)],
        vec![venu2_tested("19.05", &[THAI[0].0, THAI[1].0])],
    );
    let out = tested_with(&fonts, "006-B3703-00", "1905", &ALL_AND_THAI);
    assert_eq!(
        out["tested"],
        json!({"status": "tested", "verified_software": ["18.00", "19.05"]})
    );
}

#[test]
fn same_model_on_other_software_is_tested_other_software() {
    let out = tested("006-B3703-00", "2026");
    assert_eq!(out["device"]["software_version"], "20.26");
    assert_eq!(
        out["tested"],
        json!({"status": "tested_other_software", "verified_software": ["19.05"]})
    );
    assert_eq!(
        part_tested(&out),
        pairs(&ALL.map(|p| (p, "tested_other_software")))
    );

    // One pack verified on this software, the other only on another: the worse status wins.
    let fonts = fonts_tested(
        vec![venu2_tested("19.05", &ALL)],
        vec![venu2_tested("18.00", &[THAI[0].0, THAI[1].0])],
    );
    let out = tested_with(&fonts, "006-B3703-00", "1905", &ALL_AND_THAI);
    assert_eq!(
        out["tested"],
        json!({"status": "tested_other_software", "verified_software": ["18.00", "19.05"]})
    );
    assert_eq!(out["report"]["tested"], "tested_other_software");
}

#[test]
fn some_targets_untested_make_the_watch_untested() {
    // Armenian verified, Thai not.
    let out = tested_with(&fonts(), "006-B3703-00", "1905", &ALL_AND_THAI);
    assert_eq!(
        out["tested"],
        json!({"status": "untested", "verified_software": ["19.05"]})
    );
    assert_eq!(
        part_tested(&out),
        pairs(&[
            (ALL[0], "tested"),
            (ALL[1], "tested"),
            (ALL[2], "tested"),
            (ALL[3], "tested"),
            (THAI[0].0, "untested"),
            (THAI[1].0, "untested"),
        ])
    );
    assert_eq!(out["report"]["tested"], "untested");

    // An entry for this model and software covers only the parts it names.
    let fonts = fonts_tested(vec![venu2_tested("19.05", &[PART])], vec![]);
    let out = tested_with(&fonts, "006-B3703-00", "1905", &ALL);
    assert_eq!(out["tested"]["status"], "untested");
    assert_eq!(
        part_tested(&out),
        pairs(&[
            (ALL[0], "untested"),
            (ALL[1], "untested"),
            (PART, "tested"),
            (ALL[3], "untested"),
        ])
    );
}

#[test]
fn another_model_is_untested() {
    let out = tested("006-B4260-00", "1905");
    assert_eq!(
        out["tested"],
        json!({"status": "untested", "verified_software": []})
    );
    assert_eq!(part_tested(&out), pairs(&ALL.map(|p| (p, "untested"))));
    assert_eq!(out["report"]["tested"], "untested");
    assert_eq!(out["supported"], true);
}

// Watch report

const UNIT_ID: &str = "3398765432";

/// A `GarminDevice.xml` shaped like a real one, with the identifiers a report must leave out.
fn identifying_xml() -> Vec<u8> {
    format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">
  <Model><PartNumber>006-B3703-00</PartNumber><SoftwareVersion>1905</SoftwareVersion><Description>Venu 2</Description></Model>
  <Id>{UNIT_ID}</Id>
  <SerialNumber>SN-77X1</SerialNumber>
  <Extensions><Id>99001122</Id></Extensions>
  <MassStorageMode>
    <DataType><Name>FIT</Name><File><Location><Path>Garmin/Activity</Path></Location></File></DataType>
    <UpdateFile><PartNumber>006-B3703-00</PartNumber><Path>Garmin/RemoteSW</Path><FileName>GUPDATE.GCD</FileName></UpdateFile>
    <UpdateFile><PartNumber>006-D9035-03</PartNumber><Version><Major>2</Major><Minor>10</Minor></Version><Path>Garmin/ExtData</Path><FileName>006-D9035-03.tmp</FileName></UpdateFile>
    <UpdateFile><PartNumber>006-D8945-20</PartNumber><Version><Major>3</Major><Minor>5</Minor></Version><Path>GARMIN/EXTDATA</Path><FileName>006-D8945-20.tmp</FileName></UpdateFile>
    <UpdateFile><PartNumber>006-D7777-01</PartNumber><Path>Garmin/ExtData</Path><FileName>x|y.tmp</FileName></UpdateFile>
  </MassStorageMode>
</Device>"
    )
    .into_bytes()
}

#[test]
fn report_lists_the_font_list_and_excludes_ids() {
    let out = Fake::new(venu2_with(identifying_xml()))
        .call("inspect", json!({}), &fonts())
        .unwrap();
    let report = &out["report"];
    assert_eq!(report["app_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["model"], "Venu 2");
    assert_eq!(report["model_part"], "006-B3703-00");
    assert_eq!(report["software_version"], "19.05");
    assert_eq!(report["packs"], json!(["armenian"]));
    assert_eq!(report["tested"], "tested");
    assert_eq!(
        report["extdata_files"],
        json!([
            {"part_number": "006-D9035-03", "version": "2.10", "file_name": "006-D9035-03.tmp"},
            {"part_number": "006-D8945-20", "version": "3.05", "file_name": "006-D8945-20.tmp"},
            {"part_number": "006-D7777-01", "version": null, "file_name": "x|y.tmp"},
        ])
    );
    assert_eq!(
        report["title"],
        "Watch report: Venu 2 (006-B3703-00), software 19.05"
    );
    assert_eq!(report["connection"], "mtp");
    assert_eq!(report["test_marks"], false);
    assert_eq!(
        report["declared_files"][0],
        json!({"path": "Garmin/RemoteSW", "file_name": "GUPDATE.GCD", "part_number": "006-B3703-00", "version": null})
    );
    assert_eq!(
        report["parts"],
        json!([{"part": "006-D9035-03", "target": true, "staged": "none", "tested": "tested", "mark": null}])
    );
    assert_eq!(report["extdata_listing"], json!([]));
    let text = report["text"].as_str().unwrap();
    for line in [
        "connection mtp",
        "watch \"Venu 2\" 006-B3703-00 19.05",
        "packs armenian (tested)",
        "006-D9035-03 none yes tested -",
        "Garmin/RemoteSW GUPDATE.GCD 006-B3703-00 -",
        "Garmin/ExtData 006-D9035-03.tmp 006-D9035-03 2.10",
        "GARMIN/EXTDATA 006-D8945-20.tmp 006-D8945-20 3.05",
        "Garmin/ExtData x|y.tmp 006-D7777-01 -",
        "extdata: name bytes sha256",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "{line:?} missing from\n{text}"
        );
    }
    assert!(text.starts_with(&format!("app {} ", env!("CARGO_PKG_VERSION"))));
    let everything = out.to_string();
    for secret in [UNIT_ID, "99001122", "SN-77X1"] {
        assert!(!everything.contains(secret), "{secret} leaked");
    }
    // The report holds only the allowlisted fields.
    let keys: Vec<&str> = report
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut expected = vec![
        "app_version",
        "connection",
        "declared_files",
        "extdata_files",
        "extdata_listing",
        "hosts",
        "model",
        "model_part",
        "packs",
        "parts",
        "platform",
        "software_version",
        "test_marks",
        "tested",
        "text",
        "title",
    ];
    expected.sort_unstable();
    let mut keys = keys;
    keys.sort_unstable();
    assert_eq!(keys, expected);
}

#[test]
fn the_report_links_to_a_filled_form() {
    let out = Fake::new(venu2(&ALL))
        .call("inspect", json!({}), &fonts())
        .unwrap();
    let input = json!({
        "report": out["report"],
        "outcome": {"operation": "install", "status": "staged_verified", "parts": ALL},
        "answers": {"result": "georgian", "views": ["opened"], "phone": "iphone"},
    });
    let link = ops::call("issue_link", input, &fonts(), &mut || {
        panic!("issue_link never opens the watch")
    })
    .unwrap();
    assert_eq!(link["report_in_url"], true);
    let text = link["report_text"].as_str().unwrap();
    assert!(text.contains(&format!(
        "\nlast install staged_verified {}\n",
        ALL.join(" ")
    )));
    assert!(text.contains("\nanswers result=georgian views=opened phone=iphone\n"));
    let url = link["url"].as_str().unwrap();
    assert!(url.contains("&phone=iPhone&"), "{url}");
}

#[test]
fn beta_marks_are_in_inspect_and_the_report() {
    let mut fonts = fonts();
    for (i, font) in fonts.packs[0].fonts.iter_mut().enumerate() {
        font.mark = Some(Mark {
            variant: char::from(b'A' + i as u8).to_string(),
            code_point: "U+1C9B".into(),
            position: i as u8,
            style: "below".into(),
            picture: Cow::Borrowed(b"png"),
        });
    }
    let out = Fake::new(venu2(&[PART])).call("inspect", json!({}), &fonts).unwrap();
    assert_eq!(out["fonts"][2]["mark"], "C");
    assert_eq!(out["fonts"][4]["mark"], Value::Null, "Thai has no marks");
    assert_eq!(out["report"]["test_marks"], true);
    assert_eq!(out["report"]["parts"][0]["mark"], "C");
    let text = out["report"]["text"].as_str().unwrap();
    assert!(text.contains("\nmarks beta\n"));
    assert!(text.contains("\n006-D9035-03 none yes tested C\n"), "{text}");
}

#[test]
fn a_watch_mounted_as_a_drive_gets_the_same_report() {
    let root = std::env::temp_dir().join(format!("gfg-ops-drive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("GARMIN")).unwrap();
    std::fs::write(root.join("GARMIN/GarminDevice.xml"), identifying_xml()).unwrap();
    let out = ops::call(
        "inspect_drive",
        json!({"path": root}),
        &fonts(),
        &mut || panic!("inspect_drive never opens MTP"),
    )
    .unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let report = &out["report"];
    assert_eq!(report["connection"], "usb_drive");
    assert_eq!(report["model"], "Venu 2");
    assert_eq!(
        report["parts"],
        json!([{"part": "006-D9035-03", "target": false, "staged": "unknown", "tested": "tested", "mark": null}])
    );
    assert_eq!(report["extdata_listing"], Value::Null);
    assert!(report["text"].as_str().unwrap().contains("\nextdata not found\n"));
    let everything = out.to_string();
    for secret in [UNIT_ID, "99001122", "SN-77X1"] {
        assert!(!everything.contains(secret), "{secret} leaked");
    }
}

// Install and restore

#[test]
fn install_stages_every_listed_size_and_verifies() {
    let fake = Fake::new(venu2(&ALL));
    let out = fake.call("install", json!({}), &fonts()).unwrap();
    assert_eq!(out["status"], "staged_verified");
    assert_eq!(out["variant"], "georgian");
    assert_eq!(pack_ids(&out), ["armenian"]);
    assert_eq!(out["packs"][0]["name"], "armenian pack");
    assert_eq!(out["packs"][0]["weight"], 400);
    assert_eq!(statuses(&out), pairs(&ALL.map(|p| (p, "staged_verified"))));
    let mut expected_writes = Vec::new();
    for (i, (part, size)) in ARMENIAN.iter().enumerate() {
        let files = staged(&fake, part);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, georgian(part));
        let reported = staged_parts(&out)[i];
        assert_eq!(reported["size_px"], *size);
        assert_eq!(reported["path"], format!("GARMIN/ExtData/{part}.tmp"));
        assert_eq!(reported["handle"], format!("{:#010x}", files[0].0));
        assert_eq!(reported["bytes"], georgian(part).len());
        assert_eq!(reported["sha256"].as_str().unwrap().len(), 64);
        expected_writes.push(format!("send_object_info 0x1000009 {part}.tmp"));
        expected_writes.push("send_object".into());
    }
    assert_eq!(fake.writes(), expected_writes);

    let after = fake.call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(after["extdata"]["found_by"], "font_parent");
    for font in &after["fonts"].as_array().unwrap()[..4] {
        assert_eq!(font["staged"], "georgian");
    }

    // Running it again changes nothing.
    let again = fake.call("install", json!({}), &fonts()).unwrap();
    assert_eq!(again["status"], "already_staged");
    assert_eq!(statuses(&again), pairs(&ALL.map(|p| (p, "already_staged"))));
    assert_eq!(fake.writes().len(), 8);
}

#[test]
fn install_skips_sizes_already_staged() {
    let mut s = venu2(&ALL);
    s.file(0x0100_0050, EXTDATA, STAGED, &georgian(PART));
    let fake = Fake::new(s);
    let out = fake.call("install", json!({}), &fonts()).unwrap();
    assert_eq!(out["status"], "staged_verified");
    assert_eq!(
        statuses(&out),
        pairs(&[
            (ALL[0], "staged_verified"),
            (ALL[1], "staged_verified"),
            (PART, "already_staged"),
            (ALL[3], "staged_verified"),
        ])
    );
    assert_eq!(staged_parts(&out)[2]["handle"], "0x01000050");
    assert!(!fake.writes().iter().any(|w| w.contains(STAGED)));
    assert_eq!(fake.writes().len(), 6);
}

#[test]
fn install_of_identical_file_writes_nothing() {
    let mut s = venu2(&[PART]);
    s.file(0x0100_0050, EXTDATA, STAGED, &georgian(PART));
    let fake = Fake::new(s);
    let out = fake
        .call("install", json!({"parts": [PART]}), &fonts())
        .unwrap();
    assert_eq!(out["status"], "already_staged");
    assert_eq!(staged_parts(&out)[0]["handle"], "0x01000050");
    assert!(fake.writes().is_empty());
}

#[test]
fn a_conflict_on_any_size_stops_before_writing() {
    let last = ALL[3];
    for existing in [stock(last), {
        let mut g = georgian(last);
        g[100] ^= 1;
        g
    }] {
        let mut s = venu2(&ALL);
        s.file(0x0100_0050, EXTDATA, &format!("{last}.tmp"), &existing);
        let fake = Fake::new(s);
        let err = fake.call("install", json!({}), &fonts()).unwrap_err();
        assert_eq!(err.code, ErrorCode::Conflict);
        assert_eq!(
            err.message,
            "A different file is already staged as GARMIN/ExtData/006-D9035-04.tmp; nothing was \
             written. Unplug the watch and restart it so it loads that file, then try again."
        );
        assert!(fake.writes().is_empty());
        assert_eq!(staged(&fake, last), [(0x0100_0050, existing)]);
        for part in &ALL[..3] {
            assert!(staged(&fake, part).is_empty());
        }
    }
}

#[test]
fn failure_on_a_later_part_rolls_back_this_runs_files_only() {
    for undeletable in [false, true] {
        let mut s = venu2(&ALL);
        // Staged by an earlier run.
        s.file(0x0100_0050, EXTDATA, "006-D9035-04.tmp", &georgian(ALL[3]));
        s.fail_send_object_at = Some(3);
        s.fail_delete = undeletable;
        let fake = Fake::new(s);
        let err = fake.call("install", json!({}), &fonts()).unwrap_err();
        assert_eq!(err.code, ErrorCode::DeviceError);
        let timed_out = "DeleteObject timed out waiting for the reply";
        let expected = if undeletable {
            format!(
                "SendObject timed out waiting for the reply. A new object was created as \
                 GARMIN/ExtData/006-D9035-03.tmp (0x01000102) and could not be deleted \
                 ({timed_out}); it may hold incomplete data. Rolled back the files this run had \
                 staged: GARMIN/ExtData/006-D9035-01.tmp (0x01000100) could not be deleted \
                 ({timed_out}); GARMIN/ExtData/006-D9035-02.tmp (0x01000101) could not be \
                 deleted ({timed_out}). Files staged before this run were left in place."
            )
        } else {
            "SendObject timed out waiting for the reply. A new object was created as \
             GARMIN/ExtData/006-D9035-03.tmp (0x01000102) and has been deleted. Rolled back the \
             files this run had staged: GARMIN/ExtData/006-D9035-01.tmp (0x01000100) deleted; \
             GARMIN/ExtData/006-D9035-02.tmp (0x01000101) deleted. Files staged before this run \
             were left in place."
                .to_owned()
        };
        assert_eq!(err.message, expected);
        assert_eq!(
            fake.writes()[6..],
            [
                "delete_object 0x1000102",
                "delete_object 0x1000100",
                "delete_object 0x1000101"
            ]
        );
        assert_eq!(fake.writes().len(), 9, "part 4 was never written");
        assert_eq!(
            staged(&fake, ALL[3]),
            [(0x0100_0050, georgian(ALL[3]))],
            "the earlier run's file stays"
        );
        if !undeletable {
            for part in &ALL[..3] {
                assert!(staged(&fake, part).is_empty(), "{part}");
            }
        }
    }
}

// Several packs

#[test]
fn install_stages_every_listed_part_of_every_pack() {
    let fake = Fake::new(venu2(&ALL_AND_THAI));
    let out = fake.call("install", json!({}), &fonts()).unwrap();
    assert_eq!(out["status"], "staged_verified");
    assert_eq!(pack_ids(&out), ["armenian", "thai"]);
    assert_eq!(
        statuses(&out),
        pairs(&ALL_AND_THAI.map(|p| (p, "staged_verified")))
    );
    for part in ALL_AND_THAI {
        let files = staged(&fake, part);
        assert_eq!(files.len(), 1, "{part}");
        assert_eq!(files[0].1, georgian(part), "{part}");
    }
    assert_eq!(fake.writes().len(), 12);
    let after = fake.call("inspect", json!({}), &fonts()).unwrap();
    assert!(
        after["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["staged"] == "georgian")
    );

    // Running it again changes nothing.
    let again = fake.call("install", json!({}), &fonts()).unwrap();
    assert_eq!(again["status"], "already_staged");
    assert_eq!(
        statuses(&again),
        pairs(&ALL_AND_THAI.map(|p| (p, "already_staged")))
    );
    assert_eq!(fake.writes().len(), 12);
}

#[test]
fn a_conflict_in_the_second_pack_blocks_every_pack() {
    let mut s = venu2(&ALL_AND_THAI);
    s.file(0x0100_0050, EXTDATA, "006-D8947-02.tmp", &stock(THAI[1].0));
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(
        err.message,
        "A different file is already staged as GARMIN/ExtData/006-D8947-02.tmp; nothing was \
         written. Unplug the watch and restart it so it loads that file, then try again."
    );
    assert!(fake.writes().is_empty());
    for part in &ALL_AND_THAI[..5] {
        assert!(staged(&fake, part).is_empty(), "{part}");
    }
}

#[test]
fn a_failure_in_the_second_pack_rolls_back_the_first_packs_files() {
    let mut s = venu2(&ALL_AND_THAI);
    // Staged by an earlier run.
    s.file(0x0100_0050, EXTDATA, "006-D9035-04.tmp", &georgian(ALL[3]));
    // The first Thai part is the fourth file sent.
    s.fail_send_object_at = Some(4);
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::DeviceError);
    assert_eq!(
        err.message,
        "SendObject timed out waiting for the reply. A new object was created as \
         GARMIN/ExtData/006-D8947-01.tmp (0x01000103) and has been deleted. Rolled back the files \
         this run had staged: GARMIN/ExtData/006-D9035-01.tmp (0x01000100) deleted; \
         GARMIN/ExtData/006-D9035-02.tmp (0x01000101) deleted; GARMIN/ExtData/006-D9035-03.tmp \
         (0x01000102) deleted. Files staged before this run were left in place."
    );
    assert_eq!(
        fake.writes()[8..],
        [
            "delete_object 0x1000103",
            "delete_object 0x1000100",
            "delete_object 0x1000101",
            "delete_object 0x1000102"
        ]
    );
    assert_eq!(
        fake.writes().len(),
        12,
        "the second Thai part was never written"
    );
    assert_eq!(
        staged(&fake, ALL[3]),
        [(0x0100_0050, georgian(ALL[3]))],
        "the earlier run's file stays"
    );
    for part in [ALL[0], ALL[1], ALL[2], THAI[0].0, THAI[1].0] {
        assert!(staged(&fake, part).is_empty(), "{part}");
    }
}

#[test]
fn parts_restricts_the_targets_across_packs() {
    // Two 21 px parts from different packs, and the 31 px one.
    let fake = Fake::new(venu2(&ALL_AND_THAI));
    let out = fake
        .call(
            "install",
            json!({"parts": [THAI[0].0, ALL[3], ALL[0]]}),
            &fonts(),
        )
        .unwrap();
    assert_eq!(pack_ids(&out), ["armenian", "thai"]);
    assert_eq!(
        statuses(&out),
        pairs(&[
            (ALL[0], "staged_verified"),
            (ALL[3], "staged_verified"),
            (THAI[0].0, "staged_verified"),
        ])
    );
    assert_eq!(fake.writes().len(), 6);
}

#[test]
fn a_part_the_watch_does_not_list_is_unsupported() {
    let fake = Fake::new(venu2(&[PART]));
    let err = fake
        .call("install", json!({"parts": [ALL[1], PART]}), &fonts())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedDevice);
    assert_eq!(
        err.message,
        "Venu 2 (006-B3703-00) doesn't list part 006-D9035-02; of this build's parts it lists \
         006-D9035-03. Nothing was written."
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn readback_mismatch_deletes_the_new_file() {
    let mut s = venu2(&[PART]);
    s.corrupt_storage = true;
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::VerificationFailed);
    assert!(
        err.message.ends_with(
            "A new object was created as GARMIN/ExtData/006-D9035-03.tmp (0x01000100) and has been deleted."
        ),
        "{}",
        err.message
    );
    assert!(fake.0.borrow().named(EXTDATA, STAGED).is_empty());
    assert_eq!(fake.writes().last().unwrap(), "delete_object 0x1000100");
}

#[test]
fn transfer_failure_deletes_the_partial_file() {
    let mut s = venu2(&[PART]);
    s.fail_send_object_at = Some(1);
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::DeviceError);
    assert_eq!(
        err.message,
        "SendObject timed out waiting for the reply. A new object was created as \
         GARMIN/ExtData/006-D9035-03.tmp (0x01000100) and has been deleted."
    );
    assert!(fake.0.borrow().named(EXTDATA, STAGED).is_empty());
    assert_eq!(
        fake.writes(),
        [
            "send_object_info 0x1000009 006-D9035-03.tmp",
            "send_object",
            "delete_object 0x1000100"
        ]
    );
}

#[test]
fn an_object_placed_outside_extdata_gets_no_data_and_is_deleted() {
    let mut s = venu2(&[PART]);
    s.misplace_into = Some(GARMIN);
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::DeviceError);
    assert_eq!(
        err.message,
        "The watch placed 006-D9035-03.tmp outside GARMIN/ExtData (parent 0x01000002). A new \
         object was created as 006-D9035-03.tmp (0x01000100) and has been deleted."
    );
    assert!(fake.0.borrow().named(GARMIN, STAGED).is_empty());
    assert_eq!(
        fake.writes(),
        [
            "send_object_info 0x1000009 006-D9035-03.tmp",
            "delete_object 0x1000100"
        ]
    );
}

#[test]
fn failed_send_object_info_reports_and_removes_what_the_watch_created() {
    for (created, ending, writes) in [
        (
            true,
            "A new object was created as GARMIN/ExtData/006-D9035-03.tmp (0x01000100) and has been deleted.",
            &[
                "send_object_info 0x1000009 006-D9035-03.tmp",
                "delete_object 0x1000100",
            ][..],
        ),
        (
            false,
            "No object was created.",
            &["send_object_info 0x1000009 006-D9035-03.tmp"][..],
        ),
    ] {
        let mut s = venu2(&[PART]);
        s.send_object_info_timeout = Some(created);
        let fake = Fake::new(s);
        let err = fake.call("restore_stock", json!({}), &fonts()).unwrap_err();
        assert_eq!(
            err.message,
            format!("SendObjectInfo timed out waiting for the reply. {ending}")
        );
        assert_eq!(fake.writes(), writes);
        assert!(fake.0.borrow().named(EXTDATA, STAGED).is_empty());
    }
}

#[test]
fn undeletable_partial_file_is_reported() {
    let mut s = venu2(&[PART]);
    s.fail_send_object_at = Some(1);
    s.fail_delete = true;
    let fake = Fake::new(s);
    let err = fake.call("restore_stock", json!({}), &fonts()).unwrap_err();
    assert_eq!(
        err.message,
        "SendObject timed out waiting for the reply. A new object was created as \
         GARMIN/ExtData/006-D9035-03.tmp (0x01000100) and could not be deleted \
         (DeleteObject timed out waiting for the reply); it may hold incomplete data."
    );
}

#[test]
fn restore_stock_stages_every_original() {
    let fake = Fake::new(venu2(&ALL));
    let out = fake.call("restore_stock", json!({}), &fonts()).unwrap();
    assert_eq!(
        (out["status"].as_str(), out["variant"].as_str()),
        (Some("staged_verified"), Some("stock"))
    );
    assert_eq!(statuses(&out), pairs(&ALL.map(|p| (p, "staged_verified"))));
    for part in ALL {
        assert_eq!(staged(&fake, part)[0].1, stock(part));
    }
    let inspect = fake.call("inspect", json!({}), &fonts()).unwrap();
    for font in &inspect["fonts"].as_array().unwrap()[..4] {
        assert_eq!(font["staged"], "stock");
    }
    assert_eq!(
        fake.call("restore_stock", json!({}), &fonts()).unwrap()["status"],
        "already_staged"
    );
    // Installing over the staged stock files is refused, not replaced.
    let writes = fake.writes().len();
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(
        err.message
            .contains("006-D9035-01.tmp, GARMIN/ExtData/006-D9035-02.tmp"),
        "{}",
        err.message
    );
    assert_eq!(fake.writes().len(), writes);
}

#[test]
fn watch_without_a_pack_is_unsupported() {
    let fake = Fake::new(venu2(&["006-D1234-22"]));
    let out = fake.call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(out["supported"], false);
    assert!(targets(&out).is_empty());
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedDevice);
    assert_eq!(
        err.message,
        "Venu 2 (006-B3703-00) lists none of this build's font packs under Garmin/ExtData; \
         nothing was written."
    );
    assert_eq!(
        error_code(fake.call("restore_stock", json!({}), &fonts())),
        ErrorCode::UnsupportedDevice
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn watch_without_mtp_writes_is_unsupported() {
    let mut s = venu2(&ALL);
    s.operations.retain(|&o| o != op::SEND_OBJECT);
    let fake = Fake::new(s);
    let out = fake.call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(out["supported"], false);
    assert_eq!(pack_ids(&out), ["armenian"]);
    assert!(targets(&out).is_empty());
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedDevice);
    assert!(err.message.contains("SendObject"), "{}", err.message);
    assert!(fake.writes().is_empty());
}

#[test]
fn invalid_input_never_touches_the_watch() {
    let fake = Fake::new(venu2(&ALL));
    for (name, input) in [
        ("install", json!({"parts": []})),
        ("install", json!({"parts": [PART, PART]})),
        ("install", json!({"parts": [27]})),
        ("install", json!({"parts": PART})),
        ("install", json!({"parts": ["006-d9035-03"]})),
        ("install", json!({"parts": ["006-D9035-03.tmp"]})),
        ("install", json!({"sizes_px": [27]})),
        ("install", json!({"parts": [PART], "force": true})),
        ("restore_stock", json!([PART])),
        ("inspect", json!({"parts": [PART]})),
        ("format_watch", json!({})),
    ] {
        let err = fake.call(name, input.clone(), &fonts()).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{name} {input}");
        assert_eq!(err.exit_code(), 2);
    }
    assert_eq!(fake.0.borrow().connects, 0);
}

#[test]
fn part_missing_from_this_build_is_unavailable() {
    let fake = Fake::new(venu2(&ALL));
    let err = fake
        .call(
            "install",
            json!({"parts": [PART, "006-D1234-01"]}),
            &fonts(),
        )
        .unwrap_err();
    assert_eq!((err.code, err.exit_code()), (ErrorCode::FontUnavailable, 1));
    assert_eq!(err.message, "This build has no font part 006-D1234-01.");
    assert_eq!(fake.0.borrow().connects, 0);
}

#[test]
fn tool_definitions_follow_the_schemas() {
    let tools = ops::tools();
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["inspect", "install", "restore_stock", "inspect_drive", "issue_link"]
    );
    for pure in &tools["tools"].as_array().unwrap()[3..] {
        assert_eq!(pure["annotations"]["readOnlyHint"], true);
        assert_eq!(pure["annotations"]["destructiveHint"], false);
    }
    let install = &tools["tools"][1];
    let parts = &install["inputSchema"]["properties"]["parts"];
    assert_eq!(parts["uniqueItems"], true);
    assert_eq!(parts["minItems"], 1);
    assert_eq!(
        parts["items"]["pattern"],
        "^[0-9]{3}-[A-Z][0-9]{4}-[0-9]{2}$"
    );
    let output = &install["outputSchema"];
    assert!(output["properties"]["packs"].is_object(), "{output}");
    assert!(output["properties"].get("pack").is_none());
    assert_eq!(install["inputSchema"]["additionalProperties"], false);
    assert!(install["inputSchema"]["required"].is_null());
    assert_eq!(tools["tools"][0]["annotations"]["readOnlyHint"], true);
    assert_eq!(install["annotations"]["readOnlyHint"], false);
}

// TrueType packs: `<part>.bin` replaced in place

/// A watch listing the TrueType parts as `.bin`, with `existing` files in `ExtData` from
/// handle 0x01000050 on.
fn truetype_watch(existing: &[(&str, Vec<u8>)]) -> State {
    let mut s = venu2(&HEBREW);
    for (i, (name, data)) in existing.iter().enumerate() {
        s.file(0x0100_0050 + i as u32, EXTDATA, name, data);
    }
    s
}

fn stock_bins() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("006-D0952-08.bin", stock(HEBREW[0])),
        ("006-D0952-07.bin", stock(HEBREW[1])),
    ]
}

fn bin(fake: &Fake, part: &str) -> Vec<(u32, Vec<u8>)> {
    fake.0.borrow().named(EXTDATA, &format!("{part}.bin"))
}

/// What inspect says each TrueType part's file holds.
fn truetype_staged(fake: &Fake) -> Vec<String> {
    let out = fake.call("inspect", json!({}), &fonts_tt()).unwrap();
    out["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["kind"] == "truetype")
        .map(|f| {
            assert!(f["size_px"].is_null(), "{f}");
            f["staged"].as_str().unwrap().to_owned()
        })
        .collect()
}

#[test]
fn install_replaces_garmins_truetype_files_and_restore_puts_them_back() {
    let fake = Fake::new(truetype_watch(&stock_bins()));
    assert_eq!(truetype_staged(&fake), ["stock", "stock"]);
    let inspect = fake.call("inspect", json!({}), &fonts_tt()).unwrap();
    assert_eq!(inspect["supported"], true);
    // The hidden folder is found as the parent of Garmin's installed `.bin` fonts.
    assert_eq!(inspect["extdata"]["found_by"], "font_parent");
    assert_eq!(inspect["packs"][0]["kind"], "truetype");
    assert!(inspect["packs"][0]["parts"][0]["size_px"].is_null());

    let out = fake.call("install", json!({}), &fonts_tt()).unwrap();
    assert_eq!(out["status"], "staged_verified");
    assert_eq!(pack_ids(&out), ["hebrew"]);
    assert_eq!(out["packs"][0]["kind"], "truetype");
    assert_eq!(
        statuses(&out),
        pairs(&HEBREW.map(|p| (p, "staged_verified")))
    );
    let first = staged_parts(&out)[0];
    assert_eq!(first["path"], "GARMIN/ExtData/006-D0952-08.bin");
    assert!(first["size_px"].is_null());
    assert_eq!(first["handle"], "0x01000100");
    assert_eq!(first["bytes"], georgian(HEBREW[0]).len());
    assert_eq!(
        fake.writes(),
        [
            "delete_object 0x1000050",
            "send_object_info 0x1000009 006-D0952-08.bin",
            "send_object",
            "delete_object 0x1000051",
            "send_object_info 0x1000009 006-D0952-07.bin",
            "send_object",
        ]
    );
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0100, georgian(HEBREW[0]))]);
    assert_eq!(bin(&fake, HEBREW[1]), [(0x0100_0101, georgian(HEBREW[1]))]);
    assert_eq!(truetype_staged(&fake), ["georgian", "georgian"]);

    // Running it again changes nothing.
    let again = fake.call("install", json!({}), &fonts_tt()).unwrap();
    assert_eq!(again["status"], "already_staged");
    assert_eq!(fake.writes().len(), 6);

    let restored = fake.call("restore_stock", json!({}), &fonts_tt()).unwrap();
    assert_eq!(restored["status"], "staged_verified");
    assert_eq!(restored["variant"], "stock");
    assert_eq!(
        fake.writes()[6..8],
        [
            "delete_object 0x1000100",
            "send_object_info 0x1000009 006-D0952-08.bin"
        ]
    );
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0102, stock(HEBREW[0]))]);
    assert_eq!(bin(&fake, HEBREW[1]), [(0x0100_0103, stock(HEBREW[1]))]);
    assert_eq!(truetype_staged(&fake), ["stock", "stock"]);
    assert_eq!(
        fake.call("restore_stock", json!({}), &fonts_tt()).unwrap()["status"],
        "already_staged"
    );
    assert_eq!(fake.writes().len(), 12);
}

#[test]
fn a_truetype_file_the_watch_does_not_show_is_created() {
    let fake = Fake::new(truetype_watch(&[]));
    assert_eq!(truetype_staged(&fake), ["none", "none"]);
    let out = fake.call("install", json!({}), &fonts_tt()).unwrap();
    assert_eq!(
        statuses(&out),
        pairs(&HEBREW.map(|p| (p, "staged_verified")))
    );
    assert!(!fake.writes().iter().any(|w| w.starts_with("delete_object")));
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0100, georgian(HEBREW[0]))]);
    // Restoring after an install interrupted before its file was written creates Garmin's.
    let fake = Fake::new(truetype_watch(&[("006-D0952-07.bin", stock(HEBREW[1]))]));
    let out = fake.call("restore_stock", json!({}), &fonts_tt()).unwrap();
    assert_eq!(
        statuses(&out),
        pairs(&[
            (HEBREW[0], "staged_verified"),
            (HEBREW[1], "already_staged")
        ])
    );
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0100, stock(HEBREW[0]))]);
}

#[test]
fn an_unknown_truetype_file_is_never_replaced() {
    let mut altered = stock(HEBREW[0]);
    altered[100] ^= 1;
    let one = "GARMIN/ExtData/006-D0952-08.bin holds neither Garmin's original font nor this \
               build's Georgian one; this build replaces only those, so nothing was written.";
    for (existing, message) in [
        (vec![(BIN, altered.clone())], one.to_owned()),
        (vec![(BIN, b"newer Garmin font".to_vec())], one.to_owned()),
        // A copy that differs only in the name's case is a second file.
        (
            vec![
                (BIN, stock(HEBREW[0])),
                ("006-d0952-08.BIN", stock(HEBREW[0])),
            ],
            one.to_owned(),
        ),
        (
            vec![
                (BIN, altered.clone()),
                ("006-D0952-07.bin", altered.clone()),
            ],
            "GARMIN/ExtData/006-D0952-08.bin and GARMIN/ExtData/006-D0952-07.bin hold neither \
             Garmin's original font nor this build's Georgian one; this build replaces only \
             those, so nothing was written."
                .to_owned(),
        ),
    ] {
        for operation in ["install", "restore_stock"] {
            let fake = Fake::new(truetype_watch(&existing));
            let err = fake.call(operation, json!({}), &fonts_tt()).unwrap_err();
            assert_eq!(err.code, ErrorCode::Conflict);
            assert_eq!(err.message, message);
            assert!(fake.writes().is_empty());
        }
    }
}

#[test]
fn a_truetype_part_listed_at_another_version_or_name_is_refused() {
    for (files, listed) in [
        (
            update_file(HEBREW[0], "20", BIN),
            "006-D0952-08 as version 2.20 (006-D0952-08.bin)",
        ),
        (
            update_file(HEBREW[0], "10", "006-D0952-08.tmp"),
            "006-D0952-08 as version 2.10 (006-D0952-08.tmp)",
        ),
    ] {
        let mut s = venu2_with(device_xml_with("006-B3703-00", "1905", &files));
        s.file(0x0100_0050, EXTDATA, BIN, &stock(HEBREW[0]));
        let fake = Fake::new(s);
        let err = fake.call("install", json!({}), &fonts_tt()).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedDevice);
        assert_eq!(
            err.message,
            format!(
                "Venu 2 (006-B3703-00) lists {listed}, where this build has version 2.10 \
                 (006-D0952-08.bin). Nothing was written."
            )
        );
        assert!(fake.writes().is_empty());
    }
}

#[test]
fn a_truetype_readback_mismatch_puts_the_original_back() {
    let mut s = truetype_watch(&stock_bins());
    s.corrupt_send_at = Some(1);
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts_tt()).unwrap_err();
    assert_eq!(err.code, ErrorCode::VerificationFailed);
    let n = georgian(HEBREW[0]).len();
    assert_eq!(
        err.message,
        format!(
            "GARMIN/ExtData/006-D0952-08.bin read back as {n} bytes that differ from the {n} sent. \
             A new object was created as GARMIN/ExtData/006-D0952-08.bin (0x01000100) and has \
             been deleted. Rolled back this run's changes: GARMIN/ExtData/006-D0952-08.bin \
             written back from the original and verified (0x01000101)."
        )
    );
    assert_eq!(
        fake.writes(),
        [
            "delete_object 0x1000050",
            "send_object_info 0x1000009 006-D0952-08.bin",
            "send_object",
            "delete_object 0x1000100",
            "send_object_info 0x1000009 006-D0952-08.bin",
            "send_object",
        ]
    );
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0101, stock(HEBREW[0]))]);
    assert_eq!(
        bin(&fake, HEBREW[1]),
        [(0x0100_0051, stock(HEBREW[1]))],
        "never touched"
    );
}

#[test]
fn a_failure_on_a_later_truetype_part_puts_every_original_back() {
    let mut s = truetype_watch(&stock_bins());
    s.fail_send_object_at = Some(2);
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts_tt()).unwrap_err();
    assert_eq!(err.code, ErrorCode::DeviceError);
    assert_eq!(
        err.message,
        "SendObject timed out waiting for the reply. A new object was created as \
         GARMIN/ExtData/006-D0952-07.bin (0x01000101) and has been deleted. Rolled back this \
         run's changes: GARMIN/ExtData/006-D0952-08.bin written back from the original and \
         verified (0x01000102); GARMIN/ExtData/006-D0952-07.bin written back from the original \
         and verified (0x01000103)."
    );
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0102, stock(HEBREW[0]))]);
    assert_eq!(bin(&fake, HEBREW[1]), [(0x0100_0103, stock(HEBREW[1]))]);
    assert_eq!(truetype_staged(&fake), ["stock", "stock"]);
}

#[test]
fn rolling_back_truetype_files_the_watch_did_not_show_removes_them() {
    let mut s = truetype_watch(&[]);
    s.fail_send_object_at = Some(2);
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts_tt()).unwrap_err();
    assert!(
        err.message.ends_with(
            "Rolled back this run's changes: GARMIN/ExtData/006-D0952-08.bin removed; the watch \
             showed none before; GARMIN/ExtData/006-D0952-07.bin not left behind; the watch \
             showed none before."
        ),
        "{}",
        err.message
    );
    assert!(bin(&fake, HEBREW[0]).is_empty() && bin(&fake, HEBREW[1]).is_empty());
}

#[test]
fn a_truetype_file_that_cannot_be_deleted_stays_as_it_was() {
    let mut s = truetype_watch(&stock_bins());
    s.fail_delete = true;
    let fake = Fake::new(s);
    let err = fake.call("install", json!({}), &fonts_tt()).unwrap_err();
    assert_eq!(err.code, ErrorCode::DeviceError);
    assert_eq!(
        err.message,
        "Could not delete GARMIN/ExtData/006-D0952-08.bin (0x01000050) to replace it: \
         DeleteObject timed out waiting for the reply. Rolled back this run's changes: \
         GARMIN/ExtData/006-D0952-08.bin still holds the original."
    );
    assert_eq!(fake.writes(), ["delete_object 0x1000050"]);
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0050, stock(HEBREW[0]))]);
}

#[test]
fn a_watch_with_bitmap_parts_keeps_its_truetype_files() {
    let mut altered = stock(HEBREW[1]);
    altered[7] ^= 1;
    let mut s = venu2(&[PART, HEBREW[0], HEBREW[1]]);
    s.file(0x0100_0050, EXTDATA, BIN, &stock(HEBREW[0]));
    // An unknown file under a TrueType name isn't even a conflict: it isn't a target.
    s.file(0x0100_0051, EXTDATA, "006-D0952-07.bin", &altered);
    let fake = Fake::new(s);
    let inspect = fake.call("inspect", json!({}), &fonts_tt()).unwrap();
    assert_eq!(inspect["hosts"]["kind"], "bitmap");
    assert_eq!(inspect["hosts"]["skipped_parts"], json!(HEBREW));
    assert_eq!(pack_ids(&inspect), ["armenian"]);
    assert_eq!(targets(&inspect), [PART]);

    let out = fake.call("install", json!({}), &fonts_tt()).unwrap();
    assert_eq!(statuses(&out), pairs(&[(PART, "staged_verified")]));
    assert_eq!(
        fake.writes(),
        ["send_object_info 0x1000009 006-D9035-03.tmp", "send_object"]
    );
    assert_eq!(bin(&fake, HEBREW[0]), [(0x0100_0050, stock(HEBREW[0]))]);
    let err = fake
        .call("install", json!({"parts": [HEBREW[0]]}), &fonts_tt())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedDevice);
    assert_eq!(
        err.message,
        "Venu 2 (006-B3703-00) gets Georgian in its bitmap fonts, so this build leaves its \
         TrueType part 006-D0952-08 alone. Nothing was written."
    );
    assert_eq!(fake.writes().len(), 2);
}

/// The fenix 7 Pro Solar's font list (006-B4375-00, software 26.09, issue #11).
const FENIX_7_PRO: &[(&str, &str, &str)] = &[
    ("006-D0952-05", "2.30", "bin"),
    ("006-D0952-06", "2.30", "bin"),
    ("006-D0952-07", "2.10", "bin"),
    ("006-D0952-08", "2.10", "bin"),
    ("006-D9971-00", "2.00", "bin"),
    ("006-D9971-01", "2.10", "bin"),
    ("006-D9971-02", "2.00", "bin"),
    ("006-D9971-03", "2.00", "bin"),
    ("006-D8448-06", "2.20", "tmp"),
    ("006-D8944-01", "2.10", "tmp"),
    ("006-D8945-01", "2.30", "tmp"),
    ("006-D8947-01", "2.20", "tmp"),
    ("006-D8946-01", "2.20", "tmp"),
    ("006-D8448-07", "2.20", "tmp"),
    ("006-D8944-02", "2.10", "tmp"),
    ("006-D8945-02", "2.30", "tmp"),
    ("006-D8947-02", "2.20", "tmp"),
    ("006-D8946-02", "2.10", "tmp"),
    ("006-D9035-05", "2.00", "tmp"),
    ("006-D8448-08", "2.20", "tmp"),
    ("006-D8944-03", "2.10", "tmp"),
    ("006-D8945-03", "2.30", "tmp"),
    ("006-D8947-03", "2.20", "tmp"),
    ("006-D8946-03", "2.10", "tmp"),
    ("006-D9035-06", "2.00", "tmp"),
    ("006-D8448-11", "2.20", "tmp"),
    ("006-D8944-06", "2.10", "tmp"),
    ("006-D8945-06", "2.30", "tmp"),
    ("006-D8947-06", "2.20", "tmp"),
    ("006-D8946-06", "2.10", "tmp"),
    ("006-D9035-00", "2.10", "tmp"),
    ("006-D8448-22", "2.20", "tmp"),
    ("006-D8944-17", "2.10", "tmp"),
    ("006-D8945-17", "2.30", "tmp"),
    ("006-D8947-17", "2.20", "tmp"),
    ("006-D8946-17", "2.20", "tmp"),
    ("006-D9035-07", "2.00", "tmp"),
    ("006-D8448-13", "2.20", "tmp"),
    ("006-D8944-08", "2.10", "tmp"),
    ("006-D8945-08", "2.30", "tmp"),
    ("006-D8947-08", "2.20", "tmp"),
    ("006-D8946-08", "2.10", "tmp"),
    ("006-D9035-08", "2.00", "tmp"),
    ("006-D8448-15", "2.20", "tmp"),
    ("006-D8944-10", "2.10", "tmp"),
    ("006-D8945-10", "2.40", "tmp"),
    ("006-D8947-10", "2.20", "tmp"),
    ("006-D8946-10", "2.10", "tmp"),
    ("006-D8448-16", "2.20", "tmp"),
    ("006-D8944-11", "2.10", "tmp"),
    ("006-D8945-11", "2.30", "tmp"),
    ("006-D8947-11", "2.20", "tmp"),
    ("006-D8946-11", "2.10", "tmp"),
    ("006-D8168-09", "2.00", "tmp"),
    ("006-D8168-20", "2.10", "tmp"),
];

/// The Instinct 3 AMOLED 50mm's TrueType fonts (006-B4587-00, software 15.19, issue #8); its
/// bitmap packs are none of this build's.
const INSTINCT_3_TRUETYPE: &[(&str, &str, &str)] = &[
    ("006-D0952-05", "2.30", "bin"),
    ("006-D0952-06", "2.30", "bin"),
    ("006-D0952-07", "2.10", "bin"),
    ("006-D0952-08", "2.10", "bin"),
    ("006-D9971-00", "2.00", "bin"),
    ("006-D9971-01", "2.10", "bin"),
    ("006-D9971-02", "2.00", "bin"),
    ("006-D9971-03", "2.00", "bin"),
    ("006-D8448-23", "2.30", "tmp"),
    ("006-D8945-07", "2.20", "tmp"),
    ("006-D8947-20", "2.00", "tmp"),
    ("006-D8168-21", "2.10", "tmp"),
];

/// A watch as reported: model part, software and font list, with Garmin's stock `.bin` files
/// for this build's TrueType parts in `ExtData`.
fn reported_watch(model_part: &str, software: &str, list: &[(&str, &str, &str)]) -> State {
    let files: String = list
        .iter()
        .map(|(part, version, ext)| update_file(part, &version[2..], &format!("{part}.{ext}")))
        .collect();
    let mut s = venu2_with(device_xml_with(model_part, software, &files));
    let bundled = FontSet::bundled();
    let mut handle = 0x0100_0050;
    for (_, font) in bundled.fonts().filter(|(p, _)| p.kind == Kind::Truetype) {
        if list.iter().any(|(part, ..)| *part == font.part) {
            s.file(handle, EXTDATA, &font.file_name(), &font.stock);
            handle += 1;
        }
    }
    s
}

#[test]
fn the_fenix_7_pro_keeps_todays_bitmap_install() {
    let fake = Fake::new(reported_watch("006-B4375-00", "2609", FENIX_7_PRO));
    let fonts = FontSet::bundled();
    let inspect = fake.call("inspect", json!({}), &fonts).unwrap();
    assert_eq!(inspect["hosts"]["kind"], "bitmap");
    assert_eq!(
        inspect["hosts"]["skipped_parts"],
        json!(["006-D0952-08", "006-D0952-07"])
    );
    assert_eq!(
        inspect["hosts"]["reason"],
        "This watch lists bitmap fonts this build adds Georgian to, so Georgian goes into those, \
         as before TrueType support; its TrueType fonts 006-D0952-08 and 006-D0952-07 stay \
         Garmin's. TrueType fonts get Georgian only on watches that list none of this build's \
         bitmap fonts."
    );
    assert_eq!(
        pack_ids(&inspect),
        [
            "armenian-noto-sans-regular",
            "armenian-noto-sans-armenian-bold"
        ]
    );
    let mut expected = vec![
        "006-D9035-00",
        "006-D9035-05",
        "006-D9035-06",
        "006-D9035-07",
        "006-D9035-08",
    ];
    let mut got = targets(&inspect);
    expected.sort_unstable();
    got.sort_unstable();
    assert_eq!(got, expected);
    for font in inspect["fonts"].as_array().unwrap() {
        if font["kind"] == "truetype" && font["pack"] == "hebrew-swiss-721-truetype" {
            assert_eq!(
                (font["listed"].as_bool(), font["target"].as_bool()),
                (Some(true), Some(false))
            );
            assert_eq!(font["staged"], "stock");
        }
    }

    let out = fake.call("install", json!({}), &fonts).unwrap();
    assert!(
        staged_parts(&out)
            .iter()
            .all(|p| p["path"].as_str().unwrap().ends_with(".tmp"))
    );
    assert_eq!(staged_parts(&out).len(), 5);
    assert!(
        !fake
            .writes()
            .iter()
            .any(|w| w.contains(".bin") || w.starts_with("delete_object")),
        "{:?}",
        fake.writes()
    );
}

#[test]
fn the_instinct_3_gets_georgian_in_its_hebrew_truetype_fonts() {
    let fake = Fake::new(reported_watch("006-B4587-00", "1519", INSTINCT_3_TRUETYPE));
    let fonts = FontSet::bundled();
    let inspect = fake.call("inspect", json!({}), &fonts).unwrap();
    assert_eq!(inspect["hosts"]["kind"], "truetype");
    assert_eq!(inspect["hosts"]["skipped_parts"], json!([]));
    assert_eq!(pack_ids(&inspect), ["hebrew-swiss-721-truetype"]);
    assert_eq!(targets(&inspect), ["006-D0952-08", "006-D0952-07"]);
    let out = fake.call("install", json!({}), &fonts).unwrap();
    assert_eq!(
        statuses(&out),
        pairs(&[
            ("006-D0952-08", "staged_verified"),
            ("006-D0952-07", "staged_verified")
        ])
    );
}

#[test]
fn inspect_never_reads_files_that_cannot_be_this_builds() {
    let mut s = truetype_watch(&stock_bins());
    // A large Chinese font, an unrelated .tmp, and a file under one of this build's names but
    // of neither of its sizes.
    s.file(0x0100_0060, EXTDATA, "006-D9971-00.bin", &vec![7; 3 << 20]);
    s.file(0x0100_0061, EXTDATA, "006-D8168-12.tmp", &[1, 2, 3]);
    s.file(0x0100_0062, EXTDATA, "006-D9035-03.tmp", &[9; 10]);
    let fake = Fake::new(s);
    let inspect = fake.call("inspect", json!({}), &fonts_tt()).unwrap();
    let reads = fake.0.borrow().reads.clone();
    assert!(
        reads
            .iter()
            .all(|h| [XML, 0x0100_0050, 0x0100_0051].contains(h)),
        "{reads:x?}"
    );
    let files = inspect["extdata"]["files"].as_array().unwrap();
    let file = |name: &str| files.iter().find(|f| f["name"] == name).unwrap().clone();
    assert_eq!(
        file("006-D9971-00.bin"),
        json!({"name": "006-D9971-00.bin", "handle": "0x01000060", "bytes": 3 << 20,
               "sha256": null})
    );
    assert!(file("006-D8168-12.tmp")["sha256"].is_null());
    assert!(file("006-D9035-03.tmp")["sha256"].is_null());
    assert_eq!(
        file(BIN)["sha256"].as_str().unwrap().len(),
        64,
        "this build's file is hashed"
    );
    let staged_03 = inspect["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["part"] == PART)
        .unwrap()
        .clone();
    assert_eq!(staged_03["staged"], "other");
}

#[test]
fn install_and_restore_are_declared_destructive() {
    let tools = ops::tools();
    for (i, destructive) in [(0, false), (1, true), (2, true)] {
        assert_eq!(
            tools["tools"][i]["annotations"]["destructiveHint"],
            destructive
        );
    }
}
