//! The app's new UI (`crates/app/frontend`) is built and tested against what the core really
//! returns. Each scenario of the UI contract's flows (`crates/app/frontend/CONTRACT.md` §8) runs
//! here through the real operations, on a fake watch built from a watch report fixture
//! (`support/report_watch.rs`), and each step's result goes to
//! `crates/app/frontend/fixtures/core/<scenario>/<step>.json`:
//! `{"operation", "input"?, "output"}` or `{"operation", "input"?, "error"}`, the error as the app
//! hands it to the frontend (`Error::to_json`'s `error`).
//!
//! With `GFG_WRITE_UI_FIXTURES=1` the files are written; otherwise they are compared and any
//! drift fails, naming the file. The machine and the app version the reports name are replaced
//! by fixed ones (macOS 15.6.1 on aarch64, app 0.3.4), and the report text rebuilt with them, so
//! the files are the same on every machine and release.
//!
//! Statuses come from the examples' support list (`example_watches`), not the registry's, so a
//! watch confirmed in the registry changes none of these files. Real models appear only with a
//! status that can't change (tested on a software version); every other status is shown on a
//! role watch: a report's listing under a made-up model and part number ([`as_role`]).
//!
//! What a fake can't produce is composed, and says so where it is:
//! - a restart that doesn't import is the watch left as it was;
//! - the Venu 3's other font waiting (#2's report listed nothing) is a `.tmp` added to its
//!   listing that is neither Garmin's nor ours;
//! - the Venu 1st gen's hidden listing (#12's report has none), with its Korean `-22` imported as
//!   Garmin's original, and the Instinct 3's `-07` imported as our build (#57's report is from
//!   before installing);
//! - busy holders and connect problems are the errors the connection would return, built with
//!   the core's own `Error` builders (the holders as the demo build names them); the problems'
//!   messages are written here after the core's;
//! - `verification_failed` and `no_device` during a write are built with `Error::with_rollback`;
//! - a watch in Garmin mode is the connection failing with `Error::garmin_mode`, and
//!   `switch_to_mtp` the session's switch answering as the Venu 2 did (or, for its errors,
//!   failing with the core's messages, written here).

#[path = "support/report_watch.rs"]
mod report_watch;

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use georgian_for_garmin_core::busy::{Busy, Holder, HolderKind, System, UsbInterface};
use georgian_for_garmin_core::error::{RollbackAction, RollbackFile};
use georgian_for_garmin_core::example_watches::{self, Role};
use georgian_for_garmin_core::fonts::FontSet;
use georgian_for_garmin_core::garmin_mode::{
    GarminMode, NotSwitchable, Product, SwitchOutput, SwitchStatus,
};
use georgian_for_garmin_core::ops;
use georgian_for_garmin_core::report::{self, IssueLinkInput, WatchReport};
use georgian_for_garmin_core::session::{Control, Session};
use georgian_for_garmin_core::{Error, ErrorCode};
use report_watch::*;
use serde_json::{Value, json};

/// The app version and machine every fixture's report names.
const APP_VERSION: &str = "0.3.4";
fn platform() -> Value {
    json!({"os": "Mac OS", "os_version": "15.6.1", "arch": "aarch64"})
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/frontend/fixtures/core")
}

/// Every scenario this test writes; another directory under `fixtures/core` is stale.
const SCENARIOS: &[&str] = &[
    "venu2",
    "venu2-second-restart",
    "venu2-write-fails",
    "unconfirmed",
    "korean-boxes",
    "korean-unused",
    "expected-waiting",
    "font-unused",
    "font-unused-beta-left",
    "drive-only",
    "venu2-drive",
    "predicted",
    "other-software",
    "busy",
    "connect-problems",
    "errors",
    "garmin-mode",
];

/// The watch a report fixture describes, with the examples' fonts and support list.
fn watch_of(fixture: &Value) -> (Watch, FontSet) {
    Watch::from_fixture_with(fixture, example_watches::fonts())
}

/// `fixture`'s listing on the role watch `role`: its model and part number made up.
fn as_role(mut fixture: Value, role: Role) -> Value {
    fixture["model"] = json!(role.model);
    fixture["model_part"] = json!(role.model_part);
    fixture
}

/// A report's machine and app version replaced by the fixed ones, and its text rebuilt.
fn normalize_report(report: &mut Value) {
    report["app_version"] = json!(APP_VERSION);
    report["platform"] = platform();
    let parsed: WatchReport = serde_json::from_value(report.clone()).expect("a WatchReport");
    report["text"] = json!(report::text(&parsed, None, None));
}

struct Scenario {
    name: &'static str,
    steps: Vec<(String, Value)>,
    fonts: FontSet,
}

impl Scenario {
    fn new(name: &'static str, fonts: FontSet) -> Self {
        assert!(
            SCENARIOS.contains(&name),
            "{name} is missing from SCENARIOS"
        );
        Scenario {
            name,
            steps: Vec::new(),
            fonts,
        }
    }

    /// Records `result` of `operation` as `step`; returns the output, or the error.
    fn record(
        &mut self,
        step: &str,
        operation: &str,
        input: Option<Value>,
        result: Result<Value, Error>,
    ) -> Result<Value, Error> {
        assert!(
            !self.steps.iter().any(|(s, _)| s == step),
            "{}: step {step} twice",
            self.name
        );
        let mut file = json!({ "operation": operation });
        if let Some(input) = input {
            file["input"] = input;
        }
        match &result {
            Ok(output) => {
                let mut output = output.clone();
                if output.get("report").is_some() {
                    normalize_report(&mut output["report"]);
                }
                if operation == "inspect_drive" {
                    output["path"] = json!("/Volumes/GARMIN");
                }
                file["output"] = output;
            }
            Err(e) => file["error"] = e.to_json()["error"].clone(),
        }
        self.steps.push((step.to_owned(), file));
        result
    }

    /// `operation` on `watch`, which must succeed.
    fn ok(&mut self, step: &str, operation: &str, watch: &Watch) -> Value {
        let result = watch.call(operation, &self.fonts);
        let name = self.name;
        self.record(step, operation, None, result)
            .unwrap_or_else(|e| panic!("{name}: {step}: {}: {}", e.code.as_str(), e.message))
    }

    /// `operation` on `watch`, which must fail with `code`.
    fn fails(&mut self, step: &str, operation: &str, watch: &Watch, code: ErrorCode) -> Error {
        let result = watch.call(operation, &self.fonts);
        self.expect_err(step, operation, None, result, code)
    }

    /// `operation` with the connection failing as `error`.
    fn connect_fails(&mut self, step: &str, operation: &str, error: Error) -> Error {
        let code = error.code;
        let result = ops::call(
            operation,
            json!({}),
            &self.fonts,
            &mut || Err(error.clone()),
        );
        self.expect_err(step, operation, None, result, code)
    }

    /// An error the operation would return, built here (see the module's notes).
    fn composed_error(&mut self, step: &str, operation: &str, error: Error) {
        self.steps.push((
            step.to_owned(),
            json!({ "operation": operation, "error": error.to_json()["error"] }),
        ));
    }

    fn expect_err(
        &mut self,
        step: &str,
        operation: &str,
        input: Option<Value>,
        result: Result<Value, Error>,
        code: ErrorCode,
    ) -> Error {
        let name = self.name;
        match self.record(step, operation, input, result) {
            Ok(_) => panic!("{name}: {step} succeeded"),
            Err(e) => {
                assert_eq!(e.code, code, "{name}: {step}: {}", e.message);
                e
            }
        }
    }

    /// `issue_link` for `report` (inspect's or inspect_drive's) with the last operation's
    /// `outcome` and the `answers`.
    fn issue_link(&mut self, step: &str, input: Value) -> Value {
        let mut input = input;
        if input.get("report").is_some() {
            normalize_report(&mut input["report"]);
        }
        // The fixed app and machine, for a report without a watch report (busy alone); with one,
        // the report says them itself.
        let parsed: IssueLinkInput =
            serde_json::from_value(input.clone()).expect("an IssueLinkInput");
        let fixed: report::Platform = serde_json::from_value(platform()).unwrap();
        let result = report::issue_link_on(&parsed, APP_VERSION, &fixed)
            .map(|link| serde_json::to_value(link).unwrap());
        let name = self.name;
        self.record(step, "issue_link", Some(input), result)
            .unwrap_or_else(|e| panic!("{name}: {step}: {}", e.message))
    }

    /// Writes the files (`GFG_WRITE_UI_FIXTURES=1`) or checks them.
    fn finish(self) {
        let dir = root().join(self.name);
        let expected: BTreeSet<String> = self
            .steps
            .iter()
            .map(|(s, _)| format!("{s}.json"))
            .collect();
        if std::env::var_os("GFG_WRITE_UI_FIXTURES").is_some_and(|v| v == "1") {
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            for (step, file) in &self.steps {
                let text = serde_json::to_string_pretty(file).unwrap() + "\n";
                fs::write(dir.join(format!("{step}.json")), text).unwrap();
            }
            return;
        }
        let mut drift = Vec::new();
        for (step, file) in &self.steps {
            let path = dir.join(format!("{step}.json"));
            let want = serde_json::to_string_pretty(file).unwrap() + "\n";
            match fs::read_to_string(&path) {
                Ok(have) if have == want => {}
                Ok(_) => drift.push(format!("{} differs", path.display())),
                Err(e) => drift.push(format!("{}: {e}", path.display())),
            }
        }
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !expected.contains(&name) {
                drift.push(format!("{} is stale", entry.path().display()));
            }
        }
        assert!(
            drift.is_empty(),
            "UI fixtures drifted; regenerate with GFG_WRITE_UI_FIXTURES=1 cargo test -p \
             georgian-for-garmin-core --test ui_fixtures:\n{}",
            drift.join("\n")
        );
    }
}

/// The last install's or restore's outcome for `issue_link`, from its output.
fn outcome(operation: &str, output: &Value) -> Value {
    let parts: Vec<&Value> = output["packs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["parts"].as_array().unwrap())
        .map(|p| &p["part"])
        .collect();
    json!({"operation": operation, "status": output["status"], "parts": parts})
}

fn answers(result: &str) -> Value {
    match result {
        "georgian" => json!({"result": "georgian", "views": ["popup", "list"], "phone": "android"}),
        other => json!({"result": other, "views": [], "phone": "android"}),
    }
}

fn state(output: &Value) -> &str {
    output["state"].as_str().unwrap()
}

#[test]
fn scenarios_are_all_listed() {
    for entry in fs::read_dir(root()).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            SCENARIOS.contains(&name.as_str()),
            "fixtures/core/{name} is no scenario of ui_fixtures.rs"
        );
    }
}

/// Venu 2, confirmed: install → restart → verified; then restore → restart → Garmin's font.
#[test]
fn venu2() {
    let (watch, fonts) = watch_of(&fixture("maintainer-venu2-stock-imported.json"));
    let mut s = Scenario::new("venu2", fonts);
    let before = s.ok("inspect", "inspect", &watch);
    assert_eq!(state(&before), "not_installed");
    assert_eq!(before["support"]["status"], "tested");
    let installed = s.ok("install", "install", &watch);
    assert_eq!(state(&s.ok("inspect-staged", "inspect", &watch)), "waiting");
    watch.restart(&s.fonts);
    let after = s.ok("inspect-restarted", "inspect", &watch);
    assert_eq!(state(&after), "installed");
    s.issue_link(
        "issue_link-georgian",
        json!({"report": after["report"], "outcome": outcome("install", &installed), "answers": answers("georgian")}),
    );
    let restored = s.ok("restore_stock", "restore_stock", &watch);
    assert_eq!(
        state(&s.ok("inspect-restore-staged", "inspect", &watch)),
        "stock_waiting"
    );
    watch.restart(&s.fonts);
    let back = s.ok("inspect-restored", "inspect", &watch);
    assert_eq!(state(&back), "not_installed");
    s.issue_link(
        "issue_link-not_installed",
        json!({"report": back["report"], "outcome": outcome("restore_stock", &restored), "answers": answers("not_installed")}),
    );
    assert!(watch.0.borrow().violations.is_empty());
    s.finish();
}

/// Venu 2 with a restart that doesn't import: the watch stays as install left it (composed),
/// and the next restart imports.
#[test]
fn venu2_second_restart() {
    let (watch, fonts) = watch_of(&fixture("maintainer-venu2-stock-imported.json"));
    let mut s = Scenario::new("venu2-second-restart", fonts);
    s.ok("inspect", "inspect", &watch);
    s.ok("install", "install", &watch);
    assert_eq!(state(&s.ok("inspect-staged", "inspect", &watch)), "waiting");
    // The first restart didn't import: nothing on the watch changed.
    assert_eq!(
        state(&s.ok("inspect-not-imported", "inspect", &watch)),
        "waiting"
    );
    watch.restart(&s.fonts);
    assert_eq!(
        state(&s.ok("inspect-restarted", "inspect", &watch)),
        "installed"
    );
    s.finish();
}

/// Venu 2 whose second write fails: with the first file deleted again (rollback complete), and
/// with deletes failing too (incomplete); then restore once the watch answers again.
#[test]
fn venu2_write_fails() {
    let (watch, fonts) = watch_of(&fixture("maintainer-venu2-stock-imported.json"));
    let mut s = Scenario::new("venu2-write-fails", fonts);
    s.ok("inspect", "inspect", &watch);
    watch.fail_writes(1, false);
    let complete = s.fails(
        "install-rollback-complete",
        "install",
        &watch,
        ErrorCode::DeviceError,
    );
    assert!(!complete.rollback_incomplete());
    watch.0.borrow_mut().failure = None;
    assert_eq!(
        state(&s.ok("inspect-after-complete", "inspect", &watch)),
        "not_installed"
    );

    watch.fail_writes(1, true);
    let incomplete = s.fails(
        "install-rollback-incomplete",
        "install",
        &watch,
        ErrorCode::DeviceError,
    );
    assert!(incomplete.rollback_incomplete());
    // The watch answers again; what the failed install left is still there.
    watch.0.borrow_mut().failure = None;
    s.ok("inspect-after-incomplete", "inspect", &watch);
    s.ok("restore_stock", "restore_stock", &watch);
    s.ok("inspect-restore-staged", "inspect", &watch);
    watch.restart(&s.fonts);
    assert_eq!(
        state(&s.ok("inspect-restored", "inspect", &watch)),
        "not_installed"
    );
    s.finish();
}

/// The first confirmation, on a role watch with the Lily 2 Active's listing (short names): its
/// Korean imports read as Garmin's (composed as `the_lily_2_active_round_trips_its_short_names`
/// does), install under the short names, restart, check, share.
#[test]
fn unconfirmed() {
    let parts = ["006-D8945-67", "006-D8945-68"];
    let lily = imports_as_garmins(
        fixture("email-lily2active.json"),
        &FontSet::bundled(),
        &parts,
    );
    let (watch, fonts) = watch_of(&as_role(lily, example_watches::UNCONFIRMED));
    let mut s = Scenario::new("unconfirmed", fonts);
    let before = s.ok("inspect", "inspect", &watch);
    assert_eq!(state(&before), "not_installed");
    assert_eq!(before["support"]["status"], "expected");
    assert_eq!(before["support"]["prediction"], "bitmap_korean");
    let installed = s.ok("install", "install", &watch);
    assert_eq!(state(&s.ok("inspect-staged", "inspect", &watch)), "waiting");
    watch.restart(&s.fonts);
    let after = s.ok("inspect-restarted", "inspect", &watch);
    assert_eq!(state(&after), "installed");
    s.issue_link(
        "issue_link-georgian",
        json!({"report": after["report"], "outcome": outcome("install", &installed), "answers": answers("georgian")}),
    );
    assert!(watch.0.borrow().violations.is_empty());
    s.finish();
}

/// A role watch with the Venu 1st gen's listing (#12): install, restart, boxes; another restart;
/// boxes again, shared. Composed: its report (0.2.2) has no hidden listing, so the watch is
/// given one with its Korean `-22` imported as Garmin's original, as the Lily 2 Active's are.
#[test]
fn korean_boxes() {
    let mut venu = fixture("issue-12.json");
    let k22 = font_for(&FontSet::bundled(), "006-D8945-22")
        .unwrap()
        .clone();
    venu["hidden"] = json!({"status": "listed", "files": [
        {"name": "006-D8945-22.fnt", "bytes": k22.stock.len(), "read_only": true, "file": "stock"}
    ]});
    let (watch, fonts) = watch_of(&as_role(venu, example_watches::KOREAN));
    let mut s = Scenario::new("korean-boxes", fonts);
    let before = s.ok("inspect", "inspect", &watch);
    assert_eq!(before["support"]["status"], "expected");
    let installed = s.ok("install", "install", &watch);
    s.ok("inspect-staged", "inspect", &watch);
    watch.restart(&s.fonts);
    let after = s.ok("inspect-restarted", "inspect", &watch);
    assert_eq!(state(&after), "installed");
    watch.restart(&s.fonts);
    let again = s.ok("inspect-restarted-again", "inspect", &watch);
    assert_eq!(after, again);
    s.issue_link(
        "issue_link-boxes",
        json!({"report": again["report"], "outcome": outcome("install", &installed), "answers": answers("boxes")}),
    );
    assert!(watch.0.borrow().violations.is_empty());
    s.finish();
}

/// A role watch with the Instinct 3 AMOLED 45mm's listing (#57) after 0.3.4 installed its Korean
/// `-07` and the watch restarted (composed: the report is from before installing, so its `-07`
/// import is set to our build): unsupported, restorable; restore → restart → Garmin's font.
#[test]
fn korean_unused() {
    let mut i3 = fixture("issue-57.json");
    for file in i3["hidden"]["files"].as_array_mut().unwrap() {
        if file["name"] == "006-D8945-07.fnt" {
            file["file"] = json!("georgian");
        }
    }
    let (watch, fonts) = watch_of(&as_role(i3, example_watches::KOREAN_UNUSED));
    let mut s = Scenario::new("korean-unused", fonts);
    let out = s.ok("inspect", "inspect", &watch);
    assert_eq!(out["support"]["status"], "unsupported");
    assert_eq!(out["support"]["issue"], 57);
    assert_eq!(out["restorable"], true);
    s.ok("restore_stock", "restore_stock", &watch);
    s.ok("inspect-restore-staged", "inspect", &watch);
    watch.restart(&s.fonts);
    let back = s.ok("inspect-restored", "inspect", &watch);
    let k07 = |inspect: &Value| {
        inspect["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["part"] == "006-D8945-07")
            .map(|f| f["file"].clone())
    };
    assert_eq!(k07(&out), Some(json!("georgian")));
    assert_eq!(k07(&back), Some(json!("stock")));
    assert!(watch.0.borrow().violations.is_empty());
    s.finish();
}

/// A role watch with the Venu 3's listing (#2) and another font waiting (composed: #2's report
/// listed nothing, and its fonts are TrueType): a `.bin` for its first target part in the normal
/// listing, neither Garmin's nor ours, on a watch that keeps imported `.bin` fonts out of that
/// listing, as the Forerunner 165 does. Expected; blocked until a restart takes it up out of
/// sight, then install.
#[test]
fn expected_waiting() {
    let mut venu3 = as_role(fixture("issue-2.json"), example_watches::EXPECTED);
    let (probe, fonts) = watch_of(&venu3);
    let inspect = probe.call("inspect", &fonts).unwrap();
    assert_eq!(state(&inspect), "not_visible");
    assert_eq!(inspect["support"]["status"], "expected");
    let target = inspect["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["target"] == true)
        .expect("a target part")["part"]
        .as_str()
        .unwrap()
        .to_owned();
    venu3["extdata"] =
        json!([{"name": format!("{target}.bin"), "bytes": 41234, "sha256": null, "file": "other"}]);
    let (watch, fonts) = watch_of(&venu3);
    watch.0.borrow_mut().hides_bins = true;
    let mut s = Scenario::new("expected-waiting", fonts);
    assert_eq!(state(&s.ok("inspect", "inspect", &watch)), "other");
    watch.restart(&s.fonts);
    assert_eq!(
        state(&s.ok("inspect-restarted", "inspect", &watch)),
        "not_visible"
    );
    s.ok("install", "install", &watch);
    s.ok("inspect-staged", "inspect", &watch);
    watch.restart(&s.fonts);
    s.ok("inspect-installed", "inspect", &watch);
    s.finish();
}

/// A role watch with the fenix 6S Pro's listing (#6): unsupported; its font list shared.
#[test]
fn font_unused() {
    let (watch, fonts) = watch_of(&as_role(
        fixture("issue-6.json"),
        example_watches::FONT_UNUSED,
    ));
    let mut s = Scenario::new("font-unused", fonts);
    let out = s.ok("inspect", "inspect", &watch);
    assert_eq!(out["support"]["status"], "unsupported");
    s.issue_link("issue_link-font_list", json!({"report": out["report"]}));
    s.finish();
}

/// A role watch with the fenix 6S Pro's listing and an earlier test version's fonts left on it
/// (#6's comment): unsupported, restorable; restore → restart → Garmin's fonts.
#[test]
fn font_unused_beta_left() {
    let (watch, fonts) = watch_of(&as_role(
        fixture("issue-6-comment-7080207810.json"),
        example_watches::FONT_UNUSED,
    ));
    let mut s = Scenario::new("font-unused-beta-left", fonts);
    let out = s.ok("inspect", "inspect", &watch);
    assert_eq!(out["support"]["status"], "unsupported");
    assert_eq!(out["restorable"], true);
    s.ok("restore_stock", "restore_stock", &watch);
    s.ok("inspect-restore-staged", "inspect", &watch);
    watch.restart(&s.fonts);
    s.ok("inspect-restored", "inspect", &watch);
    assert!(watch.0.borrow().violations.is_empty());
    s.finish();
}

/// A watch volume in a temporary folder: its `GarminDevice.xml` and an `ExtData` listing.
fn drive_from(fixture: &Value, name: &str) -> PathBuf {
    let (watch, _) = watch_of(fixture);
    let dir = std::env::temp_dir().join(format!("gfg-ui-fixtures-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("GARMIN/ExtData")).unwrap();
    fs::write(
        dir.join("GARMIN/GarminDevice.xml"),
        watch.0.borrow().device_xml(),
    )
    .unwrap();
    for f in fixture["extdata"].as_array().into_iter().flatten() {
        let bytes = f["bytes"].as_u64().unwrap() as usize;
        fs::write(
            dir.join("GARMIN/ExtData").join(f["name"].as_str().unwrap()),
            vec![0u8; bytes],
        )
        .unwrap();
    }
    dir
}

/// No watch over MTP; `inspect_drive` reads the volume at `path`.
fn drive_scenario(name: &'static str, fixture: &Value) -> (Scenario, Value) {
    let mut s = Scenario::new(name, example_watches::fonts());
    s.connect_fails("inspect", "inspect", no_device());
    let dir = drive_from(fixture, name);
    let input = json!({"path": dir.display().to_string()});
    let result = ops::call("inspect_drive", input, &s.fonts, &mut || Err(no_device()));
    let out = s
        .record("inspect_drive", "inspect_drive", None, result)
        .unwrap();
    fs::remove_dir_all(&dir).unwrap();
    (s, out)
}

/// A role watch with the Forerunner 245's listing (#16), only a USB drive: follow the issue,
/// share the font list.
#[test]
fn drive_only() {
    let fixture = as_role(fixture("issue-16.json"), example_watches::DRIVE_ONLY);
    let (mut s, out) = drive_scenario("drive-only", &fixture);
    assert_eq!(out["support"]["status"], "unsupported");
    s.issue_link("issue_link-font_list", json!({"report": out["report"]}));
    s.finish();
}

/// The Venu 2 connected as a USB drive: it can use MTP, so the app waits for the switch.
#[test]
fn venu2_drive() {
    let (s, _) = drive_scenario(
        "venu2-drive",
        &fixture("maintainer-venu2-stock-imported.json"),
    );
    s.finish();
}

/// A role watch with the Forerunner 255's listing (#35), confirmed on 29.52, on newer software
/// (composed: 30.00): confirmed on other software.
#[test]
fn other_software() {
    let mut fr255 = as_role(fixture("issue-35.json"), example_watches::OTHER_SOFTWARE);
    fr255["software"] = json!("30.00");
    let (watch, fonts) = watch_of(&fr255);
    let mut s = Scenario::new("other-software", fonts);
    let out = s.ok("inspect", "inspect", &watch);
    assert_eq!(out["tested"]["status"], "tested_other_software");
    assert_eq!(out["support"]["status"], "expected");
    s.finish();
}

/// The demo build's predicted watch (`crates/app/src/demo.rs`): lists none of this build's
/// fonts, so there is nothing to install; its font list shared.
#[test]
fn predicted() {
    let role = example_watches::PREDICTED;
    let watch = json!({
        "source": "the demo build's predicted watch",
        "model": role.model,
        "model_part": role.model_part,
        "software": "29.52",
        "declared": [],
        "extdata": [],
        "hidden": {"status": "listed", "files": []},
    });
    let (watch, fonts) = watch_of(&watch);
    let mut s = Scenario::new("predicted", fonts);
    let out = s.ok("inspect", "inspect", &watch);
    assert_eq!(state(&out), "unsupported");
    assert_eq!(out["support"]["status"], "unknown");
    assert_eq!(out["support"]["prediction"], "noto_truetype");
    s.fails("install", "install", &watch, ErrorCode::UnsupportedDevice);
    s.issue_link("issue_link-font_list", json!({"report": out["report"]}));
    s.finish();
}

fn no_device() -> Error {
    // usb.rs's message when no Garmin MTP device is attached.
    Error::new(
        ErrorCode::NoDevice,
        "No Garmin watch in USB file-transfer (MTP) mode was found. Connect it with a data cable.",
    )
}

/// A holder as plain data: `Holder::named` would ask this process for its pid.
fn holder(kind: HolderKind, name: &str, pid: u32, confirmed: bool) -> Holder {
    Holder {
        kind,
        name: Some(name.into()),
        pid: Some(pid),
        confirmed,
    }
}

/// What holds the watch for `kind` on `platform`, as the demo build names it: data, never read
/// from this computer.
fn busy_sample(kind: HolderKind, platform: &str) -> Busy {
    let windows = platform == "windows";
    let holder = match kind {
        HolderKind::GarminExpress if windows => holder(kind, "ExpressTray.exe", 6312, false),
        HolderKind::GarminExpress => holder(kind, "Garmin Express", 812, true),
        HolderKind::MacosCameraService => holder(kind, "ptpcamerad", 512, true),
        HolderKind::DesktopMtp if platform == "linux" => holder(kind, "gvfsd-mtp", 2301, true),
        HolderKind::DesktopMtp => holder(kind, "Android File Tra", 1450, true),
        HolderKind::Other => holder(kind, "usb-helper", 3070, true),
        // The app itself, still on an earlier step, as macOS cuts its name to 16 bytes.
        HolderKind::ThisApp => holder(kind, "Georgian for Gar", 4100, true),
        HolderKind::Unknown => Holder::unknown(),
    };
    let interface = match kind {
        HolderKind::MacosCameraService => (6, 1, 1),
        _ => (255, 255, 0),
    };
    Busy {
        holder,
        product_id: Some(0x4E77),
        product_name: None,
        interface: (!windows).then_some(UsbInterface {
            class: interface.0,
            subclass: interface.1,
            protocol: interface.2,
        }),
    }
}

/// The system a fixture's busy message is written for, whatever this computer is.
fn system(platform: &str) -> System {
    match platform {
        "windows" => System::Windows,
        "linux" => System::Linux,
        _ => System::Macos,
    }
}

/// The watch held by each kind of program on each platform where it occurs, then released
/// (the Venu 2); and the "still stuck" share with only the holder.
#[test]
fn busy() {
    let (watch, fonts) = watch_of(&fixture("maintainer-venu2-stock-imported.json"));
    let mut s = Scenario::new("busy", fonts);
    let cases = [
        (HolderKind::GarminExpress, "macos"),
        (HolderKind::GarminExpress, "windows"),
        (HolderKind::MacosCameraService, "macos"),
        (HolderKind::DesktopMtp, "linux"),
        (HolderKind::DesktopMtp, "macos"),
        (HolderKind::Other, "linux"),
        (HolderKind::Unknown, "linux"),
        (HolderKind::ThisApp, "macos"),
        (HolderKind::ThisApp, "windows"),
    ];
    for (kind, platform) in cases {
        let kind_name = serde_json::to_value(kind).unwrap();
        let step = format!("inspect-{}-{platform}", kind_name.as_str().unwrap());
        let busy = busy_sample(kind, platform);
        s.connect_fails(&step, "inspect", Error::busy_on(busy, system(platform)));
    }
    // A 16-character macOS name, which the system may have cut.
    let cut = Busy {
        holder: holder(HolderKind::Other, "com.example.sync", 4242, true),
        ..busy_sample(HolderKind::Other, "macos")
    };
    s.connect_fails(
        "inspect-other-macos-cut",
        "inspect",
        Error::busy_on(cut, System::Macos),
    );
    let held = busy_sample(HolderKind::GarminExpress, "macos");
    s.issue_link("issue_link-problem", json!({"busy": held}));
    s.ok("inspect-released", "inspect", &watch);
    s.finish();
}

/// Connecting fails before the watch is read. Messages after the core's (permission_denied as
/// on Linux); `garmin_folder_not_found` is the real one from a watch without GarminDevice.xml.
/// The problems the user can't fix are shared with their error alone.
#[test]
fn connect_problems() {
    let mut s = Scenario::new("connect-problems", example_watches::fonts());
    s.connect_fails("inspect-no_device", "inspect", no_device());
    s.connect_fails(
        "inspect-permission_denied",
        "inspect",
        Error::new(
            ErrorCode::PermissionDenied,
            "open the watch: permission denied (Permission denied (os error 13)). Let your user \
             open Garmin devices, then reconnect the watch:\n  echo 'SUBSYSTEM==\"usb\", \
             ATTR{idVendor}==\"091e\", MODE=\"0660\", TAG+=\"uaccess\"' | sudo tee \
             /etc/udev/rules.d/70-georgian-for-garmin.rules && sudo udevadm control --reload\n\
             That is the rule shipped as 70-georgian-for-garmin.rules.",
        ),
    );
    s.connect_fails(
        "inspect-multiple_devices",
        "inspect",
        Error::new(
            ErrorCode::MultipleDevices,
            "2 Garmin devices are connected; connect only the watch to update.",
        ),
    );
    s.connect_fails(
        "inspect-device_error",
        "inspect",
        Error::new(
            ErrorCode::DeviceError,
            "open the watch: the device stopped responding",
        ),
    );
    let internal = s.connect_fails(
        "inspect-internal",
        "inspect",
        Error::new(
            ErrorCode::Internal,
            "inspect stopped unexpectedly: the task panicked",
        ),
    );
    let (watch, _) = watch_of(&fixture("maintainer-venu2-stock-imported.json"));
    watch.0.borrow_mut().objects.remove(&XML);
    let no_folder = s.fails(
        "inspect-garmin_folder_not_found",
        "inspect",
        &watch,
        ErrorCode::GarminFolderNotFound,
    );
    let mut no_extdata = fixture("maintainer-venu2-stock-imported.json");
    no_extdata["extdata"] = json!("not_found");
    let (watch, _) = watch_of(&no_extdata);
    s.ok("inspect-extdata_not_found", "inspect", &watch);
    let no_extdata = s.fails(
        "install-extdata_not_found",
        "install",
        &watch,
        ErrorCode::ExtdataNotFound,
    );
    // "Report this problem" for the problems the user can't fix: the error alone.
    for error in [internal, no_folder, no_extdata] {
        let step = format!("issue_link-{}", error.code.as_str());
        s.issue_link(
            &step,
            json!({"error": {"code": error.code, "message": error.message}}),
        );
    }
    s.finish();
}

fn rollback_file(path: &str, action: RollbackAction, done: bool) -> RollbackFile {
    let verb = if done { "" } else { "not " };
    RollbackFile {
        path: path.into(),
        action,
        done,
        result: format!("{path} {verb}deleted"),
    }
}

/// Install's and restore's errors: `conflict`, `restore_first` and `restart_first` real, the
/// rest composed.
#[test]
fn errors() {
    let mut s = Scenario::new("errors", example_watches::fonts());

    // An imported font this build doesn't recognise (replay.rs's
    // an_unrecognised_imported_font_is_never_written_over).
    let mut other = fixture("maintainer-venu2-georgian-imported.json");
    for file in other["hidden"]["files"].as_array_mut().unwrap() {
        if file["name"] == "006-D9035-03.fnt" {
            file["bytes"] = json!(35000);
            file["file"] = Value::Null;
        }
    }
    let (watch, _) = watch_of(&other);
    assert_eq!(state(&s.ok("inspect-other", "inspect", &watch)), "other");
    s.fails("install-conflict", "install", &watch, ErrorCode::Conflict);

    // The Forerunner 165 declaring 0.01 after a trailer-less earlier build.
    let (watch, fonts) = watch_of(&fixture("issue-5-comment-7078836039.json"));
    s.fonts = fonts;
    s.ok("inspect-restore_first", "inspect", &watch);
    s.fails(
        "install-restore_first",
        "install",
        &watch,
        ErrorCode::RestoreFirst,
    );
    s.fonts = example_watches::fonts();

    // A waiting `.tmp` a restart left hidden (the Venu 2 after a crash restart).
    let mut tmp = fixture("maintainer-venu2-georgian-imported.json");
    let files = tmp["hidden"]["files"].as_array_mut().unwrap();
    let mut waiting = files
        .iter()
        .find(|f| f["name"] == "006-D9035-03.fnt")
        .unwrap()
        .clone();
    waiting["name"] = json!("006-D9035-03.tmp");
    files.push(waiting);
    let (watch, _) = watch_of(&tmp);
    s.fails(
        "install-restart_first",
        "install",
        &watch,
        ErrorCode::RestartFirst,
    );

    let written = "GARMIN/ExtData/006-D9035-01.tmp";
    s.composed_error(
        "install-verification_failed",
        "install",
        Error::new(
            ErrorCode::VerificationFailed,
            format!("{written} read back differs from what was written."),
        )
        .with_rollback([rollback_file(written, RollbackAction::Delete, true)]),
    );
    s.composed_error(
        "install-no_device",
        "install",
        Error::new(ErrorCode::NoDevice, "The watch disconnected.").with_rollback([rollback_file(
            written,
            RollbackAction::Delete,
            false,
        )]),
    );
    s.composed_error(
        "restore_stock-device_error",
        "restore_stock",
        Error::new(
            ErrorCode::DeviceError,
            "SendObject failed: General error (0x2002)",
        )
        .with_rollback([rollback_file(written, RollbackAction::Delete, true)]),
    );
    s.composed_error(
        "install-font_unavailable",
        "install",
        Error::new(
            ErrorCode::FontUnavailable,
            "This build has no font part 006-D9035-09.",
        ),
    );
    s.composed_error(
        "install-internal",
        "install",
        Error::new(
            ErrorCode::Internal,
            "install stopped unexpectedly: the task panicked",
        ),
    );
    s.composed_error(
        "install-invalid_input",
        "install",
        Error::new(ErrorCode::InvalidInput, "parts: unknown field"),
    );
    s.finish();
}

/// `switch_to_mtp` as the session runs it, with its switch answering `result`.
fn switch(
    s: &mut Scenario,
    step: &str,
    result: Result<SwitchOutput, Error>,
) -> Result<Value, Error> {
    let fonts = s.fonts.clone();
    let mut result = Some(result);
    let mut session = Session::one_shot(Cow::Owned(fonts), Box::new(|| Err(no_device())))
        .with_switch(Box::new(move |_| result.take().expect("one switch")));
    let out = session.call("switch_to_mtp", json!({}), &Control::new(None, None));
    s.record(step, "switch_to_mtp", None, out)
}

/// The Venu 2's answer to the switch, as the maintainer's answered: its product data and
/// protocols.
fn venu2_switch(status: SwitchStatus, waited_seconds: f64) -> SwitchOutput {
    SwitchOutput {
        status,
        product: Some(Product {
            product_id: 3703,
            software_version: "19.05".into(),
            description: "VENU 2 V19.05".into(),
        }),
        protocols: ["P000", "L001", "A010", "A903", "A1050"]
            .map(String::from)
            .into(),
        waited_seconds,
    }
}

/// A watch connected in Garmin mode: switchable here, or not (Windows); the switch's outcomes.
#[test]
fn garmin_mode() {
    let mut s = Scenario::new("garmin-mode", example_watches::fonts());
    s.connect_fails(
        "inspect-garmin_mode",
        "inspect",
        Error::garmin_mode(GarminMode {
            switchable: true,
            reason: None,
        }),
    );
    s.connect_fails(
        "inspect-garmin_mode-windows",
        "inspect",
        Error::garmin_mode(GarminMode {
            switchable: false,
            reason: Some(NotSwitchable::NoDriver),
        }),
    );
    let switched = switch(
        &mut s,
        "switch_to_mtp-switched",
        Ok(venu2_switch(SwitchStatus::Switched, 4.2)),
    );
    assert_eq!(switched.unwrap()["status"], "switched");
    let timed_out = switch(
        &mut s,
        "switch_to_mtp-timed_out",
        Ok(venu2_switch(SwitchStatus::TimedOut, 15.0)),
    );
    assert_eq!(timed_out.unwrap()["status"], "timed_out");
    // garmin_usb.rs's messages.
    let locked = switch(
        &mut s,
        "switch_to_mtp-device_locked",
        Err(Error::new(
            ErrorCode::DeviceLocked,
            "The watch reports that it is locked, so it wasn't switched to file transfer (MTP). \
             Unlock it, then choose MTP on the watch when it asks, or unplug it and connect it \
             again.",
        )),
    );
    assert_eq!(locked.unwrap_err().code, ErrorCode::DeviceLocked);
    let unsupported = switch(
        &mut s,
        "switch_to_mtp-unsupported_device",
        Err(Error::new(
            ErrorCode::UnsupportedDevice,
            "The watch can't be switched to file transfer (MTP) over USB: it doesn't list A1050 \
             (it lists P000, L001, A010). On the watch, choose MTP when it asks, or set \
             Settings › System › USB Mode › MTP.",
        )),
    );
    assert_eq!(unsupported.unwrap_err().code, ErrorCode::UnsupportedDevice);
    s.finish();
}
