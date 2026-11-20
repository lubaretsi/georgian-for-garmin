//! Sharing the watch report as a prefilled GitHub issue.
//!
//! The report goes to the clipboard and into the new-issue URL as `title` and `body` query
//! parameters. GitHub and browsers refuse very long URLs, so a report that would make the URL
//! longer than [`MAX_URL`] is left out of it, and the body asks for a paste instead.

use url::Url;

/// Where watch reports are filed.
pub const NEW_ISSUE: &str = "https://github.com/lubaretsi/georgian-for-garmin/issues/new";

/// Longest URL we open. GitHub answers 414 somewhat above 8 KB; browsers allow more.
pub const MAX_URL: usize = 8000;

/// The body used when the report doesn't fit in the URL.
pub const PASTE_BODY: &str = "<!-- The watch report is on your clipboard. Paste it here. -->\n";

/// The new-issue URL for `title` and `body`, and whether `body` fit in it.
pub fn issue_url(title: &str, body: &str) -> (String, bool) {
    let url = |body: &str| {
        Url::parse_with_params(NEW_ISSUE, [("title", title), ("body", body)])
            .expect("NEW_ISSUE is a valid URL")
            .into()
    };
    let full: String = url(body);
    if full.len() <= MAX_URL {
        (full, true)
    } else {
        (url(PASTE_BODY), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(url: &str) -> Vec<(String, String)> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    #[test]
    fn a_report_that_fits_is_in_the_url() {
        let body = "## Watch\n\n- Model: Venu 2 & co\n\n| a | b |\n<!-- ქართული -->\n";
        let (url, complete) = issue_url("Watch report: Venu 2", body);
        assert!(complete);
        assert!(url.starts_with(NEW_ISSUE));
        assert_eq!(
            query(&url),
            [
                ("title".into(), "Watch report: Venu 2".into()),
                ("body".into(), body.into())
            ]
        );
    }

    #[test]
    fn a_report_too_long_for_a_url_asks_for_a_paste() {
        let body = "| 006-D9035-01 | 2.10 | 006-D9035-01.tmp |\n".repeat(400);
        let (url, complete) = issue_url("Watch report: Venu 2", &body);
        assert!(!complete);
        assert!(url.len() <= MAX_URL);
        assert_eq!(query(&url)[1], ("body".into(), PASTE_BODY.into()));
        assert_eq!(query(&url)[0].1, "Watch report: Venu 2");
    }

    #[test]
    fn the_limit_is_on_the_encoded_url() {
        // Georgian letters take nine bytes each once percent-encoded.
        let body = "ქ".repeat(1000);
        assert!(body.len() < MAX_URL);
        assert!(!issue_url("t", &body).1);
    }
}
