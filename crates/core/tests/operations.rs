//! The real operations driven against an in-memory MTP device modeled on the Venu 2.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use georgian_for_garmin_core::fonts::{Font, FontSet, Pack, Tested};
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

fn device_xml_for(model_part: &str, software: &str, parts: &[&str]) -> Vec<u8> {
    let files: String = parts
        .iter()
        .map(|p| format!("<UpdateFile><PartNumber>{p}</PartNumber><Version><Major>2</Major><Minor>10</Minor></Version><Path>Garmin/ExtData</Path><FileName>{p}.tmp</FileName></UpdateFile>"))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">\
         <Model><PartNumber>{model_part}</PartNumber><SoftwareVersion>{software}</SoftwareVersion><Description>Venu 2</Description></Model>\
         <MassStorageMode><UpdateFile><PartNumber>006-D8422-03</PartNumber><Path>Garmin/Text</Path><FileName>french.ln4</FileName></UpdateFile>{files}</MassStorageMode></Device>"
    )
    .into_bytes()
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
        s.sends += 1;
        let fail = s.fail_send_object_at == Some(s.sends);
        let mut stored = data.to_vec();
        if fail {
            stored.truncate(data.len() / 2);
        } else if s.corrupt_storage {
            *stored.last_mut().unwrap() ^= 0xFF;
        }
        s.objects.get_mut(&handle).unwrap().1 = stored;
        if fail {
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
        fonts: parts
            .iter()
            .map(|&(part, size_px)| Font {
                part: part.into(),
                size_px,
                stock: Cow::Owned(stock(part)),
                georgian: Cow::Owned(georgian(part)),
            })
            .collect(),
        tested,
    }
}

/// The test registry: Armenian (verified on a Venu 2 at 19.05, 27 px), then Thai (untested).
fn fonts() -> FontSet {
    FontSet {
        packs: vec![
            pack(
                "armenian",
                &ARMENIAN,
                vec![Tested {
                    model: "Venu 2".into(),
                    model_part: "006-B3703-00".into(),
                    software: "19.05".into(),
                    parts: vec![PART.into()],
                }],
            ),
            pack("thai", &THAI, vec![]),
        ],
    }
}

fn error_code(result: Result<Value, Error>) -> ErrorCode {
    result.expect_err("operation should fail").code
}

fn staged(fake: &Fake, part: &str) -> Vec<(u32, Vec<u8>)> {
    fake.0.borrow().named(EXTDATA, &format!("{part}.tmp"))
}

fn statuses(out: &Value) -> Vec<(String, String)> {
    out["parts"]
        .as_array()
        .unwrap()
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
        json!([{"name": "006-D1234-20.tmp", "handle": "0x01000050", "bytes": 3,
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
        json!({"pack": "armenian", "part": PART, "size_px": 27, "listed": true, "target": true,
               "staged": "none"})
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

// Pack selection

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

#[test]
fn one_listed_pack_is_selected_with_every_size() {
    let out = selected(&ALL);
    assert_eq!(out["pack"]["id"], "armenian");
    assert_eq!(out["pack"]["name"], "armenian pack");
    assert_eq!(out["pack"]["parts"], json!(ALL));
    assert_eq!(out["pack"]["sizes_px"], json!([21, 24, 27, 31]));
    assert_eq!(targets(&out), ALL);
    assert_eq!(out["supported"], true);
}

#[test]
fn of_two_listed_packs_the_first_in_the_registry_wins() {
    // The watch lists Thai first; registry order decides.
    let out = selected(&[THAI[0].0, THAI[1].0, ARMENIAN[1].0]);
    assert_eq!(out["pack"]["id"], "armenian");
    assert_eq!(out["pack"]["parts"], json!([ARMENIAN[1].0]));
    assert_eq!(targets(&out), [ARMENIAN[1].0]);
    let listed: Vec<&str> = out["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["listed"] == true)
        .map(|f| f["part"].as_str().unwrap())
        .collect();
    assert_eq!(listed, [ARMENIAN[1].0, THAI[0].0, THAI[1].0]);

    let out = selected(&[THAI[1].0]);
    assert_eq!(out["pack"]["id"], "thai");
    assert_eq!(out["pack"]["sizes_px"], json!([27]));
}

#[test]
fn a_watch_listing_no_pack_has_none_selected() {
    let out = selected(&["006-D1234-01"]);
    assert!(out["pack"].is_null());
    assert_eq!(out["supported"], false);
    assert!(targets(&out).is_empty());
    assert_eq!(out["report"]["pack"], Value::Null);
    assert_eq!(out["report"]["tested"], "untested");
}

#[test]
fn a_partial_list_targets_only_the_listed_sizes() {
    let out = selected(&[ARMENIAN[0].0, ARMENIAN[2].0]);
    assert_eq!(out["pack"]["parts"], json!([ARMENIAN[0].0, ARMENIAN[2].0]));
    assert_eq!(out["pack"]["sizes_px"], json!([21, 27]));
    assert_eq!(targets(&out), [ARMENIAN[0].0, ARMENIAN[2].0]);
}

// Tested status

fn tested(model_part: &str, software: &str) -> Value {
    Fake::new(venu2_with(device_xml_for(model_part, software, &ALL)))
        .call("inspect", json!({}), &fonts())
        .unwrap()
}

#[test]
fn same_model_and_software_is_tested() {
    let out = tested("006-B3703-00", "1905");
    assert_eq!(
        out["pack"]["tested"],
        json!({"status": "tested", "verified_parts": [PART], "verified_software": ["19.05"]})
    );
    assert_eq!(out["report"]["tested"], "tested");
}

#[test]
fn same_model_on_other_software_is_tested_other_software() {
    let out = tested("006-B3703-00", "2026");
    assert_eq!(out["device"]["software_version"], "20.26");
    assert_eq!(
        out["pack"]["tested"],
        json!({"status": "tested_other_software", "verified_parts": [PART],
               "verified_software": ["19.05"]})
    );
}

#[test]
fn another_model_is_untested() {
    let out = tested("006-B4260-00", "1905");
    assert_eq!(
        out["pack"]["tested"],
        json!({"status": "untested", "verified_parts": [], "verified_software": []})
    );
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
    assert_eq!(report["pack"], "armenian");
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
    let text = report["text"].as_str().unwrap();
    for line in [
        "- Model: Venu 2",
        "- Model part number: 006-B3703-00",
        "- Software version: 19.05",
        &format!("- Georgian for Garmin: {}", env!("CARGO_PKG_VERSION")),
        "- Font pack: armenian (tested on this model and software)",
        "| 006-D9035-03 | 2.10 | 006-D9035-03.tmp |",
        "| 006-D8945-20 | 3.05 | 006-D8945-20.tmp |",
        "| 006-D7777-01 | (none) | x\\|y.tmp |",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "{line:?} missing from\n{text}"
        );
    }
    assert!(!text.contains("GUPDATE"), "only ExtData files are listed");
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
    assert_eq!(out["pack"], "armenian");
    assert_eq!(statuses(&out), pairs(&ALL.map(|p| (p, "staged_verified"))));
    let mut expected_writes = Vec::new();
    for (i, (part, size)) in ARMENIAN.iter().enumerate() {
        let files = staged(&fake, part);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, georgian(part));
        let reported = &out["parts"][i];
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
    assert_eq!(out["parts"][2]["handle"], "0x01000050");
    assert!(!fake.writes().iter().any(|w| w.contains(STAGED)));
    assert_eq!(fake.writes().len(), 6);
}

#[test]
fn install_of_identical_file_writes_nothing() {
    let mut s = venu2(&[PART]);
    s.file(0x0100_0050, EXTDATA, STAGED, &georgian(PART));
    let fake = Fake::new(s);
    let out = fake
        .call("install", json!({"sizes_px": [27]}), &fonts())
        .unwrap();
    assert_eq!(out["status"], "already_staged");
    assert_eq!(out["parts"][0]["handle"], "0x01000050");
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
            "A different file is already staged as GARMIN/ExtData/006-D9035-04.tmp; nothing was written."
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

#[test]
fn sizes_px_restricts_the_parts() {
    let fake = Fake::new(venu2(&ALL));
    let out = fake
        .call("install", json!({"sizes_px": [31, 24]}), &fonts())
        .unwrap();
    assert_eq!(
        statuses(&out),
        pairs(&[(ALL[1], "staged_verified"), (ALL[3], "staged_verified")])
    );
    assert_eq!(fake.writes().len(), 4);
}

#[test]
fn a_size_the_watch_does_not_list_is_unsupported() {
    let fake = Fake::new(venu2(&[PART]));
    let err = fake
        .call("install", json!({"sizes_px": [24, 27]}), &fonts())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedDevice);
    assert!(err.message.contains("24 px"), "{}", err.message);
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
    assert_eq!(out["pack"]["id"], "armenian");
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
        ("install", json!({"sizes_px": []})),
        ("install", json!({"sizes_px": [27, 27]})),
        ("install", json!({"sizes_px": ["27"]})),
        ("install", json!({"sizes_px": 27})),
        ("install", json!({"sizes_px": [-1]})),
        ("install", json!({"size_px": 27})),
        ("install", json!({"sizes_px": [27], "force": true})),
        ("restore_stock", json!([27])),
        ("inspect", json!({"sizes_px": [27]})),
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
    let fake = Fake::new(venu2(&ALL));
    let err = fake
        .call("install", json!({"sizes_px": [27, 26]}), &fonts())
        .unwrap_err();
    assert_eq!((err.code, err.exit_code()), (ErrorCode::FontUnavailable, 1));
    assert_eq!(err.message, "This build has no 26 px font.");
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
    let sizes = &install["inputSchema"]["properties"]["sizes_px"];
    assert_eq!(sizes["uniqueItems"], true);
    assert_eq!(sizes["minItems"], 1);
    assert_eq!(install["inputSchema"]["additionalProperties"], false);
    assert!(install["inputSchema"]["required"].is_null());
    assert_eq!(tools["tools"][0]["annotations"]["readOnlyHint"], true);
    assert_eq!(install["annotations"]["readOnlyHint"], false);
}
