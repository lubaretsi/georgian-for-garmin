//! Georgian for Garmin: a desktop window over the core operations.
//!
//! The frontend (`ui/`) calls the watch commands below; each runs one core operation on a
//! blocking thread, and a mutex keeps the watch to one operation at a time, so the frontend's
//! `inspect` polling never overlaps an install. Commands return core output unchanged, and
//! errors as `{code, message}`; the frontend turns codes into words (`ui/strings.json`).
//! `share_report` copies the watch report and opens a prefilled GitHub issue (`share`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(feature = "demo", not(debug_assertions)))]
compile_error!(
    "the `demo` feature replaces the watch with a fake one; use it in debug builds only"
);

#[cfg(any(test, feature = "demo"))]
mod demo;
mod settings;
mod share;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use georgian_for_garmin_core::fonts::FontSet;
use georgian_for_garmin_core::mtp::Mtp;
use georgian_for_garmin_core::{Error, ErrorCode, ops};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

/// An operation failure as the frontend receives it.
#[derive(Debug, Serialize)]
struct Failure {
    code: &'static str,
    message: String,
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        Failure {
            code: e.code.as_str(),
            message: e.message,
        }
    }
}

/// The watch, shared by every command.
struct Watch {
    /// Held for the whole of each operation.
    busy: Mutex<()>,
    fonts: FontSet,
    connect: fn() -> Result<Box<dyn Mtp>, Error>,
}

impl Watch {
    fn new(connect: fn() -> Result<Box<dyn Mtp>, Error>) -> Self {
        Watch {
            busy: Mutex::new(()),
            fonts: FontSet::bundled(),
            connect,
        }
    }

    /// Runs `operation` while holding the watch.
    fn call(&self, operation: &str, input: Value) -> Result<Value, Failure> {
        let _busy = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        let mut connect = self.connect;
        let result = ops::call(operation, input, &self.fonts, &mut connect);
        // The demo moves on to its next watch after every install or restore.
        #[cfg(feature = "demo")]
        if operation != "inspect" {
            demo::unplug();
        }
        Ok(result?)
    }

    /// Runs `operation` on a blocking thread so USB work never stalls the window.
    async fn run(self: Arc<Self>, operation: &'static str, input: Value) -> Result<Value, Failure> {
        tauri::async_runtime::spawn_blocking(move || self.call(operation, input))
            .await
            .map_err(|e| {
                Failure::from(Error::new(
                    ErrorCode::Internal,
                    format!("{operation} stopped unexpectedly: {e}"),
                ))
            })?
    }
}

#[tauri::command]
async fn inspect(watch: State<'_, Arc<Watch>>) -> Result<Value, Failure> {
    Arc::clone(&watch).run("inspect", json!({})).await
}

/// Stages the Georgian font in every part of the bundled packs that the watch lists.
#[tauri::command]
async fn install(watch: State<'_, Arc<Watch>>) -> Result<Value, Failure> {
    Arc::clone(&watch).run("install", json!({})).await
}

/// Stages Garmin's original font in every part of the bundled packs that the watch lists.
#[tauri::command]
async fn restore_stock(watch: State<'_, Arc<Watch>>) -> Result<Value, Failure> {
    Arc::clone(&watch).run("restore_stock", json!({})).await
}

#[derive(Debug, Serialize)]
struct Shared {
    /// The whole report is in the issue; otherwise the issue asks for a paste.
    complete: bool,
}

/// Copies `text` (the watch report from `inspect`) and opens a new GitHub issue prefilled
/// with `title` and, if it fits in the URL, `text`. The demo build touches neither the
/// clipboard nor the browser.
#[tauri::command]
fn share_report(app: AppHandle, title: String, text: String) -> Result<Shared, Failure> {
    let (url, complete) = share::issue_url(&title, &text);
    let failed = |what: &str, e: &dyn std::fmt::Display| {
        Failure::from(Error::new(
            ErrorCode::Internal,
            format!("could not {what}: {e}"),
        ))
    };
    if !cfg!(feature = "demo") {
        app.clipboard()
            .write_text(text)
            .map_err(|e| failed("copy the report", &e))?;
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|e| failed("open the browser", &e))?;
    }
    Ok(Shared { complete })
}

/// Whether this build talks to the in-memory demo watch.
#[tauri::command]
fn demo_mode() -> bool {
    cfg!(feature = "demo")
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, Failure> {
    app.path().app_config_dir().map_err(|e| {
        Failure::from(Error::new(
            ErrorCode::Internal,
            format!("no config folder: {e}"),
        ))
    })
}

/// The interface language chosen earlier, or null if none was.
#[tauri::command]
fn saved_language(app: AppHandle) -> Option<String> {
    config_dir(&app)
        .ok()
        .and_then(|dir| settings::load(&dir).language)
}

/// Remembers the interface language for the next launch.
#[tauri::command]
fn save_language(app: AppHandle, language: String) -> Result<(), Failure> {
    if !settings::valid_language(&language) {
        return Err(Failure::from(Error::new(
            ErrorCode::InvalidInput,
            format!("{language:?} is not a language code"),
        )));
    }
    let dir = config_dir(&app)?;
    let mut saved = settings::load(&dir);
    saved.language = Some(language);
    settings::save(&dir, &saved).map_err(|e| {
        Failure::from(Error::new(
            ErrorCode::Internal,
            format!("could not save {}: {e}", dir.join(settings::FILE).display()),
        ))
    })
}

#[cfg(feature = "demo")]
const CONNECT: fn() -> Result<Box<dyn Mtp>, Error> = demo::connect;
#[cfg(not(feature = "demo"))]
const CONNECT: fn() -> Result<Box<dyn Mtp>, Error> = georgian_for_garmin_core::connect;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Watch::new(CONNECT)))
        .invoke_handler(tauri::generate_handler![
            inspect,
            install,
            restore_stock,
            share_report,
            demo_mode,
            saved_language,
            save_language
        ])
        .run(tauri::generate_context!())
        .expect("the window could not be opened");
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::*;

    /// Every string value of a `const` or `enum` anywhere in `schema`.
    fn constants(schema: &Value, out: &mut BTreeSet<String>) {
        match schema {
            Value::Object(map) => {
                for (key, value) in map {
                    match (key.as_str(), value) {
                        ("const", Value::String(s)) => {
                            out.insert(s.clone());
                        }
                        ("enum", Value::Array(items)) => {
                            out.extend(items.iter().filter_map(|i| i.as_str().map(String::from)));
                        }
                        _ => constants(value, out),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|i| constants(i, out)),
            _ => {}
        }
    }

    fn strings() -> serde_json::Map<String, Value> {
        match serde_json::from_str(include_str!("../ui/strings.json")).unwrap() {
            Value::Object(languages) => languages,
            other => panic!("ui/strings.json is not an object of languages: {other}"),
        }
    }

    /// Each string in `table` by dotted key, e.g. `errors.conflict`.
    fn leaves(prefix: &str, table: &Value, out: &mut Vec<(String, String)>) {
        match table {
            Value::Object(map) => {
                for (key, value) in map {
                    leaves(&format!("{prefix}{key}."), value, out);
                }
            }
            Value::String(s) => out.push((prefix.trim_end_matches('.').into(), s.clone())),
            other => panic!("{prefix}: {other} is neither a string nor a table"),
        }
    }

    /// `{name}` placeholders in `text`.
    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split('{')
            .skip(1)
            .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
            .collect()
    }

    #[test]
    fn georgian_and_english_are_shipped() {
        let languages = strings();
        assert!(languages.contains_key("ka"), "the default language");
        assert!(languages.contains_key("en"));
        assert!(languages.keys().all(|l| settings::valid_language(l)));
    }

    #[test]
    fn every_language_has_every_string() {
        let languages = strings();
        let tables: Vec<(&String, BTreeMap<String, String>)> = languages
            .iter()
            .map(|(language, table)| {
                let mut out = Vec::new();
                leaves("", table, &mut out);
                (language, out.into_iter().collect())
            })
            .collect();
        let all: BTreeSet<&String> = tables.iter().flat_map(|(_, t)| t.keys()).collect();
        let english = &tables.iter().find(|(l, _)| *l == "en").unwrap().1;
        for (language, table) in &tables {
            let missing: Vec<_> = all.iter().filter(|k| !table.contains_key(**k)).collect();
            assert!(missing.is_empty(), "{language} lacks {missing:?}");
            for (key, text) in table {
                assert!(!text.trim().is_empty(), "{language}.{key} is empty");
                assert_eq!(
                    placeholders(text),
                    placeholders(&english[key]),
                    "{language}.{key} placeholders"
                );
            }
        }
    }

    #[test]
    fn every_error_code_has_a_message() {
        let schema = schemars::schema_for!(ErrorCode).to_value();
        let mut codes = BTreeSet::new();
        constants(&schema, &mut codes);
        assert!(codes.contains("device_busy"), "unexpected schema: {schema}");
        for (language, table) in strings() {
            let errors = table["errors"].as_object().expect("an errors table");
            let missing: Vec<_> = codes.iter().filter(|c| !errors.contains_key(*c)).collect();
            assert!(missing.is_empty(), "{language} lacks errors {missing:?}");
        }
    }

    #[test]
    fn every_core_status_the_ui_shows_has_words() {
        let tables = [
            ("tested", schemars::schema_for!(ops::TestStatus).to_value()),
            (
                "afterRestart",
                schemars::schema_for!(ops::Variant).to_value(),
            ),
        ];
        for (key, schema) in tables {
            let mut values = BTreeSet::new();
            constants(&schema, &mut values);
            assert!(!values.is_empty(), "unexpected schema: {schema}");
            for (language, table) in strings() {
                let words = table[key].as_object().expect("a table");
                let missing: Vec<_> = values.iter().filter(|v| !words.contains_key(*v)).collect();
                assert!(missing.is_empty(), "{language}.{key} lacks {missing:?}");
            }
        }
        for (language, table) in strings() {
            for staged in ["georgian", "stock", "other"] {
                assert!(
                    table["stagedNow"].get(staged).is_some(),
                    "{language}.stagedNow.{staged}"
                );
            }
        }
    }

    /// Identifiers that follow `prefix` in `source`, e.g. `T.` → `install`.
    fn after<'a>(source: &'a str, prefix: &str, out: &mut BTreeSet<&'a str>) {
        for rest in source.split(prefix).skip(1) {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            if end > 0 {
                out.insert(&rest[..end]);
            }
        }
    }

    #[test]
    fn every_string_the_ui_uses_exists() {
        let html = include_str!("../ui/index.html");
        let js = include_str!("../ui/app.js");
        let mut used: BTreeSet<String> = BTreeSet::new();
        let mut found = BTreeSet::new();
        after(html, "data-t=\"", &mut found);
        after(js, "T.", &mut found);
        used.extend(found.iter().map(|k| k.to_string()));
        // Share results are string literals looked up as `T[shared.result]`.
        let mut results = BTreeSet::new();
        after(js, "\"share", &mut results);
        results.remove("_report");
        assert!(results.contains("Failed"), "{results:?}");
        used.extend(results.iter().map(|k| format!("share{k}")));
        for key in ["untestedBody", "stagedFiles", "stagedOneFile", "stagedPack"] {
            assert!(used.contains(key), "{key} not found in the UI");
        }
        for (language, table) in strings() {
            let missing: Vec<_> = used.iter().filter(|k| table.get(*k).is_none()).collect();
            assert!(missing.is_empty(), "{language} lacks {missing:?}");
        }
    }

    #[test]
    fn failures_keep_core_codes() {
        let failure = Failure::from(Error::new(ErrorCode::DeviceBusy, "held"));
        assert_eq!(
            serde_json::to_value(failure).unwrap(),
            json!({"code": "device_busy", "message": "held"})
        );
    }

    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    static MOST_ACTIVE: AtomicUsize = AtomicUsize::new(0);

    /// A watch that takes a while to answer and counts how many callers it has at once.
    fn slow_connect() -> Result<Box<dyn Mtp>, Error> {
        let active = ACTIVE.fetch_add(1, Ordering::SeqCst) + 1;
        MOST_ACTIVE.fetch_max(active, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(20));
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        Err(Error::new(ErrorCode::NoDevice, "not connected"))
    }

    #[test]
    fn operations_never_overlap() {
        let watch = Arc::new(Watch::new(slow_connect));
        let threads: Vec<_> = ["inspect", "install", "inspect", "restore_stock", "inspect"]
            .into_iter()
            .map(|operation| {
                let watch = Arc::clone(&watch);
                thread::spawn(move || watch.call(operation, json!({})))
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap().unwrap_err().code, "no_device");
        }
        assert_eq!(MOST_ACTIVE.load(Ordering::SeqCst), 1);
    }
}
