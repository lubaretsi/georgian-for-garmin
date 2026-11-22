//! CLI contract checks that stop before any USB access.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_georgian-for-garmin"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn json_error(out: &Output) -> Value {
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    serde_json::from_slice(&out.stderr).expect("stderr is one JSON value")
}

#[test]
fn help_json_prints_mcp_tools() {
    let out = run(&["--help", "--json"], "");
    assert_eq!(out.status.code(), Some(0));
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tools["tools"].as_array().unwrap().len(), 5);

    let out = run(&["restore-stock", "--json", "-h"], "");
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tools["tools"][0]["name"], "restore_stock");
    assert_eq!(tools["tools"].as_array().unwrap().len(), 1);

    let out = run(&["issue-link", "--help", "--json"], "");
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tools["tools"][0]["name"], "issue_link");
}

#[test]
fn help_json_describes_parts_and_packs() {
    let out = run(&["install", "--help", "--json"], "");
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    let install = &tools["tools"][0];
    let parts = &install["inputSchema"]["properties"]["parts"];
    assert_eq!(parts["items"]["type"], "string", "{parts}");
    assert!(
        install["inputSchema"]["properties"]
            .get("sizes_px")
            .is_none()
    );
    assert!(install["outputSchema"]["properties"]["packs"].is_object());

    let out = run(&["inspect", "--help", "--json"], "");
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    let properties = &tools["tools"][0]["outputSchema"]["properties"];
    assert!(properties["packs"].is_object() && properties["tested"].is_object());
    assert!(properties.get("pack").is_none());
}

#[test]
fn invalid_input_exits_2_with_json_error() {
    for (args, stdin) in [
        (
            &["install", "--parts", "006-D9035-03,006-D9035-03", "--json"][..],
            "",
        ),
        (&["install", "--parts", "big", "--json"][..], ""),
        (&["install", "--parts", "006-d9035-03", "--json"][..], ""),
        (
            &["install", "--input", "-", "--json"][..],
            r#"{"parts": []}"#,
        ),
        (
            &["install", "--input", "-", "--json"][..],
            r#"{"sizes_px": [27]}"#,
        ),
        (
            &["restore-stock", "--input", "-", "--json"][..],
            r#"["006-D9035-03"]"#,
        ),
        (&["install", "--input", "-", "--json"][..], "{not json"),
        (&["install", "--sizes-px", "27", "--json"][..], ""),
        (&["inspect", "--parts", "006-D9035-03", "--json"][..], ""),
        (&["--json"][..], ""),
    ] {
        let error = json_error(&run(args, stdin));
        assert_eq!(error["error"]["code"], "invalid_input", "{args:?}");
        assert!(
            error["error"]["message"]
                .as_str()
                .is_some_and(|m| !m.is_empty())
        );
    }
}

#[test]
fn a_part_missing_from_the_build_fails_before_usb() {
    let out = run(
        &["install", "--parts", "006-D9035-03,006-D1234-01", "--json"],
        "",
    );
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["code"], "font_unavailable");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("006-D1234-01")
    );
}

#[test]
fn flags_override_input_before_validation() {
    // The flag's invalid value wins over the valid --input value.
    let error = json_error(&run(
        &[
            "install",
            "--input",
            "-",
            "--parts",
            "006-D9999-30,006-D9999-30",
            "--json",
        ],
        r#"{"parts": ["006-D9035-03"]}"#,
    ));
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("006-D9999-30")
    );
}

#[test]
fn version_and_help_succeed() {
    let out = run(&["--version"], "");
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("georgian-for-garmin "));
    let out = run(&["-h"], "");
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Examples:"));
}

/// A report as `inspect --report --json` prints it.
const REPORT: &str = r#"{"app_version": "0.3.0", "platform": {"os": "Mac OS", "os_version": "15.6", "arch": "aarch64"},
  "connection": "mtp", "model": "Venu 2", "model_part": "006-B3703-00", "software_version": "19.05",
  "packs": [], "tested": "untested", "hosts": {"kind": null, "reason": "None."}, "parts": [],
  "test_marks": false, "declared_files": [], "extdata_files": [], "extdata_listing": null,
  "title": "t", "text": "x"}"#;

#[test]
fn issue_link_needs_no_watch() {
    let input = format!(r#"{{"report": {REPORT}, "answers": {{"result": "boxes"}}}}"#);
    let out = run(&["issue-link", "--input", "-", "--json"], &input);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let link: Value = serde_json::from_slice(&out.stdout).unwrap();
    let url = link["url"].as_str().unwrap();
    assert!(url.starts_with("https://github.com/lubaretsi/georgian-for-garmin/issues/new?template=watch-report.yml&title=Watch+report"));
    assert!(url.contains("&result=Boxes+instead+of+letters&"), "{url}");
    assert_eq!(link["report_in_url"], true);

    // A report with a field outside the allowlist is refused.
    let leaky = REPORT.replace(r#""test_marks""#, r#""serial": "SN-1", "test_marks""#);
    let out = run(
        &["issue-link", "--input", "-", "--json"],
        &format!(r#"{{"report": {leaky}}}"#),
    );
    assert_eq!(json_error(&out)["error"]["code"], "invalid_input");
}

#[test]
fn inspect_drive_names_a_missing_volume() {
    let out = run(
        &[
            "inspect-drive",
            "--path",
            "/nonexistent-gfg-volume",
            "--json",
        ],
        "",
    );
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["code"], "garmin_folder_not_found");
}
