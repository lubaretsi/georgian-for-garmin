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
    assert_eq!(tools["tools"].as_array().unwrap().len(), 3);

    let out = run(&["restore-stock", "--json", "-h"], "");
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tools["tools"][0]["name"], "restore_stock");
    assert_eq!(tools["tools"].as_array().unwrap().len(), 1);
}

#[test]
fn invalid_input_exits_2_with_json_error() {
    for (args, stdin) in [
        (&["install", "--size-px", "26", "--json"][..], ""),
        (
            &["install", "--input", "-", "--json"][..],
            r#"{"size_px": 99}"#,
        ),
        (&["restore-stock", "--input", "-", "--json"][..], "[27]"),
        (&["install", "--input", "-", "--json"][..], "{not json"),
        (&["install", "--sizepx", "27", "--json"][..], ""),
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
fn flags_override_input_before_validation() {
    // The flag's invalid value wins over the valid --input value.
    let error = json_error(&run(
        &["install", "--input", "-", "--size-px", "30", "--json"],
        r#"{"size_px": 27}"#,
    ));
    assert!(error["error"]["message"].as_str().unwrap().contains("30"));
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
