//! The window's remembered choices, kept as `settings.json` in the app's config directory.
//!
//! Only the interface language for now. A missing or unreadable file means "no choice yet";
//! the frontend then uses its default language.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Language code of a table in `ui/strings.json`, e.g. "ka".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

pub const FILE: &str = "settings.json";

/// A plausible language code: 2–8 lowercase ASCII letters.
pub fn valid_language(code: &str) -> bool {
    (2..=8).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_lowercase())
}

/// The saved settings, or defaults if the file is missing or damaged.
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

/// Saves `settings`, replacing the file in one step so a crash never leaves half of it.
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
    fn language_codes_are_short_lowercase_words() {
        assert!(valid_language("ka"));
        assert!(valid_language("en"));
        assert!(!valid_language("k"));
        assert!(!valid_language("EN"));
        assert!(!valid_language("en-US"));
        assert!(!valid_language(""));
    }
}
