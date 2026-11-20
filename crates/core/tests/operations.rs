//! The real operations driven against an in-memory MTP device modeled on the Venu 2.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use georgian_for_garmin_core::fonts::{Font, FontSet};
use georgian_for_garmin_core::mtp::{Mtp, MtpError, NewObject, Phase, TransportError};
use georgian_for_garmin_core::ops;
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use georgian_for_garmin_core::{Error, ErrorCode};
use serde_json::{Value, json};

const STORAGE: u32 = 0x0002_0001;
const OTHER_STORAGE: u32 = 0x0001_0001;
const GARMIN: u32 = 0x0100_0002;
const XML: u32 = 0x0100_0003;
const EXTDATA: u32 = 0x0100_0009;
const PART: &str = "006-D9035-03";
const STAGED: &str = "006-D9035-03.tmp";

fn device_xml(parts: &[&str]) -> Vec<u8> {
    let files: String = parts
        .iter()
        .map(|p| format!("<UpdateFile><PartNumber>{p}</PartNumber><Path>Garmin/ExtData</Path><FileName>{p}.tmp</FileName></UpdateFile>"))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">\
         <Model><PartNumber>006-B3703-00</PartNumber><SoftwareVersion>1905</SoftwareVersion><Description>Venu 2</Description></Model>\
         <MassStorageMode><UpdateFile><PartNumber>006-D8422-03</PartNumber><Path>Garmin/Text</Path><FileName>french.ln4</FileName></UpdateFile>{files}</MassStorageMode></Device>"
    )
    .into_bytes()
}

#[derive(Default)]
struct State {
    objects: BTreeMap<u32, (ObjectInfo, Vec<u8>)>,
    /// Objects the device answers `GetObjectInfo` for but omits from every listing.
    hidden: BTreeSet<u32>,
    operations: Vec<u16>,
    next_handle: u32,
    awaiting_data: Option<u32>,
    /// `SendObject` stores half the data, then times out.
    fail_send_object: bool,
    /// `SendObjectInfo` times out; `Some(true)` if the watch created the object anyway.
    send_object_info_timeout: Option<bool>,
    /// `DeleteObject` times out.
    fail_delete: bool,
    /// The device stores different bytes than it received.
    corrupt_storage: bool,
    writes: Vec<String>,
    connects: usize,
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
    s.file(XML, GARMIN, "GarminDevice.xml", &device_xml(declared));
    s.folder(0x0100_0004, GARMIN, "TEXT");
    s.folder(EXTDATA, GARMIN, "ExtData");
    s.hidden.insert(EXTDATA);
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
        let s = self.0.borrow();
        s.objects
            .get(&handle)
            .map(|(_, d)| d.clone())
            .ok_or(invalid_handle(op::GET_OBJECT))
    }

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
        let mut stored = data.to_vec();
        if s.fail_send_object {
            stored.truncate(data.len() / 2);
        } else if s.corrupt_storage {
            *stored.last_mut().unwrap() ^= 0xFF;
        }
        s.objects.get_mut(&handle).unwrap().1 = stored;
        if s.fail_send_object {
            return Err(timed_out(op::SEND_OBJECT));
        }
        Ok(())
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
}

fn georgian() -> Vec<u8> {
    (0..33_048u32).map(|i| (i * 7 % 251) as u8).collect()
}

fn stock() -> Vec<u8> {
    (0..20_720u32).map(|i| (i * 13 % 241) as u8).collect()
}

fn fonts() -> FontSet {
    FontSet {
        fonts: vec![Font {
            part: PART.into(),
            size_px: 27,
            default: true,
            stock: Cow::Owned(stock()),
            georgian: Cow::Owned(georgian()),
        }],
    }
}

fn error_code(result: Result<Value, Error>) -> ErrorCode {
    result.expect_err("operation should fail").code
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
    let mut s = venu2(&[PART]);
    s.file(0x0100_0050, EXTDATA, "006-D8945-20.tmp", b"cjk");
    let out = Fake::new(s).call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(out["extdata"]["found_by"], "font_parent");
    assert_eq!(out["extdata"]["listed_under_garmin"], false);
    assert_eq!(
        out["extdata"]["files"],
        json!([{"name": "006-D8945-20.tmp", "handle": "0x01000050", "bytes": 3,
                // SHA-256 of b"cjk"
                "sha256": "a875bed89364302fb6f5a5a783899015d4e285f54a9a9660a97654d33c0f428f"}])
    );
}

#[test]
fn finds_unlisted_empty_extdata_by_probe() {
    let out = Fake::new(venu2(&[PART]))
        .call("inspect", json!({}), &fonts())
        .unwrap();
    assert_eq!(
        out,
        json!({
            "device": {"model": "Venu 2", "part_number": "006-B3703-00", "software_version": "19.05"},
            "supported": true,
            "fonts": [{"part": PART, "size_px": 27, "default": true, "supported": true, "staged": "none"}],
            "extdata": {"handle": "0x01000009", "storage_id": "0x00020001", "listed_under_garmin": false,
                        "found_by": "handle_probe", "files": []},
        })
    );
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
fn absent_extdata_blocks_writes() {
    let mut s = venu2(&[PART]);
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

// Install and restore

#[test]
fn install_stages_and_verifies() {
    let fake = Fake::new(venu2(&[PART]));
    let out = fake.call("install", json!({}), &fonts()).unwrap();
    let staged = fake.0.borrow().named(EXTDATA, STAGED);
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].1, georgian());
    assert_eq!(out["status"], "staged_verified");
    assert_eq!(out["variant"], "georgian");
    assert_eq!(out["path"], "GARMIN/ExtData/006-D9035-03.tmp");
    assert_eq!(out["handle"], format!("{:#010x}", staged[0].0));
    assert_eq!(out["bytes"], 33_048);
    assert_eq!(out["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(
        fake.writes(),
        ["send_object_info 0x1000009 006-D9035-03.tmp", "send_object"]
    );

    let after = fake.call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(after["extdata"]["found_by"], "font_parent");
    assert_eq!(after["fonts"][0]["staged"], "georgian");
}

#[test]
fn install_of_identical_file_writes_nothing() {
    let mut s = venu2(&[PART]);
    s.file(0x0100_0050, EXTDATA, STAGED, &georgian());
    let fake = Fake::new(s);
    let out = fake
        .call("install", json!({"size_px": 27}), &fonts())
        .unwrap();
    assert_eq!(out["status"], "already_staged");
    assert_eq!(out["handle"], "0x01000050");
    assert!(fake.writes().is_empty());
}

#[test]
fn install_over_different_file_is_a_conflict() {
    for existing in [stock(), {
        let mut g = georgian();
        g[100] ^= 1;
        g
    }] {
        let mut s = venu2(&[PART]);
        s.file(0x0100_0050, EXTDATA, STAGED, &existing);
        let fake = Fake::new(s);
        assert_eq!(
            error_code(fake.call("install", json!({}), &fonts())),
            ErrorCode::Conflict
        );
        assert!(fake.writes().is_empty());
        assert_eq!(
            fake.0.borrow().named(EXTDATA, STAGED),
            [(0x0100_0050, existing)]
        );
    }
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
    s.fail_send_object = true;
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
    s.fail_send_object = true;
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
fn restore_stock_stages_the_original() {
    let fake = Fake::new(venu2(&[PART]));
    let out = fake.call("restore_stock", json!({}), &fonts()).unwrap();
    assert_eq!(
        (out["status"].as_str(), out["variant"].as_str()),
        (Some("staged_verified"), Some("stock"))
    );
    assert_eq!(fake.0.borrow().named(EXTDATA, STAGED)[0].1, stock());
    assert_eq!(
        fake.call("inspect", json!({}), &fonts()).unwrap()["fonts"][0]["staged"],
        "stock"
    );
    // Installing over the staged stock file is refused, not replaced.
    assert_eq!(
        error_code(fake.call("install", json!({}), &fonts())),
        ErrorCode::Conflict
    );
}

#[test]
fn watch_without_the_part_is_unsupported() {
    let fake = Fake::new(venu2(&["006-D8947-22"]));
    let out = fake.call("inspect", json!({}), &fonts()).unwrap();
    assert_eq!(out["supported"], false);
    assert_eq!(out["fonts"][0]["supported"], false);
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedDevice);
    assert!(
        err.message.contains("Venu 2") && err.message.contains(PART),
        "{}",
        err.message
    );
    assert_eq!(
        error_code(fake.call("restore_stock", json!({}), &fonts())),
        ErrorCode::UnsupportedDevice
    );
    assert!(fake.writes().is_empty());
}

#[test]
fn watch_without_mtp_writes_is_unsupported() {
    let mut s = venu2(&[PART]);
    s.operations.retain(|&o| o != op::SEND_OBJECT);
    let fake = Fake::new(s);
    assert_eq!(
        fake.call("inspect", json!({}), &fonts()).unwrap()["supported"],
        false
    );
    let err = fake.call("install", json!({}), &fonts()).unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedDevice);
    assert!(err.message.contains("SendObject"), "{}", err.message);
    assert!(fake.writes().is_empty());
}

#[test]
fn invalid_input_never_touches_the_watch() {
    let fake = Fake::new(venu2(&[PART]));
    for (name, input) in [
        ("install", json!({"size_px": 26})),
        ("install", json!({"size_px": "27"})),
        ("install", json!({"size_px": 27, "force": true})),
        ("restore_stock", json!([27])),
        ("inspect", json!({"size_px": 27})),
        ("format_watch", json!({})),
    ] {
        let err = fake.call(name, input.clone(), &fonts()).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{name} {input}");
        assert_eq!(err.exit_code(), 2);
    }
    assert_eq!(fake.0.borrow().connects, 0);
}

#[test]
fn size_missing_from_this_build_is_unavailable() {
    let fake = Fake::new(venu2(&[PART, "006-D9035-02"]));
    let err = fake
        .call("install", json!({"size_px": 24}), &fonts())
        .unwrap_err();
    assert_eq!((err.code, err.exit_code()), (ErrorCode::FontUnavailable, 1));
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
    assert_eq!(names, ["inspect", "install", "restore_stock"]);
    let install = &tools["tools"][1];
    assert_eq!(
        install["inputSchema"]["properties"]["size_px"]["enum"],
        json!([21, 24, 27, 31])
    );
    assert_eq!(
        install["inputSchema"]["properties"]["size_px"]["default"],
        27
    );
    assert_eq!(install["inputSchema"]["additionalProperties"], false);
    assert_eq!(tools["tools"][0]["annotations"]["readOnlyHint"], true);
    assert_eq!(install["annotations"]["readOnlyHint"], false);
}
