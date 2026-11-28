//! CLI contract checks that stop before any USB access.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

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

const OPERATIONS: [&str; 10] = [
    "inspect",
    "install",
    "restore_stock",
    "inspect_drive",
    "issue_link",
    "extdata_list",
    "wait_for_watch",
    "diagnose",
    "switch_to_mtp",
    "check_update",
];
const DEV_OPERATIONS: [&str; 13] = [
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

fn tool_names(tools: &Value) -> Vec<&str> {
    tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect()
}

/// The development operations exist only with the `dev` feature, which release builds never
/// have (a release build with it fails to compile).
#[test]
fn help_json_prints_mcp_tools() {
    let out = run(&["--help", "--json"], "");
    assert_eq!(out.status.code(), Some(0));
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut expected = OPERATIONS.to_vec();
    if cfg!(feature = "dev") {
        expected.extend(DEV_OPERATIONS);
    }
    assert_eq!(tool_names(&tools), expected);
    for tool in tools["tools"].as_array().unwrap() {
        let hints = &tool["annotations"];
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(hints[hint].is_boolean(), "{}: {hint}", tool["name"]);
        }
    }

    let out = run(&["extdata-list", "--help", "--json"], "");
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tool_names(&tools), ["extdata_list"]);

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
  "packs": [], "tested": "untested", "state": "unsupported", "hosts": {"kind": null, "reason": "None."}, "parts": [],
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

fn messages(out: &Output) -> Vec<Value> {
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("one JSON message per line"))
        .collect()
}

fn by_id(messages: &[Value], id: Value) -> &Value {
    messages
        .iter()
        .find(|m| m["id"] == id)
        .unwrap_or_else(|| panic!("no response to {id}: {messages:?}"))
}

/// Over stdin and stdout, without touching USB: a malformed line is answered and the session
/// goes on; the development operations are unknown methods without the `dev` feature.
#[test]
fn rpc_over_stdio() {
    let report: Value = serde_json::from_str(REPORT).unwrap();
    let request = |id: u32, method: &str, params: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
    };
    let lines = [
        "this is not JSON".to_owned(),
        request(
            1,
            "issue_link",
            json!({"report": report, "_meta": {"progressToken": 1}}),
        ),
        request(2, "issue_link", json!({"report": report, "bogus": 1})),
        request(3, "stage_file", json!({})),
        request(4, "unknown", json!({})),
    ];
    let out = run(&["rpc"], &(lines.join("\n") + "\n"));
    let messages = messages(&out);
    assert_eq!(messages.len(), 5, "{messages:?}");
    let parse = by_id(&messages, Value::Null);
    assert_eq!(parse["error"]["code"], -32700);
    assert!(
        by_id(&messages, json!(1))["result"]["url"]
            .as_str()
            .unwrap()
            .starts_with("https://github.com/")
    );
    let invalid = &by_id(&messages, json!(2))["error"];
    assert_eq!(invalid["code"], -32602);
    assert_eq!(invalid["data"]["code"], "invalid_input");
    let dev = &by_id(&messages, json!(3))["error"];
    if cfg!(feature = "dev") {
        assert_eq!(dev["code"], -32602, "stage_file without its source");
    } else {
        assert_eq!(dev["code"], -32601, "no stage_file without the dev feature");
    }
    assert_eq!(by_id(&messages, json!(4))["error"]["code"], -32601);
}

/// The official SDK over stdin and stdout: the handshake, the tool list and a call.
#[test]
fn mcp_over_stdio() {
    let report: Value = serde_json::from_str(REPORT).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_georgian-for-garmin"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut exchange = |message: Value, id: Option<u32>| -> Option<Value> {
        writeln!(stdin, "{message}").unwrap();
        let id = id?;
        loop {
            let mut line = String::new();
            assert_ne!(
                std::io::BufRead::read_line(&mut stdout, &mut line).unwrap(),
                0
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                return Some(message);
            }
        }
    };
    let init = exchange(
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "test", "version": "1"}}}),
        Some(0),
    )
    .unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "georgian-for-garmin");
    exchange(
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        None,
    );
    let list = exchange(
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
        Some(1),
    )
    .unwrap();
    let tools = &list["result"];
    let mut expected = OPERATIONS.to_vec();
    if cfg!(feature = "dev") {
        expected.extend(DEV_OPERATIONS);
    }
    assert_eq!(tool_names(tools), expected);
    let call = exchange(
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "issue_link", "arguments": {"report": report}}}),
        Some(2),
    )
    .unwrap();
    assert_eq!(call["result"]["isError"], false, "{call}");
    assert!(call["result"]["structuredContent"]["url"].is_string());
    drop(stdin);
    assert!(child.wait().unwrap().success());
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

/// `check-update` is the one open-world operation: read-only, touching no watch, and its input
/// is refused before anything goes online.
#[test]
fn check_update_is_declared_open_world_and_checks_its_input_first() {
    let out = run(&["check-update", "--help", "--json"], "");
    assert_eq!(out.status.code(), Some(0));
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    let tool = &tools["tools"][0];
    assert_eq!(tool["name"], "check_update");
    assert_eq!(
        tool["annotations"],
        json!({"title": "Check for a newer version", "readOnlyHint": true,
               "destructiveHint": false, "idempotentHint": true, "openWorldHint": true})
    );
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    let output = &tool["outputSchema"];
    for key in ["current", "latest", "newer", "url"] {
        assert!(output["properties"][key].is_object(), "{key}");
        assert!(
            output["required"].as_array().unwrap().contains(&json!(key)),
            "{key}"
        );
    }

    let error = json_error(&run(
        &["check-update", "--input", "-", "--json"],
        r#"{"channel": "beta"}"#,
    ));
    assert_eq!(error["error"]["code"], "invalid_input");

    let out = run(&["-h"], "");
    assert!(String::from_utf8_lossy(&out.stdout).contains("check-update"));
}

#[test]
fn diagnose_is_read_only_in_help_json() {
    let out = run(&["diagnose", "--help", "--json"], "");
    assert_eq!(out.status.code(), Some(0));
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    let tool = &tools["tools"][0];
    assert_eq!(tool["name"], "diagnose");
    assert_eq!(
        tool["annotations"],
        json!({"title": "Diagnostic info", "readOnlyHint": true, "destructiveHint": false,
               "idempotentHint": true, "openWorldHint": false})
    );
    assert_eq!(tool["inputSchema"]["properties"]["events"]["maxItems"], 50);
    for key in [
        "app",
        "system",
        "usb",
        "connection",
        "drives",
        "watch",
        "events",
        "text",
    ] {
        assert!(tool["outputSchema"]["properties"][key].is_object(), "{key}");
    }
    let out = run(&["diagnose", "-h"], "");
    assert!(String::from_utf8_lossy(&out.stdout).contains("--save <FILE>"));
}

/// `switch-to-mtp` changes how the watch connects (not read-only, no file written); its input
/// is refused before any USB access.
#[test]
fn switch_to_mtp_is_declared_and_checks_its_input_first() {
    let out = run(&["switch-to-mtp", "--help", "--json"], "");
    assert_eq!(out.status.code(), Some(0));
    let tools: Value = serde_json::from_slice(&out.stdout).unwrap();
    let tool = &tools["tools"][0];
    assert_eq!(tool["name"], "switch_to_mtp");
    assert_eq!(
        tool["annotations"],
        json!({"title": "Switch the watch to MTP", "readOnlyHint": false,
               "destructiveHint": false, "idempotentHint": true, "openWorldHint": false})
    );
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    let output = &tool["outputSchema"]["properties"];
    for key in ["status", "product", "protocols", "waited_seconds"] {
        assert!(output[key].is_object(), "{key}");
    }

    let error = json_error(&run(
        &["switch-to-mtp", "--input", "-", "--json"],
        r#"{"retries": 3}"#,
    ));
    assert_eq!(error["error"]["code"], "invalid_input");

    let out = run(&["switch-to-mtp", "-h"], "");
    assert!(String::from_utf8_lossy(&out.stdout).contains("Garmin mode"));
    let out = run(&["-h"], "");
    assert!(String::from_utf8_lossy(&out.stdout).contains("switch-to-mtp"));
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("gfg-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("diagnostics.txt")
}

/// An existing file is refused before anything is looked at, and left as it was.
#[test]
fn diagnose_save_never_overwrites() {
    let path = temp_path("save");
    std::fs::write(&path, "mine").unwrap();
    let out = run(
        &["diagnose", "--save", path.to_str().unwrap(), "--json"],
        "",
    );
    let error = json_error(&out);
    assert_eq!(error["error"]["code"], "invalid_input");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already exists"),
        "{error}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");
    let out = run(&["diagnose", "--save", path.to_str().unwrap()], "");
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");
}

/// Events are checked before anything is looked at: a serial number in one is refused.
#[test]
fn diagnose_refuses_identifier_events() {
    let path = temp_path("refused");
    let input = json!({"events": [{"time": "2026-11-24T03:14:05", "kind": "poll",
                                   "detail": "serial FAKESERIAL0042", "model": "Venu 2"}]});
    let out = run(
        &[
            "diagnose",
            "--input",
            "-",
            "--save",
            path.to_str().unwrap(),
            "--json",
        ],
        &input.to_string(),
    );
    let error = json_error(&out);
    assert_eq!(error["error"]["code"], "invalid_input");
    assert!(!path.exists());
}
