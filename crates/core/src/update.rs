//! `check_update`: the latest published release of this program on GitHub, and whether it is
//! newer than this build. One GET of GitHub's documented REST endpoint for a repository's latest
//! release, which leaves out drafts and prereleases; only its `tag_name` is read, and the link to
//! the release is built here from the version, never taken from the answer. Nothing about the
//! user or the watch is sent, and nothing is downloaded or installed.

use std::time::Duration;

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorCode};

/// GitHub's REST endpoint for this repository's latest published release.
pub const LATEST_RELEASE: &str =
    "https://api.github.com/repos/lubaretsi/georgian-for-garmin/releases/latest";
/// The repository's releases; release `v<version>` is `{RELEASES}/tag/v<version>`.
pub const RELEASES: &str = "https://github.com/lubaretsi/georgian-for-garmin/releases";
/// The headers GitHub's REST API documents: its media type, the API version, and a
/// `User-Agent` (which GitHub requires) naming the program and nothing else.
pub const HEADERS: [(&str, &str); 3] = [
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
    ("User-Agent", "georgian-for-garmin"),
];
/// How long the whole request may take.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// The most of an answer that is read: a release's JSON is a few kilobytes.
const MAX_BODY: u64 = 1 << 20;

/// An HTTP answer: its status code and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Gets `url` with `headers`: the answer, whatever its status, or why none came (unreachable,
/// timed out, refused by TLS). [`http_get`], or a fake in the tests.
pub type Get<'a> = dyn FnMut(&str, &[(&str, &str)]) -> Result<Answer, String> + 'a;

/// `check_update` takes no parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckUpdateInput {}

/// The latest published release, and whether it is newer than this build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckUpdateOutput {
    /// This build's version, e.g. "0.4.1".
    pub current: String,
    /// The latest published release's version, from its tag `v<version>`, e.g. "0.5.0".
    pub latest: String,
    /// `latest` is newer than `current` by semver precedence: a prerelease build (0.5.0-beta.1)
    /// sees its stable release (0.5.0) as newer.
    pub newer: bool,
    /// The latest release's page, where its downloads are:
    /// "https://github.com/lubaretsi/georgian-for-garmin/releases/tag/v0.5.0".
    pub url: String,
}

/// The page of release `version`.
pub fn release_url(version: &Version) -> String {
    format!("{RELEASES}/tag/v{version}")
}

/// What GitHub's answer is read for.
#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

fn network(message: String) -> Error {
    Error::new(ErrorCode::NetworkError, message)
}

/// Asks GitHub for the latest release through `get` and compares it with `current`, this
/// build's version.
pub fn check_update(get: &mut Get<'_>, current: &str) -> Result<CheckUpdateOutput, Error> {
    let this = Version::parse(current).map_err(|e| {
        Error::new(
            ErrorCode::Internal,
            format!("this build's version {current:?} isn't a version: {e}"),
        )
    })?;
    let answer = get(LATEST_RELEASE, &HEADERS).map_err(|why| {
        network(format!(
            "Could not reach GitHub to check for a newer version: {why}."
        ))
    })?;
    match answer.status {
        200 => {}
        403 | 429 => {
            return Err(network(format!(
                "GitHub refused the check for a newer version (HTTP {}), most likely its limit \
                 on requests per hour from one address. Try again later.",
                answer.status
            )));
        }
        404 => {
            return Err(network(
                "GitHub lists no published release of Georgian for Garmin.".into(),
            ));
        }
        status => {
            return Err(network(format!(
                "GitHub answered the check for a newer version with HTTP {status}."
            )));
        }
    }
    let release: Release = serde_json::from_slice(&answer.body)
        .map_err(|e| network(format!("GitHub's answer isn't a release: {e}.")))?;
    let latest = release
        .tag_name
        .strip_prefix('v')
        .and_then(|v| Version::parse(v).ok())
        .ok_or_else(|| {
            network(format!(
                "The latest release's tag {:?} isn't v followed by a version.",
                release.tag_name
            ))
        })?;
    Ok(CheckUpdateOutput {
        current: this.to_string(),
        latest: latest.to_string(),
        newer: latest.cmp_precedence(&this).is_gt(),
        url: release_url(&latest),
    })
}

/// Gets `url` over HTTPS with `headers`, in at most [`TIMEOUT`], reading at most 1 MiB. TLS is
/// rustls with Mozilla's root certificates built in on macOS and Linux (the Linux command line
/// is a static binary, with no system library to lean on), and Windows' own TLS and
/// certificate store on Windows.
pub fn http_get(url: &str, headers: &[(&str, &str)]) -> Result<Answer, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .tls_config(tls())
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut request = agent.get(url);
    for &(name, value) in headers {
        request = request.header(name, value);
    }
    let mut response = request.call().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    Ok(Answer { status, body })
}

#[cfg(not(windows))]
fn tls() -> ureq::tls::TlsConfig {
    ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::Rustls)
        .build()
}

#[cfg(windows)]
fn tls() -> ureq::tls::TlsConfig {
    ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transport that answers `status` and `body` once, recording what was asked.
    fn answering(
        status: u16,
        body: &str,
        asked: &mut Vec<(String, Vec<(String, String)>)>,
    ) -> impl FnMut(&str, &[(&str, &str)]) -> Result<Answer, String> {
        let body = body.as_bytes().to_vec();
        move |url, headers| {
            asked.push((
                url.to_owned(),
                headers
                    .iter()
                    .map(|&(n, v)| (n.to_owned(), v.to_owned()))
                    .collect(),
            ));
            Ok(Answer {
                status,
                body: body.clone(),
            })
        }
    }

    fn check(current: &str, tag: &str) -> CheckUpdateOutput {
        let mut asked = Vec::new();
        let body =
            format!(r#"{{"tag_name": "{tag}", "html_url": "https://example.com/elsewhere"}}"#);
        let mut get = answering(200, &body, &mut asked);
        check_update(&mut get, current).unwrap()
    }

    #[test]
    fn asks_githubs_latest_release_endpoint_once_naming_only_the_program() {
        let mut asked = Vec::new();
        let mut get = answering(200, r#"{"tag_name": "v0.4.1"}"#, &mut asked);
        check_update(&mut get, "0.4.1").unwrap();
        drop(get);
        assert_eq!(asked.len(), 1);
        let (url, headers) = &asked[0];
        assert_eq!(
            url,
            "https://api.github.com/repos/lubaretsi/georgian-for-garmin/releases/latest"
        );
        let header = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(header("User-Agent"), Some("georgian-for-garmin"));
        assert_eq!(header("Accept"), Some("application/vnd.github+json"));
        assert_eq!(header("X-GitHub-Api-Version"), Some("2022-11-28"));
        assert_eq!(headers.len(), 3);
    }

    #[test]
    fn a_newer_release_is_newer_and_links_to_its_own_page() {
        let out = check("0.4.1", "v0.5.0");
        assert_eq!(
            out,
            CheckUpdateOutput {
                current: "0.4.1".into(),
                latest: "0.5.0".into(),
                newer: true,
                url: "https://github.com/lubaretsi/georgian-for-garmin/releases/tag/v0.5.0".into(),
            }
        );
        // Versions compare as numbers, not text.
        assert!(check("0.9.0", "v0.10.0").newer);
        assert!(check("0.4.1", "v1.0.0").newer);
    }

    #[test]
    fn the_same_or_an_older_release_is_not_newer() {
        let same = check("0.4.1", "v0.4.1");
        assert!(!same.newer);
        assert_eq!(same.latest, "0.4.1");
        assert!(!check("0.5.0", "v0.4.1").newer);
        assert!(!check("0.10.0", "v0.9.0").newer);
    }

    #[test]
    fn a_prerelease_build_sees_its_release_as_newer() {
        assert!(check("0.5.0-beta.1", "v0.5.0").newer);
        assert!(!check("0.5.0", "v0.5.0-beta.1").newer);
        // Build metadata doesn't count.
        assert!(!check("0.4.1", "v0.4.1+build.7").newer);
    }

    fn error(status: u16, body: &str) -> Error {
        let mut asked = Vec::new();
        let mut get = answering(status, body, &mut asked);
        check_update(&mut get, "0.4.1").unwrap_err()
    }

    #[test]
    fn refusals_and_other_answers_are_network_errors() {
        for (status, body, says) in [
            (403, r#"{"message": "API rate limit exceeded"}"#, "limit"),
            (429, "", "limit"),
            (404, r#"{"message": "Not Found"}"#, "no published release"),
            (500, "", "HTTP 500"),
            (200, "<html>", "isn't a release"),
            (200, r#"{"name": "v0.5.0"}"#, "isn't a release"),
            (
                200,
                r#"{"tag_name": "0.5.0"}"#,
                "isn't v followed by a version",
            ),
            (
                200,
                r#"{"tag_name": "v0.5"}"#,
                "isn't v followed by a version",
            ),
            (
                200,
                r#"{"tag_name": "latest"}"#,
                "isn't v followed by a version",
            ),
        ] {
            let e = error(status, body);
            assert_eq!(e.code, ErrorCode::NetworkError, "{status} {body}");
            assert!(e.message.contains(says), "{status} {body}: {}", e.message);
        }
    }

    #[test]
    fn no_answer_is_a_network_error_with_the_reason() {
        let mut get = |_: &str, _: &[(&str, &str)]| Err("timed out".to_owned());
        let e = check_update(&mut get, "0.4.1").unwrap_err();
        assert_eq!(e.code, ErrorCode::NetworkError);
        assert!(e.message.contains("timed out"), "{}", e.message);
    }
}
