//! The window's remembered choices, kept as `settings.json` in the app's config directory.
//!
//! The interface language, and what the user has already shared, so a repeat is thanked
//! instead of asked for again. A missing or unreadable file means "no choice yet": the
//! frontend then uses its default language and asks for every share.

use std::fs;
use std::io;
use std::path::Path;

use georgian_for_garmin_core::report::Seen;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Language code of one of the frontend's copy catalogs, e.g. "ka".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// What was shared on GitHub, one entry per watch model, software and result.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared: Vec<ShareKey>,
}

/// What a user already shared about a model and its software; no unit ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShareKey {
    /// The watch model's part number, e.g. "006-B3703-00".
    #[schemars(length(max = 40))]
    pub model_part: String,
    /// The watch's software version, e.g. "19.05".
    #[schemars(length(max = 40))]
    pub software: String,
    pub result: ShareSubject,
}

/// What was shared: the answer to what the watch showed, or the watch's font list alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ShareSubject {
    Result(Seen),
    FontList(FontList),
}

/// The watch's font list, shared without an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FontList {
    FontList,
}

/// The demo build keeps the language and its shares in memory, so it never reads or writes
/// this file.
#[cfg_attr(feature = "demo", allow(dead_code))]
pub const FILE: &str = "settings.json";

/// A plausible language code: 2–8 lowercase ASCII letters.
pub fn valid_language(code: &str) -> bool {
    (2..=8).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_lowercase())
}

/// The saved settings, or defaults if the file is missing or damaged.
#[cfg_attr(feature = "demo", allow(dead_code))]
pub fn load(dir: &Path) -> Settings {
    fs::read(dir.join(FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
        .map(|mut s| {
            s.language = s.language.filter(|l| valid_language(l));
            s
        })
        .unwrap_or_default()
}

impl Settings {
    /// Remembers that `key` was shared; false if it already was. (The demo build keeps its
    /// shares in memory instead.)
    #[cfg_attr(feature = "demo", allow(dead_code))]
    pub fn mark_shared(&mut self, key: ShareKey) -> bool {
        let new = !self.shared.contains(&key);
        if new {
            self.shared.push(key);
        }
        new
    }
}

/// Saves `settings`, replacing the file in one step so a crash never leaves half of it.
#[cfg_attr(feature = "demo", allow(dead_code))]
pub fn save(dir: &Path, settings: &Settings) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    fs::rename(&tmp, dir.join(FILE))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "georgian-for-garmin-settings-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn language_round_trips_through_a_new_directory() {
        let dir = scratch("round-trip");
        assert_eq!(load(&dir), Settings::default());
        let english = Settings {
            language: Some("en".into()),
            ..Settings::default()
        };
        save(&dir, &english).unwrap();
        assert_eq!(load(&dir), english);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn damaged_or_odd_files_fall_back_to_defaults() {
        let dir = scratch("damaged");
        fs::create_dir_all(&dir).unwrap();
        for contents in ["{", "[]", r#"{"language": 7}"#, r#"{"language": "../x"}"#] {
            fs::write(dir.join(FILE), contents).unwrap();
            assert_eq!(load(&dir), Settings::default(), "{contents}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shares_round_trip_as_the_frontend_names_them() {
        let dir = scratch("shared");
        let key: ShareKey = serde_json::from_value(serde_json::json!(
            {"modelPart": "006-B3703-00", "software": "19.05", "result": "boxes"}
        ))
        .unwrap();
        let list: ShareKey = serde_json::from_value(serde_json::json!(
            {"modelPart": "006-B3703-00", "software": "19.05", "result": "font_list"}
        ))
        .unwrap();
        assert_eq!(key.result, ShareSubject::Result(Seen::Boxes));
        assert_eq!(list.result, ShareSubject::FontList(FontList::FontList));
        let mut settings = Settings::default();
        assert!(settings.mark_shared(key.clone()));
        assert!(!settings.mark_shared(key.clone()));
        save(&dir, &settings).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.shared, vec![key]);
        assert!(!loaded.shared.contains(&list));
        // Only these three fields: no room for a unit ID.
        assert!(
            serde_json::from_value::<ShareKey>(serde_json::json!(
                {"modelPart": "006-B3703-00", "software": "19.05", "result": "boxes", "unit": "1"}
            ))
            .is_err()
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn language_codes_are_short_lowercase_words() {
        assert!(valid_language("ka"));
        assert!(valid_language("en"));
        assert!(!valid_language("k"));
        assert!(!valid_language("EN"));
        assert!(!valid_language("en-US"));
        assert!(!valid_language(""));
    }
}
