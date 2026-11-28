//! `check_update` through a session, as the CLI, `rpc`, `mcp` and the app run it: over a fake
//! transport, never the network, and never touching a watch.

use std::borrow::Cow;
use std::cell::Cell;

use georgian_for_garmin_core::fonts::FontSet;
use georgian_for_garmin_core::session::{Control, Session};
use georgian_for_garmin_core::update::Answer;
use georgian_for_garmin_core::{Error, ErrorCode, ops};
use serde_json::{Value, json};

fn no_fonts() -> FontSet {
    FontSet {
        packs: Vec::new(),
        watches: Vec::new(),
    }
}

/// `check_update` in a session whose transport answers `answer`, counting its requests; the
/// watch, if anything opened it, would fail the test.
fn check(answer: Result<Answer, String>, requests: &Cell<usize>) -> Result<Value, Error> {
    let fonts = no_fonts();
    let mut session = Session::one_shot(
        Cow::Borrowed(&fonts),
        Box::new(|| panic!("check_update opened the watch")),
    )
    .with_http(Box::new(|_, _| {
        requests.set(requests.get() + 1);
        answer.clone()
    }));
    session.call("check_update", json!({}), &Control::NONE)
}

fn release(tag: &str) -> Result<Answer, String> {
    Ok(Answer {
        status: 200,
        body: format!(r#"{{"tag_name": "{tag}", "draft": false, "prerelease": false}}"#).into(),
    })
}

#[test]
fn reports_this_build_against_the_latest_release_in_one_request() {
    let requests = Cell::new(0);
    let out = check(release("v99.0.0"), &requests).unwrap();
    assert_eq!(requests.get(), 1);
    assert_eq!(
        out,
        json!({
            "current": ops::APP_VERSION,
            "latest": "99.0.0",
            "newer": true,
            "url": "https://github.com/lubaretsi/georgian-for-garmin/releases/tag/v99.0.0",
        })
    );

    let same = check(release(&format!("v{}", ops::APP_VERSION)), &requests).unwrap();
    assert_eq!(same["newer"], false);
    assert_eq!(same["latest"], ops::APP_VERSION);
    let older = check(release("v0.0.1"), &requests).unwrap();
    assert_eq!(older["newer"], false);
    assert_eq!(requests.get(), 3);
}

#[test]
fn failures_are_network_errors_and_invalid_input_never_goes_online() {
    let requests = Cell::new(0);
    let offline = check(Err("connection refused".into()), &requests).unwrap_err();
    assert_eq!(offline.code, ErrorCode::NetworkError);
    let limited = check(
        Ok(Answer {
            status: 403,
            body: br#"{"message": "API rate limit exceeded"}"#.to_vec(),
        }),
        &requests,
    )
    .unwrap_err();
    assert_eq!(limited.code, ErrorCode::NetworkError);
    assert_eq!(requests.get(), 2);
    assert_eq!(
        offline.to_json()["error"]["code"],
        "network_error",
        "the CLI's --json error"
    );

    let fonts = no_fonts();
    let mut session = Session::one_shot(Cow::Borrowed(&fonts), Box::new(|| panic!("opened")))
        .with_http(Box::new(|_, _| panic!("went online")));
    let refused = session
        .call("check_update", json!({"channel": "beta"}), &Control::NONE)
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::InvalidInput);
}
