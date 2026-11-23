//! Sessions, `extdata_list`, `wait_for_watch`, the development operations and the `rpc` loop,
//! against a fake Venu 2 that can be unplugged.

#[path = "support/fake.rs"]
mod fake;

use std::borrow::Cow;
use std::io::{BufReader, PipeWriter, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use fake::Watch;
use georgian_for_garmin_core::fonts::sha256_hex;
use georgian_for_garmin_core::names::{self, Role};
use georgian_for_garmin_core::session::{Control, Progress, Session, Worker};
use georgian_for_garmin_core::{Error, ErrorCode, ops, rpc};
use serde_json::{Value, json};

fn session(watch: &Watch) -> Session<'static> {
    let watch = watch.clone();
    Session::new(Cow::Owned(fake::fonts()), Box::new(move || watch.connect()))
        .with_poll_interval(Duration::from_millis(10))
}

fn code(result: Result<Value, Error>) -> ErrorCode {
    result.expect_err("an error").code
}

fn call(session: &mut Session<'_>, name: &str, input: Value) -> Result<Value, Error> {
    session.call(name, input, &Control::NONE)
}

#[test]
fn a_session_keeps_the_watch_open() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    call(&mut s, "inspect", json!({})).unwrap();
    call(&mut s, "extdata_list", json!({})).unwrap();
    assert_eq!(watch.state().connects, 1);

    let mut connect = || watch.connect();
    ops::call("inspect", json!({}), &fake::fonts(), &mut connect).unwrap();
    ops::call("inspect", json!({}), &fake::fonts(), &mut connect).unwrap();
    assert_eq!(
        watch.state().connects,
        3,
        "one-shot calls open the watch each time"
    );
}

#[test]
fn a_session_reopens_the_watch_after_unplug_and_replug() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    call(&mut s, "inspect", json!({})).unwrap();
    watch.unplug();
    assert_eq!(
        code(call(&mut s, "inspect", json!({}))),
        ErrorCode::NoDevice
    );
    assert_eq!(
        code(call(&mut s, "extdata_list", json!({}))),
        ErrorCode::NoDevice
    );
    watch.plug();
    call(&mut s, "inspect", json!({})).unwrap();
    assert_eq!(watch.state().connects, 2);

    // Replugged between two calls: the held connection is dead and is replaced.
    watch.unplug();
    watch.plug();
    call(&mut s, "extdata_list", json!({})).unwrap();
    assert_eq!(watch.state().connects, 3);
}

/// Records progress; `cancel_at` sets `cancel` when progress reaches it.
struct Recorder {
    steps: Mutex<Vec<Progress>>,
    cancel: AtomicBool,
    cancel_at: Option<f64>,
}

impl Recorder {
    fn new(cancel_at: Option<f64>) -> Recorder {
        Recorder {
            steps: Mutex::new(Vec::new()),
            cancel: AtomicBool::new(false),
            cancel_at,
        }
    }

    fn sink(&self, p: Progress) {
        if self.cancel_at.is_some_and(|at| p.progress >= at) {
            self.cancel.store(true, Ordering::SeqCst);
        }
        self.steps.lock().unwrap().push(p);
    }

    fn progress(&self) -> Vec<(f64, Option<f64>)> {
        let steps = self.steps.lock().unwrap();
        steps.iter().map(|p| (p.progress, p.total)).collect()
    }

    fn run(&self, s: &mut Session<'_>, name: &str, input: Value) -> Result<Value, Error> {
        let sink = |p| self.sink(p);
        s.call(name, input, &Control::new(Some(&sink), Some(&self.cancel)))
    }
}

/// The two ways the hidden listing comes back incomplete: Garmin's GetObjectIds fails for
/// ExtData, or GetObjectInfo fails for one hidden child (the imported CJK font).
fn incomplete_hidden_listings() -> [(&'static str, Watch); 2] {
    let ids = Watch::venu2();
    ids.state().fail_object_ids_of = Some(fake::EXTDATA);
    let info = Watch::venu2();
    info.state().fail_object_info_of = Some(fake::IMPORTED_CJK);
    [
        ("GetObjectIds fails", ids),
        ("GetObjectInfo fails for one child", info),
    ]
}

/// Whether the watch was sent anything that writes.
fn wrote(watch: &Watch) -> bool {
    watch
        .state()
        .log
        .iter()
        .any(|o| o.starts_with("Send") || o == "DeleteObject")
}

/// Install and restore never count an incomplete hidden listing as complete: an import it
/// leaves unseen would go unchecked and be replaced at the next restart. Nothing is written.
#[test]
fn install_and_restore_refuse_an_incomplete_hidden_listing() {
    for (case, watch) in incomplete_hidden_listings() {
        let mut s = session(&watch);
        for name in ["install", "restore_stock"] {
            let error = call(&mut s, name, json!({})).unwrap_err();
            assert_eq!(
                error.code,
                ErrorCode::DeviceError,
                "{case}, {name}: {error}"
            );
            assert!(error.message.contains("hidden listing"), "{case}: {error}");
            assert!(
                error.message.contains("nothing was written"),
                "{case}: {error}"
            );
        }
        assert!(!wrote(&watch), "{case}: {:?}", watch.state().log);
        assert!(watch.listed().is_empty(), "{case}");
    }
}

/// The read-only listings still answer, and flag what they couldn't read.
#[test]
fn listings_flag_an_incomplete_hidden_listing() {
    for (case, watch) in incomplete_hidden_listings() {
        let mut s = session(&watch);
        let out = call(&mut s, "extdata_list", json!({})).unwrap();
        assert_eq!(out["hidden_listing"], "error", "{case}: {out}");
        assert!(out["note"].is_string(), "{case}: {out}");
        let out = call(&mut s, "inspect", json!({})).unwrap();
        assert_eq!(out["extdata_hidden"]["status"], "error", "{case}");
        assert!(out["extdata_hidden"]["note"].is_string(), "{case}");
    }
}

#[test]
fn install_reports_one_step_per_part() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    let recorder = Recorder::new(None);
    let out = recorder.run(&mut s, "install", json!({})).unwrap();
    assert_eq!(out["status"], "staged_verified");
    let total = Some(4.0);
    assert_eq!(
        recorder.progress(),
        [
            (0.0, total),
            (1.0, total),
            (2.0, total),
            (3.0, total),
            (4.0, total)
        ]
    );
}

#[test]
fn a_cancelled_install_rolls_back_what_it_wrote() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    let recorder = Recorder::new(Some(2.0));
    let error = recorder.run(&mut s, "install", json!({})).unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert!(error.message.contains("Rolled back"), "{}", error.message);
    let rollback = error.to_json()["error"]["rollback"].clone();
    assert_eq!(rollback["status"], "complete", "{rollback}");
    let undone: Vec<&Value> = rollback["files"].as_array().unwrap().iter().collect();
    assert_eq!(undone.len(), 2, "{rollback}");
    assert!(
        undone
            .iter()
            .all(|f| f["done"] == true && f["action"] == "delete")
    );
    assert!(watch.listed().is_empty(), "{:?}", watch.listed());

    // Cancelled before anything was written.
    let recorder = Recorder::new(Some(0.0));
    let error = recorder
        .run(&mut s, "restore_stock", json!({}))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert!(
        error.message.contains("nothing was written"),
        "{}",
        error.message
    );
    assert!(watch.listed().is_empty());
}

#[test]
fn wait_for_watch_waits_for_a_replug() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    let out = call(&mut s, "wait_for_watch", json!({"timeout_seconds": 0})).unwrap();
    assert_eq!(out["model"], "Venu 2");

    watch.unplug();
    let plug = {
        let watch = watch.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            watch.plug();
        })
    };
    let recorder = Recorder::new(None);
    let out = recorder
        .run(&mut s, "wait_for_watch", json!({"timeout_seconds": 10}))
        .unwrap();
    plug.join().unwrap();
    assert_eq!(out["model"], "Venu 2");
    assert!(out["waited_seconds"].as_f64().unwrap() >= 0.1, "{out}");
    let progress = recorder.progress();
    assert!(!progress.is_empty());
    assert!(progress.windows(2).all(|w| w[0].0 < w[1].0), "{progress:?}");
    assert!(progress.iter().all(|p| p.1 == Some(10.0)));
    // The session holds the new connection.
    let connects = watch.state().connects;
    call(&mut s, "inspect", json!({})).unwrap();
    assert_eq!(watch.state().connects, connects);
}

#[test]
fn wait_for_watch_times_out_and_stops_on_cancel() {
    let watch = Watch::venu2();
    watch.unplug();
    let mut s = session(&watch);
    let input = json!({"timeout_seconds": 0.05});
    assert_eq!(
        code(call(&mut s, "wait_for_watch", input)),
        ErrorCode::NoDevice
    );

    let recorder = Recorder::new(Some(0.0));
    let input = json!({"timeout_seconds": 60});
    let error = recorder.run(&mut s, "wait_for_watch", input).unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
}

#[test]
fn inputs_are_checked_against_the_schemas() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    for (name, input) in [
        ("extdata_list", json!({"limit": 0})),
        ("extdata_list", json!({"limit": 1001})),
        ("extdata_list", json!({"offset": -1})),
        ("extdata_list", json!({"hash": "006-D9035-03.fnt"})),
        ("extdata_list", json!({"unknown": true})),
        ("wait_for_watch", json!({"timeout_seconds": -1})),
        ("wait_for_watch", json!({"timeout_seconds": 3601})),
        ("no_such_operation", json!({})),
    ] {
        assert_eq!(
            code(call(&mut s, name, input.clone())),
            ErrorCode::InvalidInput,
            "{name} {input}"
        );
    }
    assert_eq!(
        watch.state().connects,
        0,
        "invalid input never opens the watch"
    );
}

#[test]
fn extdata_list_shows_both_listings_with_hashes_on_request() {
    let watch = Watch::venu2();
    let staged = watch.add_file("006-D9035-01.tmp", fake::georgian("006-D9035-01"));
    let mut s = session(&watch);
    let input = json!({"hash": ["006-d9035-03.FNT", "006-D9035-01.tmp", "missing.fnt"]});
    let out = call(&mut s, "extdata_list", input).unwrap();
    assert_eq!(out["hidden_listing"], "listed");
    assert_eq!(out["total"], 3);
    assert_eq!(out["next_offset"], Value::Null);
    assert_eq!(out["hash_not_found"], json!(["missing.fnt"]));
    let files = out["files"].as_array().unwrap();
    let names: Vec<&str> = files.iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["006-D9035-01.tmp", "006-D9035-03.fnt", "D844870.fnt"]
    );
    assert_eq!(
        files[0],
        json!({"name": "006-D9035-01.tmp", "handle": format!("{staged:#010x}"),
               "bytes": fake::georgian("006-D9035-01").len(), "hidden": false,
               "read_only": false, "sha256": sha256_hex(&fake::georgian("006-D9035-01"))})
    );
    assert_eq!(files[1]["hidden"], true);
    assert_eq!(files[1]["read_only"], true);
    assert_eq!(files[1]["sha256"], sha256_hex(&fake::stock("006-D9035-03")));
    assert_eq!(files[2]["sha256"], Value::Null);

    // Hashes are kept for the connection: a second call doesn't read the file again.
    let reads = |w: &Watch| w.state().log.iter().filter(|o| *o == "GetObject").count();
    let before = reads(&watch);
    call(
        &mut s,
        "extdata_list",
        json!({"hash": ["006-D9035-03.fnt"]}),
    )
    .unwrap();
    assert_eq!(
        reads(&watch),
        before,
        "the hash comes from the session's cache"
    );

    // Pages.
    let page = call(&mut s, "extdata_list", json!({"offset": 1, "limit": 1})).unwrap();
    assert_eq!(page["files"].as_array().unwrap().len(), 1);
    assert_eq!(page["files"][0]["name"], "006-D9035-03.fnt");
    assert_eq!(page["next_offset"], 2);

    // The whole list to a file.
    let path = temp_path("listing.json");
    let out = call(
        &mut s,
        "extdata_list",
        json!({"output_file": path.to_str().unwrap(), "limit": 1}),
    )
    .unwrap();
    assert_eq!(out["files"], json!([]));
    assert_eq!(out["output_file"], path.to_str().unwrap());
    let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["files"].as_array().unwrap().len(), 3);
    let _ = std::fs::remove_file(&path);
}

fn temp_path(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::AtomicUsize;
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "gfg-session-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// The `rollback` paths that weren't undone.
fn left(error: &Error) -> Vec<String> {
    let rollback = error.rollback.as_ref().expect("a rollback");
    let mut left: Vec<String> = rollback
        .files
        .iter()
        .filter(|f| !f.done)
        .map(|f| f.path.clone())
        .collect();
    left.sort();
    left.dedup();
    left
}

#[test]
fn wait_for_watch_notices_that_the_held_watch_is_gone() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    call(&mut s, "inspect", json!({})).unwrap();
    watch.unplug();
    // The held connection's device_info is cached, as on both real backends: it must not count
    // as an answer.
    let input = json!({"timeout_seconds": 0.05});
    assert_eq!(
        code(call(&mut s, "wait_for_watch", input)),
        ErrorCode::NoDevice
    );
    watch.plug();
    call(&mut s, "wait_for_watch", json!({"timeout_seconds": 1})).unwrap();
    call(&mut s, "inspect", json!({})).unwrap();
    assert_eq!(watch.state().connects, 2);
}

#[test]
fn a_failed_install_whose_rollback_fails_says_it_is_incomplete() {
    let watch = Watch::venu2();
    {
        let mut state = watch.state();
        state.corrupt_create_at = Some(2);
        state.fail_delete = true;
    }
    let mut s = session(&watch);
    let error = call(&mut s, "install", json!({})).unwrap_err();
    assert_eq!(error.code, ErrorCode::VerificationFailed);
    assert!(error.rollback_incomplete());
    assert_eq!(
        left(&error),
        [
            "GARMIN/ExtData/006-D9035-01.tmp",
            "GARMIN/ExtData/006-D9035-02.tmp"
        ]
    );
    assert!(
        error.message.contains("rollback was incomplete"),
        "{}",
        error.message
    );
    assert!(error.message.contains("restore-stock"), "{}", error.message);
    assert!(!error.message.contains("Rolled back"), "{}", error.message);
    let json = error.to_json();
    assert_eq!(json["error"]["rollback"]["status"], "incomplete");

    // The first write failing alone, its partial file undeletable: incomplete too.
    let watch = Watch::venu2();
    {
        let mut state = watch.state();
        state.corrupt_create_at = Some(1);
        state.fail_delete = true;
    }
    let mut s = session(&watch);
    let error = call(&mut s, "install", json!({})).unwrap_err();
    assert_eq!(left(&error), ["GARMIN/ExtData/006-D9035-01.tmp"]);
    assert!(
        error.message.contains("rollback was incomplete"),
        "{}",
        error.message
    );
}

#[test]
fn a_cancelled_install_disconnected_mid_rollback_says_it_is_incomplete() {
    let watch = Watch::venu2();
    let mut s = session(&watch);
    let cancel = AtomicBool::new(false);
    let sink = |p: Progress| {
        if p.progress >= 2.0 {
            cancel.store(true, Ordering::SeqCst);
            watch.unplug();
        }
    };
    let error = s
        .call(
            "install",
            json!({}),
            &Control::new(Some(&sink), Some(&cancel)),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert_eq!(
        left(&error),
        [
            "GARMIN/ExtData/006-D9035-01.tmp",
            "GARMIN/ExtData/006-D9035-02.tmp"
        ]
    );
    assert!(
        error.message.contains("rollback was incomplete"),
        "{}",
        error.message
    );
    watch.plug();
    assert_eq!(watch.listed().len(), 2, "both files really are still there");
}

#[test]
fn an_imported_font_in_the_normal_listing_is_identified() {
    // Same size as Garmin's file, other bytes: neither stock nor Georgian.
    let mut unknown = fake::stock("006-D9035-02");
    unknown[5] ^= 0xff;
    let watch = Watch::venu2();
    watch.add_file("006-D9035-02.fnt", unknown);
    let mut s = session(&watch);
    for name in ["install", "restore_stock"] {
        let error = call(&mut s, name, json!({"parts": ["006-D9035-02"]})).unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict, "{name}");
        assert!(
            error.message.contains("006-D9035-02.fnt"),
            "{}",
            error.message
        );
    }
    assert_eq!(watch.listed().len(), 1, "nothing was written");

    // A Georgian import there is this program's: inspect reports it, restore replaces it.
    let watch = Watch::venu2();
    watch.add_file("006-D9035-02.fnt", fake::georgian("006-D9035-02"));
    let mut s = session(&watch);
    let out = call(&mut s, "inspect", json!({})).unwrap();
    let font = out["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["part"] == "006-D9035-02")
        .unwrap()
        .clone();
    assert_eq!(
        (font["file"].clone(), font["imported"].clone()),
        (json!("georgian"), json!(true))
    );
    let out = call(&mut s, "restore_stock", json!({"parts": ["006-D9035-02"]})).unwrap();
    assert_eq!(out["status"], "staged_verified");
}

/// `ExtData`'s normal listing by part, role and contents: what the watch holds, whatever the
/// files are named.
fn by_part(watch: &Watch) -> Vec<(String, Role, Vec<u8>)> {
    let mut files: Vec<(String, Role, Vec<u8>)> = watch
        .listed()
        .into_iter()
        .map(|(name, data)| {
            let file = names::resolve(&name).expect("a part's file");
            (file.part, file.role, data)
        })
        .collect();
    files.sort_by(|a, b| (&a.0, a.1 as u8, &a.2).cmp(&(&b.0, b.1 as u8, &b.2)));
    files
}

/// Makes a fresh fake watch: [`Watch::venu2`] or [`Watch::declaring_short_names`].
type MakeWatch = fn() -> Watch;

/// What inspect says of part 006-D9035-02's file on the watch `watch` makes, and what install
/// and restore of that part do: their outcome (status or error code) and what the watch holds
/// afterwards.
fn outcome(watch: MakeWatch, place: &str, name: &str, data: &[u8]) -> Vec<Value> {
    let mut results = Vec::new();
    for operation in ["inspect", "install", "restore_stock"] {
        let watch = watch();
        match place {
            "normal" => watch.add_file(name, data.to_vec()),
            _ => watch.add_hidden(name, data.to_vec()),
        };
        let mut s = session(&watch);
        let input = if operation == "inspect" {
            json!({})
        } else {
            json!({"parts": ["006-D9035-02"]})
        };
        let result = match call(&mut s, operation, input) {
            Ok(out) if operation == "inspect" => {
                let font = out["fonts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|f| f["part"] == "006-D9035-02")
                    .unwrap()
                    .clone();
                json!([font["file"], font["imported"]])
            }
            // Success either way: what the watch then holds is in `watch`. (An identical
            // waiting file under the other name form is written again under the name the watch
            // declares, so its status is staged_verified, not already_staged.)
            Ok(_) => json!("ok"),
            Err(e) => json!(e.code.as_str()),
        };
        let held: Vec<Value> = by_part(&watch)
            .into_iter()
            .map(|(part, role, data)| json!([part, format!("{role:?}"), sha256_hex(&data)]))
            .collect();
        results.push(json!({"operation": operation, "result": result, "watch": held}));
    }
    results
}

/// Every state and listing a part's file can be in, under either name form, gives the same
/// inspect result and the same install and restore outcome, on a watch declaring part numbers
/// and on one declaring short names: one name rule for all of them.
#[test]
fn both_name_forms_give_the_same_result_everywhere() {
    let part = "006-D9035-02";
    let mut unknown = fake::stock(part);
    unknown[7] ^= 0xff;
    let states = [
        ("stock", fake::stock(part)),
        ("georgian", fake::georgian(part)),
        ("unknown", unknown),
    ];
    let places = [
        ("normal", Role::Waiting),
        ("normal", Role::Imported),
        ("hidden", Role::Imported),
    ];
    let watches: [(&str, MakeWatch); 2] = [
        ("long", Watch::venu2),
        ("short", Watch::declaring_short_names),
    ];
    for (declares, watch) in watches {
        for (state, data) in &states {
            for (place, role) in places {
                let ext = if role == Role::Waiting { "tmp" } else { "fnt" };
                let long = outcome(watch, place, &format!("{part}.{ext}"), data);
                let short = outcome(watch, place, &format!("D903502.{ext}"), data);
                assert_eq!(long, short, "declares {declares}: {state} {place} {role:?}");
            }
        }
        // And the cases mean something: an unknown waiting file blocks both, a stock one is
        // replaced by install, never leaving two files.
        let blocked = outcome(watch, "normal", "D903502.tmp", &states[2].1);
        assert_eq!(blocked[1]["result"], "conflict");
        let replaced = outcome(watch, "normal", "D903502.tmp", &states[0].1);
        assert_eq!(replaced[1]["result"], "ok");
        assert_eq!(
            replaced[1]["watch"].as_array().unwrap().len(),
            1,
            "never two files"
        );
    }
}

/// Install and restore write a part under the file name the watch declares in
/// `GarminDevice.xml` (`Font::file_name`): on a watch declaring part numbers, a waiting copy
/// under the short name moves to the long one; on a watch declaring short names, as the Lily 2
/// Active does, a long-named copy moves to the short one. A file that is neither Garmin's nor
/// ours stays as it is under either name, and nothing is written.
#[test]
fn install_and_restore_write_the_name_the_watch_declares() {
    let part = "006-D9035-02";
    let (stock, georgian) = (fake::stock(part), fake::georgian(part));
    let mut unknown = stock.clone();
    unknown[7] ^= 0xff;
    let watches: [(MakeWatch, &str, &str); 2] = [
        (Watch::venu2, "006-D9035-02.tmp", "D903502.tmp"),
        (
            Watch::declaring_short_names,
            "D903502.tmp",
            "006-D9035-02.tmp",
        ),
    ];
    let run = |watch: &Watch, operation: &str| {
        call(&mut session(watch), operation, json!({"parts": [part]}))
    };
    for (watch, declared, other) in watches {
        // Nothing there: install writes the declared name.
        let w = watch();
        assert_eq!(run(&w, "install").unwrap()["status"], "staged_verified");
        assert_eq!(w.listed(), [(declared.to_owned(), georgian.clone())]);
        // Already staged under the declared name: nothing is written.
        let out = run(&w, "install").unwrap();
        assert_eq!(out["status"], "already_staged", "{declared}");
        assert_eq!(w.listed(), [(declared.to_owned(), georgian.clone())]);

        for (operation, before, after) in [
            // An identical copy under the other name form moves to the declared name.
            ("install", &georgian, &georgian),
            ("restore_stock", &stock, &stock),
            // Garmin's original or ours under the other form is replaced under the declared name.
            ("install", &stock, &georgian),
            ("restore_stock", &georgian, &stock),
        ] {
            let w = watch();
            w.add_file(other, before.clone());
            let out = run(&w, operation).unwrap();
            assert_eq!(out["status"], "staged_verified", "{declared} {operation}");
            assert_eq!(
                w.listed(),
                [(declared.to_owned(), after.clone())],
                "{declared} {operation}"
            );
        }

        // Neither Garmin's nor ours, under either name: refused, nothing written.
        for name in [declared, other] {
            let w = watch();
            w.add_file(name, unknown.clone());
            for operation in ["install", "restore_stock"] {
                let error = run(&w, operation).unwrap_err();
                assert_eq!(error.code, ErrorCode::Conflict, "{name} {operation}");
            }
            assert_eq!(w.listed(), [(name.to_owned(), unknown.clone())]);
        }
    }
}

#[test]
fn two_waiting_files_for_one_part_are_named_in_the_error() {
    let watch = Watch::venu2();
    watch.add_file("006-D9035-02.tmp", fake::stock("006-D9035-02"));
    watch.add_file("D903502.tmp", fake::stock("006-D9035-02"));
    let mut s = session(&watch);
    for operation in ["install", "restore_stock"] {
        let error = call(&mut s, operation, json!({})).unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
        assert!(
            error.message.contains(
                "GARMIN/ExtData/006-D9035-02.tmp and GARMIN/ExtData/D903502.tmp are both \
                 waiting files of part 006-D9035-02"
            ),
            "{}",
            error.message
        );
    }
    assert_eq!(watch.listed().len(), 2, "nothing was written");
    let out = call(&mut s, "inspect", json!({})).unwrap();
    let file = out["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["part"] == "006-D9035-02")
        .unwrap()["file"]
        .clone();
    assert_eq!(file, "other");
}

#[test]
fn a_failed_rename_puts_the_short_named_file_back() {
    let watch = Watch::venu2();
    watch.add_file("D903502.tmp", fake::stock("006-D9035-02"));
    // The new file under this build's name reads back wrong: it is deleted and the short-named
    // original written back.
    watch.state().corrupt_create_at = Some(1);
    let mut s = session(&watch);
    let error = call(&mut s, "install", json!({"parts": ["006-D9035-02"]})).unwrap_err();
    assert_eq!(error.code, ErrorCode::VerificationFailed);
    assert!(!error.rollback_incomplete(), "{}", error.message);
    assert_eq!(
        watch.listed(),
        [("D903502.tmp".to_owned(), fake::stock("006-D9035-02"))]
    );
}

#[cfg(not(feature = "dev"))]
#[test]
fn development_operations_are_absent_without_the_dev_feature() {
    let dev = [
        "read_file",
        "stage_file",
        "remove_staged",
        "mtp_device_info",
        "mtp_storage_ids",
        "mtp_storage_info",
        "mtp_handles",
        "mtp_object_info",
        "mtp_read",
        "ptp_transaction",
        "mtp_create",
        "mtp_delete",
        "mtp_trace",
    ];
    for name in dev {
        assert!(ops::operation(name).is_none(), "{name}");
        let watch = Watch::venu2();
        let error = call(&mut session(&watch), name, json!({})).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert!(
            error.message.contains("unknown operation"),
            "{}",
            error.message
        );
    }
    let tools = ops::tools().to_string();
    assert!(dev.iter().all(|name| !tools.contains(name)), "{tools}");
    assert!(!tools.contains("notifications/trace"));
}

#[cfg(feature = "dev")]
mod dev {
    use super::*;
    use fake::CJK;

    fn dev_session(watch: &Watch) -> (Session<'static>, std::path::PathBuf) {
        let log = temp_path("staged.jsonl");
        (session(watch).with_staged_log(log.clone()), log)
    }

    fn source(bytes: &[u8]) -> String {
        let path = temp_path("font.tmp");
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn cjk_garmin() -> Value {
        json!([sha256_hex(&fake::cjk_stock())])
    }

    #[test]
    fn stage_file_checks_the_part_and_what_it_would_replace() {
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        let font = source(b"experimental CJK host");
        // The imported D844870.fnt is unknown without Garmin's hash.
        let input = json!({"source": font, "name": "D844870.tmp", "dry_run": true});
        assert_eq!(code(call(&mut s, "stage_file", input)), ErrorCode::Conflict);

        let input = json!({"source": font, "name": "D844870.tmp", "dry_run": true,
                           "garmin_sha256": cjk_garmin()});
        let out = call(&mut s, "stage_file", input).unwrap();
        assert_eq!(out["status"], "dry_run");
        assert_eq!(out["action"], "create");
        assert_eq!(out["part"], CJK);
        assert_eq!(out["id"], Value::Null);
        assert!(watch.listed().is_empty());

        // The 27 px Armenian part's import is Garmin's (by this build's fonts).
        let input = json!({"source": font, "name": "006-D9035-03.tmp", "dry_run": true});
        assert_eq!(
            call(&mut s, "stage_file", input).unwrap()["action"],
            "create"
        );

        for (name, expected) in [
            ("D999999.tmp", ErrorCode::UnsupportedDevice),
            ("../D844870.tmp", ErrorCode::InvalidInput),
            ("D844870.fnt", ErrorCode::InvalidInput),
        ] {
            let input = json!({"source": font, "name": name, "dry_run": true});
            assert_eq!(code(call(&mut s, "stage_file", input)), expected, "{name}");
        }
        let input = json!({"source": "/nonexistent/font", "name": "D844870.tmp"});
        assert_eq!(
            code(call(&mut s, "stage_file", input)),
            ErrorCode::InvalidInput
        );
        assert!(watch.listed().is_empty());
        assert!(!watch.state().log.iter().any(|o| o.starts_with("Send")));
    }

    #[test]
    fn stage_file_and_remove_staged_round_trip() {
        let watch = Watch::venu2();
        let (mut s, log) = dev_session(&watch);
        let bytes = b"experimental CJK host".to_vec();
        let font = source(&bytes);
        let input = json!({"source": font, "name": "D844870.tmp", "garmin_sha256": cjk_garmin()});
        let recorder = Recorder::new(None);
        let out = recorder.run(&mut s, "stage_file", input.clone()).unwrap();
        assert_eq!(out["status"], "staged_verified");
        assert_eq!(out["sha256"], sha256_hex(&bytes));
        assert_eq!(recorder.progress().last(), Some(&(2.0, Some(2.0))));
        let id = out["id"].as_str().unwrap().to_owned();
        assert_eq!(watch.listed(), [("D844870.tmp".to_owned(), bytes.clone())]);

        let again = call(&mut s, "stage_file", input).unwrap();
        assert_eq!(again["status"], "already_staged");
        assert_eq!(again["id"], id.as_str());

        // Another session, as a later one-shot call would be, removes it by id.
        let mut other = session(&watch).with_staged_log(log.clone());
        let out = call(&mut other, "remove_staged", json!({"id": id})).unwrap();
        assert_eq!(out["status"], "removed");
        assert!(watch.listed().is_empty());
        let out = call(&mut other, "remove_staged", json!({"id": id})).unwrap();
        assert_eq!(out["status"], "already_removed");
        let input = json!({"id": "stg-000000000000"});
        assert_eq!(
            code(call(&mut other, "remove_staged", input)),
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn stage_file_replaces_only_files_it_can_identify() {
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        let garmin = cjk_garmin();
        let first = source(b"first try");
        let second = source(b"second try");
        let stage = |s: &mut Session<'_>, font: &str| {
            let input = json!({"source": font, "name": "D844870.tmp", "garmin_sha256": garmin});
            call(s, "stage_file", input)
        };
        let id = stage(&mut s, &first).unwrap()["id"].clone();
        let out = stage(&mut s, &second).unwrap();
        assert_eq!(out["action"], "replace");
        assert_eq!(out["replaces"]["identified_as"], "staged");
        assert_ne!(out["id"], id);
        assert_eq!(
            watch.listed(),
            [("D844870.tmp".to_owned(), b"second try".to_vec())]
        );

        // The first id's file was replaced: its id counts as removed, and nothing is deleted.
        let out = call(&mut s, "remove_staged", json!({"id": id})).unwrap();
        assert_eq!(out["status"], "already_removed");
        assert_eq!(watch.listed().len(), 1);
    }

    #[test]
    fn stage_file_refuses_unknown_and_competing_files() {
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        let font = source(b"font");
        let garmin = cjk_garmin();
        watch.add_file("D844870.tmp", b"someone else's".to_vec());
        let input = json!({"source": font, "name": "D844870.tmp", "garmin_sha256": garmin});
        assert_eq!(
            code(call(&mut s, "stage_file", input.clone())),
            ErrorCode::Conflict
        );
        // An unknown waiting file under the part's other name: refused just the same.
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        watch.add_file("006-D8448-70.tmp", b"someone else's".to_vec());
        assert_eq!(
            code(call(&mut s, "stage_file", input.clone())),
            ErrorCode::Conflict
        );
        // The same bytes under the other name: moved to the name asked for, never two files.
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        watch.add_file("006-D8448-70.tmp", b"font".to_vec());
        let out = call(&mut s, "stage_file", input).unwrap();
        assert_eq!(out["replaces"]["identified_as"], "same");
        assert_eq!(
            watch.listed(),
            [("D844870.tmp".to_owned(), b"font".to_vec())]
        );
    }

    #[test]
    fn remove_staged_leaves_a_changed_file() {
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        let font = source(b"font");
        let input = json!({"source": font, "name": "006-D9035-02.tmp"});
        let id = call(&mut s, "stage_file", input).unwrap()["id"].clone();
        assert_eq!(watch.listed().len(), 1);
        // Something else replaces it under the same name.
        let mut dev = watch.connect().unwrap();
        for handle in dev.object_handles(fake::STORAGE, fake::EXTDATA).unwrap() {
            dev.delete_object(handle).unwrap();
        }
        drop(dev);
        watch.add_file("006-D9035-02.tmp", b"other".to_vec());
        let error = call(&mut s, "remove_staged", json!({"id": id})).unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
        assert_eq!(watch.listed().len(), 1);
    }

    #[test]
    fn read_file_copies_from_either_listing() {
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        let path = temp_path("D844870.fnt");
        let input = json!({"name": "d844870.fnt", "path": path.to_str().unwrap()});
        let out = call(&mut s, "read_file", input.clone()).unwrap();
        assert_eq!(out["hidden"], true);
        assert_eq!(out["sha256"], sha256_hex(&fake::cjk_stock()));
        assert_eq!(std::fs::read(&path).unwrap(), fake::cjk_stock());
        // An existing local file is never overwritten.
        assert_eq!(
            code(call(&mut s, "read_file", input)),
            ErrorCode::InvalidInput
        );
        let input = json!({"name": "missing.fnt", "path": temp_path("x").to_str().unwrap()});
        assert_eq!(
            code(call(&mut s, "read_file", input)),
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn a_cancelled_stage_file_writes_nothing() {
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        let recorder = Recorder::new(Some(0.0));
        let input = json!({"source": source(b"font"), "name": "006-D9035-02.tmp"});
        let error = recorder.run(&mut s, "stage_file", input).unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(watch.listed().is_empty());
    }

    #[test]
    fn stage_file_checks_imported_fonts_in_the_normal_listing() {
        for name in ["006-D9035-02.fnt", "D903502.fnt"] {
            let watch = Watch::venu2();
            let (mut s, _) = dev_session(&watch);
            watch.add_file(name, b"an import this build doesn't know".to_vec());
            let input = json!({"source": source(b"font"), "name": "006-D9035-02.tmp"});
            let error = call(&mut s, "stage_file", input).unwrap_err();
            assert_eq!(error.code, ErrorCode::Conflict, "{name}");
            assert!(error.message.contains(name), "{}", error.message);
            assert_eq!(watch.listed().len(), 1, "nothing was written");
        }
        // Garmin's own, by this build's fonts: staged.
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        watch.add_file("D903502.fnt", fake::stock("006-D9035-02"));
        let input = json!({"source": source(b"font"), "name": "006-D9035-02.tmp"});
        assert_eq!(
            call(&mut s, "stage_file", input).unwrap()["status"],
            "staged_verified"
        );
    }

    #[test]
    fn a_failed_stage_file_says_when_its_cleanup_failed() {
        let watch = Watch::venu2();
        {
            let mut state = watch.state();
            state.corrupt_create_at = Some(1);
            state.fail_delete = true;
        }
        let (mut s, _) = dev_session(&watch);
        let input = json!({"source": source(b"font"), "name": "006-D9035-02.tmp"});
        let error = call(&mut s, "stage_file", input).unwrap_err();
        assert_eq!(error.code, ErrorCode::VerificationFailed);
        assert_eq!(left(&error), ["GARMIN/ExtData/006-D9035-02.tmp"]);
        assert!(error.message.contains("remove_staged"), "{}", error.message);
    }

    /// `stage_file` under the short name, then install or restore of that part: an experiment
    /// this build can't identify blocks both; a file it can identify is replaced under this
    /// build's name. A part never ends up with two waiting files.
    #[test]
    fn a_short_name_staged_file_meets_install_and_restore() {
        let part = "006-D9035-02";
        let experiment = b"an experimental host font".to_vec();
        for operation in ["install", "restore_stock"] {
            let watch = Watch::venu2();
            let (mut s, _) = dev_session(&watch);
            let input = json!({"source": source(&experiment), "name": "D903502.tmp"});
            call(&mut s, "stage_file", input).unwrap();
            let error = call(&mut s, operation, json!({"parts": [part]})).unwrap_err();
            assert_eq!(error.code, ErrorCode::Conflict, "{operation}");
            assert!(error.message.contains("D903502.tmp"), "{}", error.message);
            assert_eq!(
                watch.listed(),
                [("D903502.tmp".to_owned(), experiment.clone())]
            );
        }
        for (operation, staged, expected) in [
            ("install", fake::stock(part), fake::georgian(part)),
            ("restore_stock", fake::georgian(part), fake::stock(part)),
        ] {
            let watch = Watch::venu2();
            let (mut s, _) = dev_session(&watch);
            let input = json!({"source": source(&staged), "name": "D903502.tmp"});
            call(&mut s, "stage_file", input).unwrap();
            let out = call(&mut s, operation, json!({"parts": [part]})).unwrap();
            assert_eq!(out["status"], "staged_verified", "{operation}");
            assert_eq!(
                watch.listed(),
                [(format!("{part}.tmp"), expected)],
                "{operation}"
            );
        }
    }

    /// Waiting files of a part under the other name form are checked like the one under name.
    #[test]
    fn stage_file_handles_the_other_name_form_like_its_own() {
        let part = "006-D9035-02";
        // Garmin's own under the long name: replaced by the short-named stage.
        let watch = Watch::venu2();
        let (mut s, log) = dev_session(&watch);
        watch.add_file("006-D9035-02.tmp", fake::stock(part));
        let input = json!({"source": source(b"font"), "name": "D903502.tmp"});
        let out = call(&mut s, "stage_file", input).unwrap();
        assert_eq!(out["action"], "replace");
        assert_eq!(out["replaces"]["identified_as"], "garmin");
        assert_eq!(
            watch.listed(),
            [("D903502.tmp".to_owned(), b"font".to_vec())]
        );
        // remove_staged finds it by part and contents.
        let id = out["id"].clone();
        let mut other = session(&watch).with_staged_log(log);
        let out = call(&mut other, "remove_staged", json!({"id": id})).unwrap();
        assert_eq!(out["status"], "removed");
        assert!(watch.listed().is_empty());

        // An unknown one under the long name: refused, whichever name is asked for.
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        watch.add_file("006-D9035-02.tmp", b"someone else's".to_vec());
        for name in ["D903502.tmp", "006-D9035-02.tmp"] {
            let input = json!({"source": source(b"font"), "name": name});
            assert_eq!(
                code(call(&mut s, "stage_file", input)),
                ErrorCode::Conflict,
                "{name}"
            );
        }

        // Two waiting files: named in the error.
        let watch = Watch::venu2();
        let (mut s, _) = dev_session(&watch);
        watch.add_file("006-D9035-02.tmp", fake::stock(part));
        watch.add_file("D903502.tmp", fake::stock(part));
        let input = json!({"source": source(b"font"), "name": "D903502.tmp"});
        let error = call(&mut s, "stage_file", input).unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
        assert!(
            error.message.contains("all wait for part"),
            "{}",
            error.message
        );

        // Imported short-name fonts, in either listing, unknown: refused.
        for hidden in [false, true] {
            let watch = Watch::venu2();
            let (mut s, _) = dev_session(&watch);
            let data = b"an unknown import".to_vec();
            if hidden {
                watch.add_hidden("D903502.fnt", data);
            } else {
                watch.add_file("D903502.fnt", data);
            }
            let input = json!({"source": source(b"font"), "name": "006-D9035-02.tmp"});
            assert_eq!(
                code(call(&mut s, "stage_file", input)),
                ErrorCode::Conflict,
                "{hidden}"
            );
        }
    }
}

/// An `rpc` session on a fake watch: lines in through a pipe, lines out through a channel.
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
            let value = serde_json::from_slice(&line).expect("one JSON message per line");
            let _ = self.0.send(value);
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
            let worker = Worker::spawn(move || session(&watch));
            rpc::serve(BufReader::new(reader), LineSink(tx, Vec::new()), worker);
        });
        Rpc {
            input: Some(writer),
            output,
            server: Some(server),
        }
    }

    fn send(&mut self, line: &str) {
        let input = self.input.as_mut().unwrap();
        input.write_all(line.as_bytes()).unwrap();
        input.write_all(b"\n").unwrap();
    }

    fn next(&self) -> Value {
        self.output
            .recv_timeout(Duration::from_secs(10))
            .expect("a message")
    }

    /// Sends a request and returns its response, skipping other messages.
    fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send(
            &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string(),
        );
        loop {
            let message = self.next();
            if message["id"] == id {
                return message;
            }
        }
    }

    /// Ends the input and returns the messages that came after.
    fn close(mut self) -> Vec<Value> {
        drop(self.input.take());
        self.server.take().unwrap().join().unwrap();
        self.output.try_iter().collect()
    }
}

#[test]
fn rpc_answers_a_malformed_line_and_goes_on() {
    let watch = Watch::venu2();
    let mut rpc = Rpc::start(&watch);
    rpc.send("{not json");
    let error = rpc.next();
    assert_eq!(error["jsonrpc"], "2.0");
    assert_eq!(error["id"], Value::Null);
    assert_eq!(error["error"]["code"], rpc::PARSE_ERROR);
    assert_eq!(error["error"]["data"]["code"], "invalid_input");
    rpc.send("[1, 2]");
    assert_eq!(rpc.next()["error"]["code"], rpc::INVALID_REQUEST);
    rpc.send(r#"{"jsonrpc": "1.0", "id": 7, "method": "inspect"}"#);
    let error = rpc.next();
    assert_eq!(
        (error["id"].clone(), error["error"]["code"].clone()),
        (json!(7), json!(rpc::INVALID_REQUEST))
    );
    // A notification gets no answer; the next request does.
    rpc.send(r#"{"jsonrpc": "2.0", "method": "notifications/initialized"}"#);

    let out = rpc.request(1, "inspect", json!({"_meta": {"client": "test"}}));
    assert_eq!(out["result"]["device"]["model"], "Venu 2");
    let out = rpc.request(2, "no_such_method", json!({}));
    assert_eq!(out["error"]["code"], rpc::METHOD_NOT_FOUND);
    let out = rpc.request(3, "extdata_list", json!({"limit": 0}));
    assert_eq!(out["error"]["code"], rpc::INVALID_PARAMS);
    assert_eq!(out["error"]["data"]["code"], "invalid_input");
    let out = rpc.request(4, "extdata_list", json!([]));
    assert_eq!(out["error"]["code"], rpc::INVALID_PARAMS);
    if !cfg!(feature = "dev") {
        let out = rpc.request(5, "stage_file", json!({}));
        assert_eq!(out["error"]["code"], rpc::METHOD_NOT_FOUND);
    }
    assert!(rpc.close().is_empty());
}

#[test]
fn rpc_reports_no_device_while_unplugged_and_reconnects() {
    let watch = Watch::venu2();
    let mut rpc = Rpc::start(&watch);
    assert!(rpc.request(1, "inspect", json!({}))["result"].is_object());
    watch.unplug();
    let out = rpc.request(2, "inspect", json!({}));
    assert_eq!(out["error"]["code"], rpc::OPERATION_FAILED);
    assert_eq!(out["error"]["data"]["code"], "no_device");
    watch.plug();
    assert!(rpc.request(3, "inspect", json!({}))["result"].is_object());
    rpc.close();
    assert_eq!(watch.state().connects, 2);
}

#[test]
fn rpc_sends_progress_and_stops_on_cancel() {
    let watch = Watch::venu2();
    let mut rpc = Rpc::start(&watch);
    let out = rpc.request(
        1,
        "install",
        json!({"parts": ["006-D9035-01"], "_meta": {"progressToken": "t1"}}),
    );
    assert_eq!(out["result"]["status"], "staged_verified");

    watch.state().write_delay = Duration::from_millis(300);
    rpc.send(
        &json!({"jsonrpc": "2.0", "id": "two", "method": "restore_stock",
                "params": {"_meta": {"progressToken": 9}}})
        .to_string(),
    );
    let mut steps = Vec::new();
    let response = loop {
        let message = rpc.next();
        if message["method"] == "notifications/progress" {
            assert_eq!(message["params"]["progressToken"], 9);
            assert_eq!(message["params"]["total"], 4.0);
            let progress = message["params"]["progress"].as_f64().unwrap();
            steps.push(progress);
            if progress == 1.0 {
                rpc.send(r#"{"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": "two", "reason": "test"}}"#);
            }
            continue;
        }
        break message;
    };
    assert_eq!(response["id"], "two");
    assert_eq!(response["error"]["code"], rpc::REQUEST_CANCELLED);
    assert_eq!(response["error"]["data"]["code"], "cancelled");
    assert!(steps.starts_with(&[0.0, 1.0]), "{steps:?}");
    // Restore wrote stock over the Georgian part (put back) and created others (deleted).
    assert_eq!(
        watch.listed(),
        [(
            "006-D9035-01.tmp".to_owned(),
            fake::georgian("006-D9035-01")
        )]
    );
    rpc.close();
}

/// Every progress update of a call comes before its response: the worker writes both, in that
/// order, on its own thread. Many calls back to back, each checked in full.
#[test]
fn rpc_sends_all_of_a_calls_progress_before_its_response() {
    let watch = Watch::venu2();
    let mut rpc = Rpc::start(&watch);
    for id in 1..=40 {
        let method = if id % 2 == 1 {
            "install"
        } else {
            "restore_stock"
        };
        rpc.send(
            &json!({"jsonrpc": "2.0", "id": id, "method": method,
                    "params": {"_meta": {"progressToken": id}}})
            .to_string(),
        );
        let mut steps = Vec::new();
        let response = loop {
            let message = rpc.next();
            if message["method"] == "notifications/progress" {
                assert_eq!(message["params"]["progressToken"], id);
                steps.push(message["params"]["progress"].as_f64().unwrap());
                continue;
            }
            break message;
        };
        assert_eq!(response["id"], id);
        assert!(response["result"].is_object(), "{response}");
        assert_eq!(steps, [0.0, 1.0, 2.0, 3.0, 4.0], "call {id}");
    }
    assert!(rpc.close().is_empty(), "nothing after the last response");
}

#[test]
fn rpc_answers_what_was_read_when_input_ends() {
    let watch = Watch::venu2();
    let mut rpc = Rpc::start(&watch);
    for id in 1..=3 {
        rpc.send(&json!({"jsonrpc": "2.0", "id": id, "method": "inspect"}).to_string());
    }
    let rest = rpc.close();
    let ids: Vec<&Value> = rest.iter().map(|m| &m["id"]).collect();
    assert_eq!(ids, [&json!(1), &json!(2), &json!(3)], "answered in order");
    assert!(rest.iter().all(|m| m["result"].is_object()));
}
