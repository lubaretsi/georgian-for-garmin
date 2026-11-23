//! The raw MTP operations (cargo feature `dev`): each read primitive, `ptp_transaction`'s read
//! allowlist, the checks of `mtp_create` and `mtp_delete`, and the live trace over `rpc`,
//! against the fake Venu 2.
#![cfg(feature = "dev")]

#[path = "support/fake.rs"]
mod fake;

use std::borrow::Cow;
use std::io::{BufReader, PipeWriter, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use fake::{
    EXTDATA, GARMIN, IMPORTED_ARMENIAN, IMPORTED_CJK, OWNER_NAME, SERIAL, STORAGE, UNIT_ID, Watch,
    XML,
};
use georgian_for_garmin_core::fonts::sha256_hex;
use georgian_for_garmin_core::session::{Control, Session, Worker};
use georgian_for_garmin_core::{Error, ErrorCode, rpc};
use serde_json::{Value, json};

const ROOT: u32 = 0xFFFF_FFFF;

fn temp_path(name: &str) -> std::path::PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "gfg-raw-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// A session on `watch` logging to a fresh `staged.jsonl`.
fn session(watch: &Watch) -> (Session<'static>, std::path::PathBuf) {
    let log = temp_path("staged.jsonl");
    let w = watch.clone();
    let session = Session::new(Cow::Owned(fake::fonts()), Box::new(move || w.connect()))
        .with_poll_interval(Duration::from_millis(10))
        .with_staged_log(log.clone());
    (session, log)
}

fn call(s: &mut Session<'_>, name: &str, input: Value) -> Result<Value, Error> {
    s.call(name, input, &Control::NONE)
}

fn refused(s: &mut Session<'_>, name: &str, input: Value) -> Error {
    call(s, name, input).expect_err("refused")
}

fn base64(out: &Value) -> Vec<u8> {
    STANDARD
        .decode(out["base64"].as_str().expect("inline data"))
        .unwrap()
}

fn log_lines(log: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn u32s(bytes: &[u8]) -> Vec<u32> {
    bytes[4..]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn reads_mirror_the_mtp_primitives() {
    let watch = Watch::venu2();
    let (mut s, _) = session(&watch);
    let info = call(&mut s, "mtp_device_info", json!({})).unwrap();
    assert_eq!(info["model"], "Venu 2");
    assert!(
        info["operations_supported"]
            .as_array()
            .unwrap()
            .contains(&json!(0x9002))
    );
    let ids = call(&mut s, "mtp_storage_ids", json!({})).unwrap();
    assert_eq!(ids["storage_ids"], json!([STORAGE]));
    let storage = call(&mut s, "mtp_storage_info", json!({"storage": STORAGE})).unwrap();
    assert_eq!(storage["storage_description"], "Internal Storage");
    assert_eq!(storage["free_space_in_bytes"], 4u64 << 30);

    let handles = |s: &mut Session<'_>, parent: u32, hidden: bool| {
        let input = json!({"storage": STORAGE, "parent": parent, "hidden": hidden});
        call(s, "mtp_handles", input).unwrap()["handles"].clone()
    };
    assert_eq!(handles(&mut s, ROOT, false), json!([GARMIN]));
    // ExtData and its imports are hidden from GetObjectHandles; Garmin's GetObjectIds lists them.
    assert_eq!(handles(&mut s, GARMIN, false), json!([XML]));
    assert_eq!(handles(&mut s, EXTDATA, false), json!([]));
    assert_eq!(
        handles(&mut s, EXTDATA, true),
        json!([IMPORTED_ARMENIAN, IMPORTED_CJK])
    );

    let object = call(
        &mut s,
        "mtp_object_info",
        json!({"handle": IMPORTED_ARMENIAN}),
    )
    .unwrap();
    assert_eq!(object["filename"], "006-D9035-03.fnt");
    assert_eq!(object["parent"], EXTDATA);
    assert_eq!(object["read_only"], true);

    let stock = fake::stock("006-D9035-03");
    let whole = call(&mut s, "mtp_read", json!({"handle": IMPORTED_ARMENIAN})).unwrap();
    assert_eq!(base64(&whole["data"]), stock);
    assert_eq!(whole["data"]["sha256"], sha256_hex(&stock));
    assert_eq!(whole["data"]["bytes"], stock.len());
    let input = json!({"handle": IMPORTED_ARMENIAN, "offset": 10, "length": 5});
    let range = call(&mut s, "mtp_read", input).unwrap();
    assert_eq!(
        (range["offset"].clone(), base64(&range["data"])),
        (json!(10), stock[10..15].to_vec())
    );
    let input = json!({"handle": IMPORTED_ARMENIAN, "offset": stock.len() - 3});
    assert_eq!(
        base64(&call(&mut s, "mtp_read", input).unwrap()["data"]),
        &stock[stock.len() - 3..]
    );

    let path = temp_path("object.fnt");
    let input = json!({"handle": IMPORTED_ARMENIAN, "path": path});
    let written = call(&mut s, "mtp_read", input.clone()).unwrap();
    assert_eq!(written["data"]["base64"], Value::Null);
    assert_eq!(std::fs::read(&path).unwrap(), stock);
    // An existing local file is never overwritten.
    assert_eq!(
        refused(&mut s, "mtp_read", input).code,
        ErrorCode::InvalidInput
    );
    let input = json!({"handle": IMPORTED_ARMENIAN, "max_bytes": 100});
    let error = refused(&mut s, "mtp_read", input);
    assert!(error.message.contains("max_bytes"), "{}", error.message);
    let error = refused(&mut s, "mtp_read", json!({"handle": 0x0DEAD}));
    assert_eq!(error.code, ErrorCode::DeviceError);
    assert!(
        error.message.contains("InvalidObjectHandle"),
        "{}",
        error.message
    );
}

#[test]
fn ptp_transaction_sends_only_reads() {
    let watch = Watch::venu2();
    let (mut s, _) = session(&watch);
    let input = json!({"opcode": 0x1007, "params": [STORAGE, 0, ROOT]});
    let out = call(&mut s, "ptp_transaction", input).unwrap();
    assert_eq!(out["operation"], "GetObjectHandles");
    assert_eq!(
        (out["response_code"].clone(), out["response"].clone()),
        (json!(0x2001), json!("OK"))
    );
    assert_eq!(u32s(&base64(&out["data"])), [GARMIN]);

    let input = json!({"opcode": 0x9002, "params": [STORAGE, EXTDATA]});
    let out = call(&mut s, "ptp_transaction", input).unwrap();
    assert_eq!(
        u32s(&base64(&out["data"])),
        [IMPORTED_ARMENIAN, IMPORTED_CJK]
    );
    let input = json!({"opcode": 0x1006, "params": [STORAGE, 0, GARMIN]});
    let out = call(&mut s, "ptp_transaction", input).unwrap();
    assert_eq!(out["response_params"], json!([1]));
    assert_eq!(out["data"]["bytes"], 0);
    // A response other than OK is an answer, not an error.
    let input = json!({"opcode": 0x1008, "params": [0x0DEAD]});
    let out = call(&mut s, "ptp_transaction", input).unwrap();
    assert_eq!(out["response_code"], 0x2009);
    assert_eq!(out["response"], "InvalidObjectHandle");

    let sent = watch.state().log.len();
    for (opcode, says) in [
        (0x100C, "mtp_create"),     // SendObjectInfo
        (0x100D, "mtp_create"),     // SendObject
        (0x100B, "mtp_delete"),     // DeleteObject
        (0x1019, "mtp_create"),     // MoveObject
        (0x9804, "mtp_create"),     // SetObjectPropValue
        (0x1016, "mtp_create"),     // SetDevicePropValue
        (0x9999, "unknown vendor"), // an unknown vendor operation
        (0x9003, "unknown vendor"), // a Garmin code next to the allowed one
        (0x1002, "session"),        // OpenSession
        (0x1003, "session"),        // CloseSession
        (0x100F, "erases"),         // FormatStore
        (0x1010, "resets"),         // ResetDevice
        (0x1013, "switches off"),   // PowerDown
    ] {
        let error = refused(&mut s, "ptp_transaction", json!({"opcode": opcode}));
        assert_eq!(error.code, ErrorCode::InvalidInput, "{opcode:#06x}");
        assert!(
            error.message.contains(says),
            "{opcode:#06x}: {}",
            error.message
        );
        assert!(
            error.message.contains("Nothing was sent"),
            "{}",
            error.message
        );
    }
    // No data-out phase, no write flag, at most five parameters.
    for input in [
        json!({"opcode": 0x1001, "data_out": "AAAA"}),
        json!({"opcode": 0x1001, "allow_write": true}),
        json!({"opcode": 0x1007, "params": [1, 2, 3, 4, 5, 6]}),
        json!({"opcode": 70000}),
    ] {
        let error = refused(&mut s, "ptp_transaction", input.clone());
        assert_eq!(error.code, ErrorCode::InvalidInput, "{input}");
    }
    assert_eq!(watch.state().log.len(), sent, "nothing reached the watch");
}

#[test]
fn mtp_create_writes_only_over_identified_files_in_extdata() {
    let watch = Watch::venu2();
    let (mut s, log) = session(&watch);
    let data = STANDARD.encode(b"experimental host");
    let create = |name: &str| json!({"name": name, "data_base64": data, "allow_write": true});

    let error = refused(
        &mut s,
        "mtp_create",
        json!({"name": "006-D9035-03.tmp", "data_base64": data}),
    );
    assert_eq!(error.code, ErrorCode::InvalidInput);
    assert!(error.message.contains("allow_write"), "{}", error.message);
    assert_eq!(
        watch.state().connects,
        0,
        "refused before the watch was opened"
    );
    let input = json!({"name": "006-D9035-03.tmp", "data_base64": data, "allow_write": false});
    assert_eq!(
        refused(&mut s, "mtp_create", input).code,
        ErrorCode::InvalidInput
    );

    let mut expect = |input: Value, code: ErrorCode, says: &str| {
        let error = refused(&mut s, "mtp_create", input);
        assert_eq!(error.code, code, "{}", error.message);
        assert!(error.message.contains(says), "{}", error.message);
    };
    // Not a part's file, or a part the watch doesn't declare.
    expect(
        create("notes.txt"),
        ErrorCode::InvalidInput,
        "not a part's file",
    );
    expect(
        create("D999999.tmp"),
        ErrorCode::UnsupportedDevice,
        "doesn't declare",
    );
    // Outside ExtData.
    let mut outside = create("006-D9035-03.tmp");
    outside["parent"] = json!(GARMIN);
    expect(outside, ErrorCode::InvalidInput, "only into GARMIN/ExtData");
    let mut outside = create("006-D9035-03.tmp");
    outside["storage"] = json!(STORAGE + 1);
    expect(outside, ErrorCode::InvalidInput, "only into GARMIN/ExtData");
    // The imported CJK font is no file this build or tool knows.
    expect(
        create("D844870.tmp"),
        ErrorCode::Conflict,
        "neither Garmin's original",
    );
    // Nor is an unknown .fnt of the 21 px part in the normal listing.
    watch.add_file("006-D9035-01.fnt", b"mystery".to_vec());
    expect(
        create("006-D9035-01.tmp"),
        ErrorCode::Conflict,
        "neither Garmin's original",
    );
    // A waiting file of the part is there already, in either name form.
    watch.add_file("006-D9035-02.tmp", fake::georgian("006-D9035-02"));
    expect(
        create("D903502.tmp"),
        ErrorCode::Conflict,
        "delete it first",
    );
    // Neither bytes nor both.
    expect(
        json!({"name": "006-D9035-03.tmp", "allow_write": true}),
        ErrorCode::InvalidInput,
        "exactly one",
    );
    expect(
        json!({"name": "006-D9035-03.tmp", "data_base64": "%%", "allow_write": true}),
        ErrorCode::InvalidInput,
        "base64",
    );
    assert!(
        !watch.state().log.iter().any(|o| o.starts_with("Send")),
        "no refused call wrote anything"
    );
    assert!(log_lines(&log).is_empty());

    // The 27 px part's hidden import is Garmin's (this build's stock font): allowed.
    let out = call(&mut s, "mtp_create", create("006-D9035-03.tmp")).unwrap();
    let handle = out["handle"].as_u64().unwrap() as u32;
    assert_eq!(out["parent"], EXTDATA);
    assert_eq!(out["sha256"], sha256_hex(b"experimental host"));
    assert!(
        watch
            .listed()
            .contains(&("006-D9035-03.tmp".to_owned(), b"experimental host".to_vec()))
    );
    let id = out["id"].as_str().unwrap();
    let lines = log_lines(&log);
    assert_eq!(lines[0]["event"], "staged");
    assert_eq!(lines[0]["op"], "mtp_create");
    assert_eq!(lines[0]["id"], id);
    assert_eq!(lines[1]["event"], "created");
    assert_eq!(lines[1]["handle"], handle);
    // A local source works as well; the file it made is one this tool created.
    let source = temp_path("host.bin");
    std::fs::write(&source, b"TrueType experiment").unwrap();
    let input = json!({"name": "006-D9035-03.bin", "source": source, "allow_write": true});
    let bin = call(&mut s, "mtp_create", input).unwrap();
    // remove_staged takes either id back, whatever kind of file it is.
    let out = call(&mut s, "remove_staged", json!({"id": bin["id"]})).unwrap();
    assert_eq!(out["status"], "removed");
    let out = call(&mut s, "remove_staged", json!({"id": id})).unwrap();
    assert_eq!(out["status"], "removed");
}

#[test]
fn mtp_delete_deletes_only_identified_files_in_extdata() {
    let watch = Watch::venu2();
    let (mut s, log) = session(&watch);
    let delete = |handle: u32| json!({"handle": handle, "allow_write": true});
    let georgian = watch.add_file("006-D9035-02.tmp", fake::georgian("006-D9035-02"));

    let error = refused(&mut s, "mtp_delete", json!({"handle": georgian}));
    assert_eq!(error.code, ErrorCode::InvalidInput);
    assert!(error.message.contains("allow_write"), "{}", error.message);
    assert_eq!(watch.state().connects, 0);

    let mystery = watch.add_file("006-D9035-01.tmp", b"mystery".to_vec());
    let notes = watch.add_file("notes.txt", b"notes".to_vec());
    for (handle, code, says) in [
        (XML, ErrorCode::InvalidInput, "not a file in GARMIN/ExtData"),
        (
            GARMIN,
            ErrorCode::InvalidInput,
            "not a file in GARMIN/ExtData",
        ),
        (
            IMPORTED_ARMENIAN,
            ErrorCode::InvalidInput,
            "hidden or read-only import",
        ),
        (
            IMPORTED_CJK,
            ErrorCode::InvalidInput,
            "hidden or read-only import",
        ),
        (mystery, ErrorCode::Conflict, "not identified"),
        (notes, ErrorCode::Conflict, "not identified"),
    ] {
        let error = refused(&mut s, "mtp_delete", delete(handle));
        assert_eq!(error.code, code, "{handle:#x}: {}", error.message);
        assert!(error.message.contains(says), "{}", error.message);
        assert!(
            error.message.contains("Nothing was deleted"),
            "{}",
            error.message
        );
    }
    assert!(!watch.state().log.iter().any(|o| o == "DeleteObject"));
    assert!(log_lines(&log).is_empty());

    // One of this program's Georgian builds.
    let out = call(&mut s, "mtp_delete", delete(georgian)).unwrap();
    assert_eq!(out["identified_as"], "georgian");
    assert_eq!(out["name"], "006-D9035-02.tmp");
    let deleted = log_lines(&log);
    assert_eq!(deleted[0]["event"], "deleted");
    assert_eq!(
        deleted[0]["sha256"],
        sha256_hex(&fake::georgian("006-D9035-02"))
    );
    // A delete can't be undone.
    let error = refused(&mut s, "remove_staged", json!({"id": out["id"]}));
    assert!(
        error.message.contains("can't be undone"),
        "{}",
        error.message
    );

    // A file mtp_create made; its id is then already removed.
    let input = json!({"name": "006-D9035-04.tmp", "data_base64": STANDARD.encode(b"x"),
                       "allow_write": true});
    let created = call(&mut s, "mtp_create", input).unwrap();
    let handle = created["handle"].as_u64().unwrap() as u32;
    let out = call(&mut s, "mtp_delete", delete(handle)).unwrap();
    assert_eq!(out["identified_as"], "staged");
    let out = call(&mut s, "remove_staged", json!({"id": created["id"]})).unwrap();
    assert_eq!(out["status"], "already_removed");
    let mut listed: Vec<String> = watch.listed().into_iter().map(|f| f.0).collect();
    listed.sort();
    assert_eq!(listed, ["006-D9035-01.tmp", "notes.txt"]);
}

/// `bytes` with every byte of `UNIT_ID` replaced by `*`: what masking must leave.
fn masked_xml(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let id = format!("<Id>{UNIT_ID}</Id>");
    assert_eq!(
        text.matches(&id).count(),
        1,
        "the fake's GarminDevice.xml holds the unit ID"
    );
    text.replace(&id, &format!("<Id>{}</Id>", "*".repeat(UNIT_ID.len())))
        .into_bytes()
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

/// A PTP string's units as bytes, to look for it in raw data.
fn ucs2(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn identifiers_are_masked_unless_revealed() {
    let watch = Watch::venu2();
    let (mut s, log) = session(&watch);
    let reveal = |mut input: Value| {
        input["reveal_identifiers"] = json!(true);
        input
    };

    // mtp_device_info: the serial number.
    let info = call(&mut s, "mtp_device_info", json!({})).unwrap();
    assert_eq!(info["serial_number"], "<redacted>");
    assert_eq!(info["model"], "Venu 2");
    let info = call(&mut s, "mtp_device_info", reveal(json!({}))).unwrap();
    assert_eq!(info["serial_number"], SERIAL);

    // A raw GetDeviceInfo: the serial masked in place; the dataset still parses.
    let out = call(&mut s, "ptp_transaction", json!({"opcode": 0x1001})).unwrap();
    let masked = base64(&out["data"]);
    let revealed = call(&mut s, "ptp_transaction", reveal(json!({"opcode": 0x1001}))).unwrap();
    let revealed = base64(&revealed["data"]);
    assert_eq!(masked.len(), revealed.len());
    let parsed = georgian_for_garmin_core::ptp::DeviceInfo::decode(&masked).unwrap();
    assert_eq!(parsed.serial_number, "*".repeat(SERIAL.len()));
    assert_eq!(parsed.model, "Venu 2");
    let parsed = georgian_for_garmin_core::ptp::DeviceInfo::decode(&revealed).unwrap();
    assert_eq!(parsed.serial_number, SERIAL);
    let serial = ucs2(SERIAL);
    assert!(!masked.windows(serial.len()).any(|w| w == serial));
    assert_eq!(
        out["data"]["sha256"],
        sha256_hex(&masked),
        "the hash is of what's returned"
    );

    // DeviceFriendlyName (0xD402), its value and its description.
    for opcode in [0x1015, 0x1014] {
        let input = json!({"opcode": opcode, "params": [0xD402]});
        let masked = base64(&call(&mut s, "ptp_transaction", input.clone()).unwrap()["data"]);
        let revealed = base64(&call(&mut s, "ptp_transaction", reveal(input)).unwrap()["data"]);
        assert_eq!(masked.len(), revealed.len(), "{opcode:#x}");
        let name = ucs2(OWNER_NAME);
        assert!(
            revealed.windows(name.len()).any(|w| w == name),
            "{opcode:#x}"
        );
        assert!(
            !masked.windows(name.len()).any(|w| w == name),
            "{opcode:#x}"
        );
        assert!(
            masked
                .windows(2 * OWNER_NAME.len())
                .any(|w| w == ucs2(&"*".repeat(OWNER_NAME.len())))
        );
    }

    // GarminDevice.xml through mtp_read: whole, and a range cutting through the Id.
    let raw = base64(&call(&mut s, "mtp_read", reveal(json!({"handle": XML}))).unwrap()["data"]);
    assert!(contains(&raw, UNIT_ID));
    let expected = masked_xml(&raw);
    let whole = base64(&call(&mut s, "mtp_read", json!({"handle": XML})).unwrap()["data"]);
    assert_eq!(whole, expected, "only the Id changes, byte for byte");
    let start = raw.windows(4).position(|w| w == b"<Id>").unwrap() + 4 + 3;
    let input = json!({"handle": XML, "offset": start, "length": 12});
    let range = base64(&call(&mut s, "mtp_read", input).unwrap()["data"]);
    assert_eq!(range, &expected[start..start + 12]);
    assert!(
        range.starts_with(b"*******</Id>"),
        "{}",
        String::from_utf8_lossy(&range)
    );

    // A raw GetObject and GetPartialObject of it.
    let input = json!({"opcode": 0x1009, "params": [XML]});
    assert_eq!(
        base64(&call(&mut s, "ptp_transaction", input).unwrap()["data"]),
        expected
    );
    let input = json!({"opcode": 0x101B, "params": [XML, start, 12]});
    let partial = base64(&call(&mut s, "ptp_transaction", input.clone()).unwrap()["data"]);
    assert_eq!(partial, &expected[start..start + 12]);
    let partial = base64(&call(&mut s, "ptp_transaction", reveal(input)).unwrap()["data"]);
    assert_eq!(partial, &raw[start..start + 12]);

    // read_file, for a file of that name (any case) in ExtData.
    watch.add_file("garmindevice.xml", raw.clone());
    let path = temp_path("device.xml");
    let out = call(
        &mut s,
        "read_file",
        json!({"name": "GARMINDEVICE.XML", "path": path}),
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert_eq!(out["sha256"], sha256_hex(&expected));
    let path = temp_path("device.xml");
    let input = json!({"name": "garmindevice.xml", "path": path, "reveal_identifiers": true});
    call(&mut s, "read_file", input).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), raw);

    // The live trace carries codes and sizes, and the log names and hashes: neither carries an
    // identifier.
    call(&mut s, "mtp_trace", json!({"enabled": true})).unwrap();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink: georgian_for_garmin_core::trace::Sink = {
        let seen = seen.clone();
        std::sync::Arc::new(move |t: &georgian_for_garmin_core::trace::Transaction| {
            seen.lock().unwrap().push(serde_json::to_string(t).unwrap());
        })
    };
    let control = Control::NONE.with_trace(Some(&sink));
    for (name, input) in [
        ("ptp_transaction", reveal(json!({"opcode": 0x1001}))),
        ("mtp_read", reveal(json!({"handle": XML}))),
        (
            "mtp_create",
            json!({"name": "006-D9035-04.tmp", "data_base64": STANDARD.encode(b"x"),
                              "allow_write": true}),
        ),
    ] {
        s.call(name, input, &control).unwrap();
    }
    let seen = seen.lock().unwrap();
    assert!(seen.len() > 5, "{seen:?}");
    let log = std::fs::read_to_string(&log).unwrap();
    for text in seen.iter().chain([&log]) {
        for id in [SERIAL, UNIT_ID, OWNER_NAME] {
            assert!(!text.contains(id), "{id} in {text}");
        }
    }
}

struct Rpc {
    input: Option<PipeWriter>,
    output: Receiver<Value>,
    server: Option<std::thread::JoinHandle<()>>,
}

struct LineSink(mpsc::Sender<Value>, Vec<u8>);

impl Write for LineSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.1.extend_from_slice(buf);
        while let Some(end) = self.1.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.1.drain(..=end).collect();
            let _ = self
                .0
                .send(serde_json::from_slice(&line).expect("one JSON message per line"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Rpc {
    fn start(watch: &Watch) -> Rpc {
        let (reader, writer) = std::io::pipe().unwrap();
        let (tx, output) = mpsc::channel();
        let watch = watch.clone();
        let server = std::thread::spawn(move || {
            let worker = Worker::spawn(move || session(&watch).0);
            rpc::serve(BufReader::new(reader), LineSink(tx, Vec::new()), worker);
        });
        Rpc {
            input: Some(writer),
            output,
            server: Some(server),
        }
    }

    fn send(&mut self, message: Value) {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{message}").unwrap();
    }

    fn next(&self) -> Value {
        self.output
            .recv_timeout(Duration::from_secs(10))
            .expect("a message")
    }

    /// Sends a request; returns the operation names of the traces before its response, and the
    /// response.
    fn traced(&mut self, id: i64, method: &str, params: Value) -> (Vec<String>, Value) {
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut traces = Vec::new();
        loop {
            let message = self.next();
            if message["method"] == "notifications/trace" {
                assert_eq!(message["params"]["requestId"], id, "{message}");
                let name = message["params"]["operation"].as_str().unwrap_or("?");
                traces.push(name.to_owned());
                continue;
            }
            assert_eq!(message["id"], id, "{message}");
            return (traces, message);
        }
    }

    fn close(mut self) -> Vec<Value> {
        drop(self.input.take());
        self.server.take().unwrap().join().unwrap();
        self.output.try_iter().collect()
    }
}

#[test]
fn rpc_sends_each_transaction_as_a_trace_before_the_response() {
    let watch = Watch::venu2();
    let mut rpc = Rpc::start(&watch);
    let (traces, out) = rpc.traced(1, "mtp_storage_ids", json!({}));
    assert!(traces.is_empty(), "off by default: {traces:?}");
    assert_eq!(out["result"]["storage_ids"], json!([STORAGE]));

    let (traces, out) = rpc.traced(2, "mtp_trace", json!({"enabled": true}));
    assert_eq!(out["result"]["enabled"], true);
    assert!(traces.is_empty());
    // The session's check that the watch still answers, then the call's own transactions.
    let input = json!({"storage": STORAGE, "parent": EXTDATA, "hidden": true});
    let (traces, out) = rpc.traced(3, "mtp_handles", input);
    assert_eq!(
        out["result"]["handles"],
        json!([IMPORTED_ARMENIAN, IMPORTED_CJK])
    );
    assert_eq!(
        traces,
        ["GetStorageIDs", "GetDeviceInfo", "GarminGetObjectIds"]
    );
    let input = json!({"opcode": 0x1008, "params": [IMPORTED_ARMENIAN]});
    let (traces, _) = rpc.traced(4, "ptp_transaction", input);
    assert_eq!(traces, ["GetStorageIDs", "GetObjectInfo"]);
    // A refused call sends nothing to trace.
    let (traces, out) = rpc.traced(5, "ptp_transaction", json!({"opcode": 0x100B}));
    assert_eq!(out["error"]["data"]["code"], "invalid_input");
    assert!(traces.is_empty());

    let (_, out) = rpc.traced(6, "mtp_trace", json!({"enabled": false}));
    assert_eq!(out["result"]["enabled"], false);
    let (traces, _) = rpc.traced(7, "mtp_storage_ids", json!({}));
    assert!(traces.is_empty());
    assert!(rpc.close().is_empty());
}

/// A traced call cancelled midway: its traces still come before its cancelled response, and
/// nothing comes after.
#[test]
fn a_traced_call_still_stops_on_cancel() {
    let watch = Watch::venu2();
    let mut rpc = Rpc::start(&watch);
    rpc.traced(1, "install", json!({}));
    rpc.traced(10, "mtp_trace", json!({"enabled": true}));
    watch.state().write_delay = Duration::from_millis(300);
    rpc.send(json!({"jsonrpc": "2.0", "id": 2, "method": "restore_stock",
                    "params": {"_meta": {"progressToken": "p"}}}));
    let (mut traces, mut steps) = (Vec::new(), Vec::new());
    let response = loop {
        let message = rpc.next();
        match message["method"].as_str() {
            Some("notifications/trace") => {
                assert_eq!(message["params"]["requestId"], 2);
                traces.push(message["params"]["operation"].clone());
            }
            Some("notifications/progress") => {
                let step = message["params"]["progress"].as_f64().unwrap();
                steps.push(step);
                if step == 1.0 {
                    rpc.send(
                        json!({"jsonrpc": "2.0", "method": "notifications/cancelled",
                                    "params": {"requestId": 2}}),
                    );
                }
            }
            _ => break message,
        }
    };
    assert_eq!(response["id"], 2);
    assert_eq!(response["error"]["code"], rpc::REQUEST_CANCELLED);
    assert!(steps.starts_with(&[0.0, 1.0]), "{steps:?}");
    assert!(traces.contains(&json!("SendObject")), "{traces:?}");
    assert!(
        traces.contains(&json!("DeleteObject")),
        "rolled back: {traces:?}"
    );
    assert!(rpc.close().is_empty(), "nothing after the response");
}
