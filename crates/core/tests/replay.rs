//! Replays every watch report posted in the repository's issues against a fake watch built from
//! it: inspect, install, a restart, install again, a restart, restore.
//!
//! The fixtures (`tests/reports/*.json`) come from the reports' text through
//! `scripts/report_fixture.py`; `tests/reports/README.md` lists them and the watch behaviors the
//! fake (`support/report_watch.rs`) models, each with the report that showed it. Whatever the
//! watch held, the real operations must never write over a file that is neither Garmin's
//! original nor one of this program's builds, never delete a hidden (imported, read-only)
//! object, put Garmin's original back wherever restore writes, and never refuse a state this
//! program's own builds produced with `unsupported_device`; install may ask to restore first
//! where it can't read a file the watch declares below 1.00, as it did after this program's
//! trailer-less builds.

#[path = "support/report_watch.rs"]
mod report_watch;

use georgian_for_garmin_core::fonts::{FontSet, sha256_hex};
use georgian_for_garmin_core::names;
use georgian_for_garmin_core::ops;
use georgian_for_garmin_core::{Error, ErrorCode};
use report_watch::*;
use serde_json::{Value, json};

/// Where a fixture comes from: a report in the repository's issues, the maintainer's own watch,
/// or a report a watch's owner emailed the maintainer (owner anonymized).
fn known_source(source: &str) -> bool {
    source.starts_with("https://github.com/lubaretsi/georgian-for-garmin/issues/")
        || source.starts_with("maintainer's ")
        || source.starts_with("maintainer email report, owner anonymized")
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

/// Whether a file of one of `parts`, under either name form, is neither Garmin's original nor
/// ours: in the normal listing, its own file or its import, or the watch's hidden import (a
/// bitmap part's `.fnt`, a TrueType part's `.bin`).
fn other_under(watch: &Watch, fonts: &FontSet, parts: &[&str]) -> bool {
    let s = watch.0.borrow();
    parts.iter().any(|part| {
        let Some(font) = font_for(fonts, part) else {
            return false;
        };
        s.objects.values().any(|o| {
            o.info.parent == EXTDATA
                && o.origin == Origin::Other
                && names::resolve(&o.info.filename).is_some_and(|f| {
                    f.part == font.part
                        && (f.role == imported_role(font)
                            || !o.hidden && f.role == font.target_role())
                })
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
            // Optional: a hand-composed fixture's sources, and a note on the report.
            .filter(|&k| k != "composed_from" && k != "note")
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

/// The Forerunner 255 on 29.52: Georgian confirmed with 0.2.2 (#13), its font list from that
/// install (#37, 0.2.2) and from 0.3.2 (#35). Every part install writes is tested there, so it
/// is `tested` with no report wanted. (On other software its status can change with the next
/// confirmation; the UI fixtures' `other-software` role watch shows that status.)
#[test]
fn the_forerunner_255_on_29_52_is_tested() {
    for name in ["issue-35.json", "issue-37.json"] {
        let (watch, fonts) = Watch::from_fixture(&fixture(name));
        let out = watch.call("inspect", &fonts).unwrap();
        assert_eq!(out["tested"]["status"], "tested", "{name}");
        assert_eq!(out["support"]["status"], "tested", "{name}");
        assert_eq!(out["support"]["issue"], 13, "{name}");
        assert_eq!(out["support"]["report_wanted"], false, "{name}");
    }
}

/// The names of the files the operations wrote since `from`, with their bytes.
fn written_since(watch: &Watch, from: usize) -> Vec<(String, Vec<u8>)> {
    let s = watch.0.borrow();
    s.written[from..]
        .iter()
        .filter_map(|h| s.objects.get(h))
        .map(|o| (o.info.filename.clone(), o.data.clone()))
        .collect()
}

/// The Lily 2 Active (`email-lily2active.json`) declares its fonts under short names
/// (`D894567.tmp`) and lists its imports so (`D894567.fnt`). With Garmin's originals imported,
/// install writes only its Korean parts' builds, under the declared short names; the restart
/// imports them as `D894567.fnt` and inspect reads Georgian installed; restore writes Garmin's
/// originals under the same names, and after the next restart Georgian is gone. No file under
/// the long name is ever written.
#[test]
fn the_lily_2_active_round_trips_its_short_names() {
    let bundled = FontSet::bundled();
    let parts = ["006-D8945-67", "006-D8945-68"];
    let lily = imports_as_garmins(fixture("email-lily2active.json"), &bundled, &parts);
    let (watch, fonts) = Watch::from_fixture(&lily);
    let font = |part: &str| font_for(&fonts, part).unwrap().clone();
    let (k67, k68) = (font(parts[0]), font(parts[1]));

    let out = watch.call("inspect", &fonts).unwrap();
    assert_eq!(out["state"], "not_installed");
    assert_eq!(
        targets(&out, "target"),
        ["006-D8945-67", "006-D8945-68"],
        "only the Korean parts the watch declares"
    );

    let installed = watch.call("install", &fonts).unwrap();
    assert_eq!(installed["status"], "staged_verified");
    assert_eq!(
        written_since(&watch, 0),
        [
            ("D894567.tmp".to_owned(), k67.georgian.to_vec()),
            ("D894568.tmp".to_owned(), k68.georgian.to_vec()),
        ]
    );
    watch.restart(&fonts);
    assert_eq!(watch.call("inspect", &fonts).unwrap()["state"], "installed");
    {
        let s = watch.0.borrow();
        let imported: Vec<(&str, bool)> = s
            .objects
            .values()
            .filter(|o| o.info.filename.starts_with("D8945"))
            .map(|o| (o.info.filename.as_str(), o.hidden))
            .collect();
        assert_eq!(imported, [("D894567.fnt", true), ("D894568.fnt", true)]);
    }

    let from = watch.0.borrow().written.len();
    let restored = watch.call("restore_stock", &fonts).unwrap();
    assert_eq!(restored["status"], "staged_verified");
    assert_eq!(
        written_since(&watch, from),
        [
            ("D894567.tmp".to_owned(), k67.stock.to_vec()),
            ("D894568.tmp".to_owned(), k68.stock.to_vec()),
        ]
    );
    watch.restart(&fonts);
    assert_eq!(
        watch.call("inspect", &fonts).unwrap()["state"],
        "not_installed"
    );
    let s = watch.0.borrow();
    assert!(
        s.objects
            .values()
            .all(|o| !o.info.filename.starts_with("006-D8945")),
        "a long name was written"
    );
    assert!(s.violations.is_empty(), "{:?}", s.violations);
}

/// The Lily 2 Active as reported: its imports' contents unknown (the report lists their sizes
/// only). Install and restore both refuse and write nothing, since the next restart would
/// import over a font neither Garmin's nor ours.
#[test]
fn the_lily_2_active_never_writes_over_unknown_imports() {
    let (watch, fonts) = Watch::from_fixture(&fixture("email-lily2active.json"));
    assert_eq!(watch.call("inspect", &fonts).unwrap()["state"], "other");
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

/// The watches that get Georgian in Korean fonts, from their reports: install writes the Korean
/// builds of exactly these parts, under the names the watch declares, and nothing else; restore
/// writes the pinned originals. Where a report lists hidden imports (the Lily 2 Active), those of
/// the target parts are read as Garmin's originals ([`imports_as_garmins`]); the 0.2.2 report
/// (#12) lists none, so its watch shows no import.
const KOREAN_ROUTE: [(&str, &[&str]); 2] = [
    ("issue-12.json", &["006-D8945-22"]),
    ("email-lily2active.json", &["006-D8945-67", "006-D8945-68"]),
];

#[test]
fn korean_route_watches_install_only_their_korean_builds() {
    let bundled = FontSet::bundled();
    for (name, parts) in KOREAN_ROUTE {
        let fixture = imports_as_garmins(fixture(name), &bundled, parts);
        let (watch, fonts) = Watch::from_fixture(&fixture);
        let declared_name = |part: &str| {
            let s = watch.0.borrow();
            s.declared
                .iter()
                .find(|d| d.1 == part)
                .map(|d| d.0.clone())
                .unwrap()
        };

        let out = watch.call("inspect", &fonts).unwrap();
        assert_eq!(targets(&out, "target"), parts, "{name}");
        assert_eq!(out["hosts"]["kind"], "bitmap", "{name}");
        // Installs: expected, or tested once someone confirms the model.
        assert!(
            matches!(
                out["support"]["status"].as_str(),
                Some("expected" | "tested")
            ),
            "{name}: {}",
            out["support"]
        );
        let packs: Vec<&str> = out["packs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        assert!(
            packs.iter().all(|p| p.starts_with("korean-")),
            "{name}: {packs:?}"
        );

        let installed = watch.call("install", &fonts).unwrap();
        assert_eq!(installed["status"], "staged_verified", "{name}");
        let expected: Vec<(String, Vec<u8>)> = parts
            .iter()
            .map(|p| {
                let font = font_for(&fonts, p).unwrap();
                (declared_name(p), font.georgian.to_vec())
            })
            .collect();
        assert_eq!(written_since(&watch, 0), expected, "{name}");
        for part in installed["packs"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["parts"].as_array().unwrap())
        {
            let font = font_for(&fonts, part["part"].as_str().unwrap()).unwrap();
            assert_eq!(part["sha256"], sha256_hex(&font.georgian), "{name}");
        }
        watch.restart(&fonts);
        if watch.0.borrow().get_object_ids {
            let again = watch.call("inspect", &fonts).unwrap();
            assert_eq!(again["state"], "installed", "{name}");
        }

        let restore_parts = targets(&out, "restore");
        let from = watch.0.borrow().written.len();
        let restored = watch.call("restore_stock", &fonts).unwrap();
        assert_eq!(restored["status"], "staged_verified", "{name}");
        // Every file restore writes is the pinned original of a part it restores.
        let written = written_since(&watch, from);
        for (file, data) in &written {
            let part = names::resolve(file).unwrap().part;
            assert!(restore_parts.contains(&part.as_str()), "{name}: {file}");
            assert_eq!(
                *data,
                *font_for(&fonts, &part).unwrap().stock,
                "{name}: {file}"
            );
        }
        for part in parts {
            assert!(
                written.iter().any(|(file, _)| *file == declared_name(part)),
                "{name}: restore didn't put back {part}"
            );
        }
        watch.restart(&fonts);
        let s = watch.0.borrow();
        assert!(s.violations.is_empty(), "{name}: {:?}", s.violations);
    }
}

/// Tested watches that also declare Korean fonts keep Georgian in their Armenian fonts: install
/// targets only bitmap Armenian parts, no Korean pack is listed, and they stay `tested`.
#[test]
fn tested_watches_keep_their_armenian_fonts() {
    for name in [
        "issue-11.json",
        "issue-15.json",
        "issue-35.json",
        "issue-37.json",
        "issue-46.json",
        "issue-4.json",
        "maintainer-venu2-georgian-imported.json",
        "maintainer-venu2-stock-imported.json",
    ] {
        let (watch, fonts) = Watch::from_fixture(&fixture(name));
        assert!(
            watch
                .0
                .borrow()
                .declared
                .iter()
                .any(|d| d.1.starts_with("006-D8945")),
            "{name} declares no Korean font"
        );
        let out = watch.call("inspect", &fonts).unwrap();
        let parts = targets(&out, "target");
        assert!(
            !parts.is_empty() && parts.iter().all(|p| p.starts_with("006-D9035")),
            "{name}: {parts:?}"
        );
        assert!(
            out["packs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["id"].as_str().unwrap().starts_with("armenian-")),
            "{name}: {}",
            out["packs"]
        );
        assert_eq!(out["support"]["status"], "tested", "{name}");
        assert_eq!(out["tested"]["status"], "tested", "{name}");
    }
}

/// The fenix 6 family lists only Swiss 721 Hebrew of this build's fonts, a family no watch
/// confirmed: install writes nothing, restore can still put Garmin's originals back.
#[test]
fn the_fenix_6_stays_restore_only() {
    for name in [
        "issue-3.json",
        "issue-6-comment-7080207810.json",
        "issue-14.json",
        "issue-20.json",
        "issue-25.json",
    ] {
        let (watch, fonts) = Watch::from_fixture(&fixture(name));
        let out = watch.call("inspect", &fonts).unwrap();
        assert_eq!(out["supported"], false, "{name}");
        assert_eq!(out["support"]["status"], "unsupported", "{name}");
        assert!(targets(&out, "target").is_empty(), "{name}");
        assert_eq!(
            targets(&out, "restore"),
            ["006-D0952-08", "006-D0952-07"],
            "{name}"
        );
        let err = watch.call("install", &fonts).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedDevice, "{name}");
        assert!(watch.0.borrow().written.is_empty(), "{name}");
    }
}

/// The Instinct 3 AMOLED, 50mm (#8, #33) and 45mm (#23, #55, #57): Georgian added to its Korean
/// 19 px font `006-D8945-07` didn't show in notifications after 0.3.4 installed it (#55, #57), so
/// the model is unsupported and that pack restore-only.
const INSTINCT_3: [&str; 5] = [
    "issue-8.json",
    "issue-23.json",
    "issue-33.json",
    "issue-55.json",
    "issue-57.json",
];

/// Each Instinct 3 report as reported, with 0.3.4's `-07` waiting as a `.tmp`, and, where the
/// report lists the watch's imports, with it imported (composed: the reports are from before
/// installing). Install refuses with `unsupported_device` and writes nothing; restore writes only
/// the pinned originals of the parts it restores, and puts `-07`'s back wherever the watch holds
/// our build; after the restart Georgian is gone.
#[test]
fn the_instinct_3_is_restore_only() {
    let bundled = FontSet::bundled();
    let k07 = font_for(&bundled, "006-D8945-07").unwrap();
    for name in INSTINCT_3 {
        let reported = imports_as_garmins(fixture(name), &bundled, &["006-D8945-07"]);
        let mut waiting = reported.clone();
        waiting["extdata"] = json!([{
            "name": "006-D8945-07.tmp", "bytes": k07.georgian.len(),
            "sha256": sha256_hex(&k07.georgian), "file": "georgian"
        }]);
        let mut variants = vec![
            ("as reported", reported.clone(), false),
            ("waiting", waiting, true),
        ];
        if reported["hidden"]["status"] == "listed" {
            let mut imported = reported.clone();
            for file in imported["hidden"]["files"].as_array_mut().unwrap() {
                if file["name"] == "006-D8945-07.fnt" {
                    file["bytes"] = json!(k07.georgian.len());
                    file["file"] = json!("georgian");
                }
            }
            variants.push(("imported", imported, true));
        }
        for (variant, fixture, ours) in variants {
            let at = format!("{name} {variant}");
            let (watch, fonts) = Watch::from_fixture(&fixture);
            let out = watch.call("inspect", &fonts).unwrap();
            assert_eq!(out["supported"], false, "{at}");
            assert_eq!(out["support"]["status"], "unsupported", "{at}");
            assert_eq!(out["support"]["issue"], 57, "{at}");
            assert_eq!(
                out["support"]["reason"]["en"],
                "Georgian added to its Korean 19 px font didn't show in notifications.",
                "{at}"
            );
            assert_eq!(out["support"]["report_wanted"], false, "{at}");
            assert!(targets(&out, "target").is_empty(), "{at}");
            assert_eq!(
                targets(&out, "restore"),
                ["006-D0952-08", "006-D0952-07", "006-D8945-07"],
                "{at}"
            );

            let err = watch.call("install", &fonts).unwrap_err();
            assert_eq!(
                err.code,
                ErrorCode::UnsupportedDevice,
                "{at}: {}",
                err.message
            );
            assert!(watch.0.borrow().written.is_empty(), "{at}");

            let restored = watch
                .call("restore_stock", &fonts)
                .unwrap_or_else(|e| panic!("{at}: restore: {e}"));
            let written = written_since(&watch, 0);
            for (file, data) in &written {
                let font = font_for(&fonts, file).unwrap();
                assert_eq!(*data, *font.stock, "{at}: {file}");
            }
            if ours {
                assert_eq!(restored["status"], "staged_verified", "{at}");
                assert!(
                    written.iter().any(|(file, _)| file == "006-D8945-07.tmp"),
                    "{at}: restore didn't put back 006-D8945-07: {written:?}"
                );
            }
            watch.restart(&fonts);
            assert!(
                watch
                    .0
                    .borrow()
                    .objects
                    .values()
                    .all(|o| o.data != *k07.georgian),
                "{at}: our 006-D8945-07 is still on the watch"
            );
            let s = watch.0.borrow();
            assert!(s.violations.is_empty(), "{at}: {:?}", s.violations);
        }
    }
}

/// A watch that lists the tested Noto TrueType fonts and a Korean bitmap font gets Georgian in
/// the TrueType ones: bitmap Armenian first, then TrueType, then Korean. The Forerunner 165
/// (#18) with the Korean 006-D8945-22 added to what it declares.
#[test]
fn truetype_comes_before_korean() {
    let mut fr165 = fixture("issue-18.json");
    fr165["declared"].as_array_mut().unwrap().push(json!({
        "path": "Garmin/ExtData", "file": "006-D8945-22.tmp", "part": "006-D8945-22",
        "version": "2.10"
    }));
    let (watch, fonts) = Watch::from_fixture(&fr165);
    let out = watch.call("inspect", &fonts).unwrap();
    assert_eq!(out["hosts"]["kind"], "truetype");
    let parts = targets(&out, "target");
    assert!(
        !parts.is_empty() && !parts.contains(&"006-D8945-22"),
        "{parts:?}"
    );
    assert_eq!(out["hosts"]["skipped_parts"], json!(["006-D8945-22"]));
}
