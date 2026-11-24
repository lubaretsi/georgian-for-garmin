//! `diagnose` against the fake Venu 2, with USB devices and volumes given by the test: the
//! connection's outcomes, drives, events, and that nothing in the output identifies the watch.

#[path = "support/fake.rs"]
mod fake;

use std::borrow::Cow;
use std::path::PathBuf;

use fake::Watch;
use georgian_for_garmin_core::busy::{Busy, Holder, UsbInterface};
use georgian_for_garmin_core::diagnose::{
    DrivesReport, HEADER, Probe, UsbDevice, UsbReport, drives_at,
};
use georgian_for_garmin_core::session::{Control, Session};
use georgian_for_garmin_core::{Error, ErrorCode};
use serde_json::{Value, json};

fn no_usb() -> UsbReport {
    UsbReport {
        listed: true,
        note: None,
        devices: Vec::new(),
    }
}

fn no_drives() -> DrivesReport {
    drives_at(&[])
}

fn probe(usb: UsbReport, drives: DrivesReport) -> Probe<'static> {
    Probe {
        usb: Box::new(move || usb.clone()),
        drives: Box::new(move || drives.clone()),
    }
}

/// A one-shot session, as the CLI and the app run it, whose connection is `connect`.
fn one_shot(
    connect: impl FnMut() -> Result<Box<dyn georgian_for_garmin_core::mtp::Mtp>, Error> + 'static,
    probe: Probe<'static>,
) -> Session<'static> {
    Session::one_shot(Cow::Owned(fake::fonts()), Box::new(connect)).with_probe(probe)
}

fn diagnose(session: &mut Session<'_>, input: Value) -> Result<Value, Error> {
    session.call("diagnose", input, &Control::NONE)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gfg-diagnose-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A watch volume whose GarminDevice.xml holds the fake's identifiers.
fn watch_volume(name: &str) -> PathBuf {
    let root = temp_dir(name).join("GARMIN");
    std::fs::create_dir_all(root.join("GARMIN/ExtData")).unwrap();
    std::fs::write(
        root.join("GARMIN/GarminDevice.xml"),
        format!(
            "<?xml version=\"1.0\"?><Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">\
             <Model><PartNumber>006-B3076-00</PartNumber><SoftwareVersion>1060</SoftwareVersion>\
             <Description>Forerunner 245</Description></Model><Id>{}</Id><Ifix>{}</Ifix>\
             <MassStorageMode></MassStorageMode></Device>",
            fake::UNIT_ID,
            fake::IFIX
        ),
    )
    .unwrap();
    root
}

/// No string in `out` holds one of the fake's identifiers or a run of 9 or more digits (the
/// unit ID's shape), as `fixtures_hold_no_identifiers` checks the report fixtures.
fn assert_no_identifiers(out: &Value) {
    fn scan(value: &Value, key: &str) {
        match value {
            Value::Object(map) => map.iter().for_each(|(k, v)| scan(v, k)),
            Value::Array(items) => items.iter().for_each(|v| scan(v, key)),
            Value::String(s) if key != "sha256" => {
                let digits = s
                    .split(|c: char| !c.is_ascii_digit())
                    .map(str::len)
                    .max()
                    .unwrap_or(0);
                assert!(digits < 9, "{key} {s:?} looks like an identifier");
            }
            _ => {}
        }
    }
    scan(out, "");
    let all = out.to_string();
    for id in [
        fake::SERIAL,
        fake::UNIT_ID,
        fake::IFIX,
        fake::APP_ID,
        fake::OWNER_NAME,
    ] {
        assert!(!all.contains(id), "{id} is in the output");
    }
}

#[test]
fn a_readable_watch_gives_its_report() {
    let watch = Watch::venu2();
    let w = watch.clone();
    let mut session = one_shot(move || w.connect(), probe(no_usb(), no_drives()));
    let out = diagnose(&mut session, json!({})).unwrap();
    assert_eq!(
        out["connection"],
        json!({"ok": true, "code": null, "message": null, "busy": null})
    );
    assert_eq!(out["watch"]["model"], "Venu 2");
    assert_eq!(out["app"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(out["events"], json!([]));
    let text = out["text"].as_str().unwrap();
    assert_eq!(text.lines().next(), Some(HEADER));
    assert!(text.contains("\nconnection ok\n"), "{text}");
    assert!(text.contains("\nwatch report:\n  "), "{text}");
    let report_text = out["watch"]["text"].as_str().unwrap();
    assert!(text.contains(&format!("  {}", report_text.lines().next().unwrap())));
    assert_no_identifiers(&out);
    // Only read: nothing was written or deleted.
    let log = watch.state().log.clone();
    assert!(
        !log.iter()
            .any(|op| op.contains("Send") || op.contains("Delete")),
        "{log:?}"
    );
}

#[test]
fn device_problems_are_data_not_errors() {
    let watch = Watch::venu2();
    watch.unplug();
    let w = watch.clone();
    let mut session = one_shot(move || w.connect(), probe(no_usb(), no_drives()));
    let out = diagnose(&mut session, json!({})).unwrap();
    assert_eq!(out["connection"]["ok"], false);
    assert_eq!(out["connection"]["code"], "no_device");
    assert_eq!(out["watch"], Value::Null);
    let text = out["text"].as_str().unwrap();
    assert!(text.contains("\nconnection no_device message="), "{text}");
    assert!(text.contains("\nwatch -\n"), "{text}");

    let mut session = one_shot(
        || {
            Err(Error::new(
                ErrorCode::PermissionDenied,
                "cannot open the watch: permission denied",
            ))
        },
        probe(no_usb(), no_drives()),
    );
    let out = diagnose(&mut session, json!({})).unwrap();
    assert_eq!(out["connection"]["code"], "permission_denied");
    assert_eq!(
        out["connection"]["message"],
        "cannot open the watch: permission denied"
    );
    assert_no_identifiers(&out);
}

fn held_by_express() -> Busy {
    Busy {
        holder: Holder::named("Garmin Express", Some(812), true),
        product_id: Some(0x4E77),
        product_name: None,
        interface: Some(UsbInterface {
            class: 0xFF,
            subclass: 0xFF,
            protocol: 0,
        }),
    }
}

#[test]
fn a_busy_watch_names_its_holder() {
    let usb = UsbReport {
        listed: true,
        note: None,
        devices: vec![UsbDevice {
            product_id: 0x4E77,
            product_name: None,
            mtp: true,
            interfaces: vec![UsbInterface {
                class: 0xFF,
                subclass: 0xFF,
                protocol: 0,
            }],
            holder: Some(Holder::named("Garmin Express", Some(812), true)),
        }],
    };
    let mut session = one_shot(
        || Err(Error::busy(held_by_express())),
        probe(usb, no_drives()),
    );
    let out = diagnose(&mut session, json!({})).unwrap();
    assert_eq!(out["connection"]["ok"], false);
    assert_eq!(out["connection"]["code"], "device_busy");
    assert_eq!(
        out["connection"]["busy"]["holder"]["kind"],
        "garmin_express"
    );
    assert_eq!(out["usb"]["devices"][0]["product_id"], 0x4E77);
    assert_eq!(out["usb"]["devices"][0]["holder"]["pid"], 812);
    let text = out["text"].as_str().unwrap();
    assert!(
        text.contains(
            "\n  device product_id=4e77 product=- mtp=yes interfaces=ff/ff/00 \
             holder=garmin_express confirmed name=\"Garmin Express\" pid=812\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("\n  busy garmin_express confirmed name=\"Garmin Express\" pid=812 "),
        "{text}"
    );
    assert_no_identifiers(&out);
}

/// One connection per process: in a session holding the watch, diagnose reads it through that
/// connection instead of opening it again, which the watch would refuse as busy.
#[test]
fn a_session_holding_the_watch_reads_it_through_its_connection() {
    let watch = Watch::venu2();
    let w = watch.clone();
    let mut opened = false;
    let exclusive = move || {
        if opened {
            return Err(Error::busy(Busy {
                holder: Holder::named("georgian-for-ga", Some(std::process::id()), true),
                ..Busy::unknown()
            }));
        }
        opened = true;
        w.connect()
    };
    let mut session = Session::new(Cow::Owned(fake::fonts()), Box::new(exclusive))
        .with_probe(probe(no_usb(), no_drives()));
    session.call("inspect", json!({}), &Control::NONE).unwrap();
    let out = diagnose(&mut session, json!({})).unwrap();
    assert_eq!(out["connection"]["ok"], true, "{}", out["connection"]);
    assert_eq!(out["watch"]["model"], "Venu 2");
    assert_eq!(watch.state().connects, 1);

    // The second open, had there been one, would have named this program as the holder.
    let mut again = one_shot(
        || {
            Err(Error::busy(Busy {
                holder: Holder::named("georgian-for-ga", Some(std::process::id()), true),
                ..Busy::unknown()
            }))
        },
        probe(no_usb(), no_drives()),
    );
    let out = diagnose(&mut again, json!({})).unwrap();
    assert_eq!(out["connection"]["busy"]["holder"]["kind"], "this_app");
}

#[test]
fn mounted_watch_volumes_are_listed_by_name() {
    let volume = watch_volume("drive");
    let broken = temp_dir("broken").join("NO NAME");
    std::fs::create_dir_all(broken.join("GARMIN")).unwrap();
    std::fs::write(broken.join("GARMIN/GarminDevice.xml"), "not xml").unwrap();
    let drives = drives_at(&[volume, broken]);
    let mut session = one_shot(
        || Err(Error::new(ErrorCode::NoDevice, "No watch.")),
        probe(no_usb(), drives),
    );
    let out = diagnose(&mut session, json!({})).unwrap();
    assert_eq!(
        out["drives"],
        json!({"listed": true, "note": null, "volumes": [
            {"volume": "GARMIN", "model": "Forerunner 245", "part_number": "006-B3076-00",
             "software_version": "10.60", "error": null},
            {"volume": "NO NAME", "model": null, "part_number": null, "software_version": null,
             "error": "garmin_folder_not_found"},
        ]})
    );
    let text = out["text"].as_str().unwrap();
    assert!(
        text.contains(
            "\n  volume GARMIN model=\"Forerunner 245\" part=006-B3076-00 software=10.60 error=-\n"
        ),
        "{text}"
    );
    // The volume's path (which can hold the user's name) is never shown.
    assert!(!out.to_string().contains("gfg-diagnose"));
    assert_no_identifiers(&out);
}

fn event(detail: &str, model: Option<&str>) -> Value {
    json!({"time": "2026-11-24T03:14:05", "kind": "operation_end", "detail": detail,
           "model": model})
}

#[test]
fn events_are_echoed_capped_and_checked() {
    let unreachable = || -> Result<Box<dyn georgian_for_garmin_core::mtp::Mtp>, Error> {
        Err(Error::new(ErrorCode::NoDevice, "No watch."))
    };
    let mut session = one_shot(unreachable, probe(no_usb(), no_drives()));
    let events: Vec<Value> = (0..50)
        .map(|i| {
            event(
                if i % 2 == 0 {
                    "no_device"
                } else {
                    "install: staged_verified"
                },
                Some("Venu 2"),
            )
        })
        .collect();
    let out = diagnose(&mut session, json!({"events": events})).unwrap();
    assert_eq!(out["events"], json!(events));
    let text = out["text"].as_str().unwrap();
    assert!(text.contains("\nevents 50\n"), "{text}");
    assert!(
        text.contains(
            "\n  2026-11-24T03:14:05 operation_end \"install: staged_verified\" model=\"Venu 2\"\n"
        ),
        "{text}"
    );
    assert_no_identifiers(&out);

    let mut refused = |input: Value| {
        let e = diagnose(&mut session, input).expect_err("refused");
        assert_eq!(e.code, ErrorCode::InvalidInput, "{}", e.message);
        e.message
    };
    let mut too_many = events.clone();
    too_many.push(event("no_device", None));
    refused(json!({"events": too_many}));
    for (detail, model) in [
        (fake::SERIAL, None),
        (&*format!("unit {}", fake::UNIT_ID), None),
        ("no_device", Some(fake::IFIX)),
        ("serial A1B2C3D4E5F6", None),
    ] {
        let message = refused(json!({"events": [event(detail, model)]}));
        assert!(message.contains("looks like an identifier"), "{message}");
    }
    refused(json!({"events": [{"time": "03:14", "kind": "poll", "detail": "x", "model": null}]}));
    refused(
        json!({"events": [{"time": "2026-11-24T03:14:05", "kind": "reboot", "detail": "x",
                               "model": null}]}),
    );
    refused(
        json!({"events": [{"time": "2026-11-24T03:14:05", "kind": "poll", "detail": "x",
                               "model": null, "serial": "x"}]}),
    );
}
