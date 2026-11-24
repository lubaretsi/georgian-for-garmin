//! Georgian for Garmin: a desktop window over the core operations.
//!
//! The frontend (`frontend/`, built into `frontend/dist`) calls the commands below through its
//! `TauriBackend`. Each watch command runs one core operation on a blocking thread, and a mutex
//! keeps the watch to one operation at a time, so the frontend's `inspect` polling never
//! overlaps an install. Commands return core output unchanged, and errors as [`Failure`]: the
//! core's code and message, who holds a busy watch, and how a failed write was undone; the
//! frontend turns codes into words. `install` and `restore_stock` report their progress over a
//! channel.
//! `share_report` opens the GitHub issue form filled in by the core's `issue_link`, `open_issue`
//! a watch's issue from the support list and `open_watch_list` the README's list of watches;
//! `share` draws the check screen's QR code and test-mark pictures. Diagnostic info is saved to
//! the Downloads folder or copied, never sent.
//!
//! Debug builds print the JSON Schemas of the app's own types with `--schemas`, for the
//! frontend's `pnpm gen:types`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(feature = "demo", not(debug_assertions)))]
compile_error!(
    "the `demo` feature replaces the watch with a fake one; use it in debug builds only"
);

#[cfg(any(test, feature = "demo"))]
mod demo;
mod settings;
mod share;
mod window;

use std::borrow::Cow;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use georgian_for_garmin_core::busy::Busy;
use georgian_for_garmin_core::error::Rollback;
use georgian_for_garmin_core::fonts::FontSet;
use georgian_for_garmin_core::mtp::Mtp;
use georgian_for_garmin_core::session::{self, Control, Session};
use georgian_for_garmin_core::{Error, ErrorCode, ops, report};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

use settings::ShareKey;

/// An operation failure as the frontend receives it; `busy` only for `device_busy`.
#[derive(Debug, Serialize, JsonSchema)]
struct Failure {
    code: ErrorCode,
    message: String,
    /// `device_busy`: who holds the watch, as far as the system tells.
    #[serde(skip_serializing_if = "Option::is_none")]
    busy: Option<Box<Busy>>,
    /// A failed install or restore: how undoing it went.
    #[serde(skip_serializing_if = "Option::is_none")]
    rollback: Option<Box<Rollback>>,
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        Failure {
            code: e.code,
            message: e.message,
            busy: e.busy,
            rollback: e.rollback,
        }
    }
}

impl Failure {
    /// A failure of the app itself, outside the core's operations.
    fn internal(message: String) -> Self {
        Failure::from(Error::new(ErrorCode::Internal, message))
    }
}

/// One step of an install or restore: `done` of `total` files written.
#[derive(Debug, Clone, Serialize, JsonSchema)]
struct Progress {
    done: u32,
    total: u32,
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

    /// Runs `operation` while holding the watch, opening it for this call only; `progress`
    /// gets the steps of a long operation.
    fn call(
        &self,
        operation: &str,
        input: Value,
        progress: Option<&dyn Fn(session::Progress)>,
    ) -> Result<Value, Failure> {
        let _busy = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        let mut connect = self.connect;
        let mut session = Session::one_shot(Cow::Borrowed(&self.fonts), Box::new(&mut connect));
        // The demo's `diagnose` lists its own watch and drive, not this computer's.
        #[cfg(feature = "demo")]
        {
            session = session.with_probe(demo::probe());
        }
        let result = session.call(operation, input, &Control::new(progress, None));
        drop(session);
        // The demo watch restarts after every write, as the app asks the owner to.
        #[cfg(feature = "demo")]
        if result.is_ok() && matches!(operation, "install" | "restore_stock") {
            demo::written();
        }
        Ok(result?)
    }

    /// Runs `operation` on a blocking thread so USB work never stalls the window, sending the
    /// files written so far to `progress`.
    async fn run(
        self: Arc<Self>,
        operation: &'static str,
        input: Value,
        progress: Option<Channel<Progress>>,
    ) -> Result<Value, Failure> {
        tauri::async_runtime::spawn_blocking(move || {
            let send = |step: session::Progress| {
                if let (Some(channel), Some(total)) = (&progress, step.total) {
                    // A closed window has no one left to tell.
                    let _ = channel.send(Progress {
                        done: step.progress as u32,
                        total: total as u32,
                    });
                }
            };
            self.call(operation, input, Some(&send))
        })
        .await
        .map_err(|e| Failure::internal(format!("{operation} stopped unexpectedly: {e}")))?
    }
}

#[tauri::command]
async fn inspect(watch: State<'_, Arc<Watch>>) -> Result<Value, Failure> {
    Arc::clone(&watch).run("inspect", json!({}), None).await
}

/// Stages the Georgian font in every part of the bundled packs that the watch lists.
#[tauri::command]
async fn install(
    watch: State<'_, Arc<Watch>>,
    on_progress: Channel<Progress>,
) -> Result<Value, Failure> {
    Arc::clone(&watch)
        .run("install", json!({}), Some(on_progress))
        .await
}

/// Stages Garmin's original font in every part of the bundled packs that the watch lists.
#[tauri::command]
async fn restore_stock(
    watch: State<'_, Arc<Watch>>,
    on_progress: Channel<Progress>,
) -> Result<Value, Failure> {
    Arc::clone(&watch)
        .run("restore_stock", json!({}), Some(on_progress))
        .await
}

/// Reads a watch mounted as a USB drive (the demo build's is a folder it makes).
#[tauri::command]
async fn inspect_drive(watch: State<'_, Arc<Watch>>) -> Result<Value, Failure> {
    #[cfg(feature = "demo")]
    let input = match demo::drive() {
        Some(path) => json!({ "path": path }),
        None => {
            return Err(Failure::from(Error::new(
                ErrorCode::NoDevice,
                "Demo: no watch is mounted as a drive.",
            )));
        }
    };
    #[cfg(not(feature = "demo"))]
    let input = json!({});
    Arc::clone(&watch).run("inspect_drive", input, None).await
}

/// The core's `diagnose` for `input` (`{events}`, the app's recent steps), through the shared
/// watch like `inspect`. A device problem is data in its output, not a failure.
#[tauri::command]
async fn diagnose(watch: State<'_, Arc<Watch>>, input: Value) -> Result<Value, Failure> {
    Arc::clone(&watch).run("diagnose", input, None).await
}

/// The issue link for `input` (`issue_link`'s input), without opening it: what the share
/// screen shows as what will be shared.
#[tauri::command]
fn issue_link(watch: State<'_, Arc<Watch>>, input: Value) -> Result<Value, Failure> {
    Ok(ops::call("issue_link", input, &watch.fonts, &mut || {
        Err(Error::new(
            ErrorCode::Internal,
            "issue_link opened the watch",
        ))
    })?)
}

/// `share_report`'s result.
#[derive(Debug, Serialize, JsonSchema)]
struct Shared {
    /// The whole report is in the link; otherwise it is on the clipboard and the form asks
    /// for a paste.
    complete: bool,
    /// The link opened (the demo build opens nothing and shows it instead).
    url: String,
}

/// Opens `url` in the browser; the demo build opens nothing.
fn open_url(app: &AppHandle, url: &str) -> Result<(), Failure> {
    if cfg!(feature = "demo") {
        return Ok(());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| Failure::internal(format!("could not open the browser: {e}")))
}

/// Opens the GitHub issue form filled in from `input` (`issue_link`'s input). A report too
/// long for the link goes to the clipboard. The demo build touches neither the clipboard nor
/// the browser.
#[tauri::command]
fn share_report(
    app: AppHandle,
    watch: State<'_, Arc<Watch>>,
    input: Value,
) -> Result<Shared, Failure> {
    let link = issue_link(watch, input)?;
    let complete = link["report_in_url"] == true;
    let url = link["url"].as_str().unwrap_or_default().to_owned();
    if !complete && !cfg!(feature = "demo") {
        let text = link["report_text"].as_str().unwrap_or_default().to_owned();
        app.clipboard()
            .write_text(text)
            .map_err(|e| Failure::internal(format!("could not copy the report: {e}")))?;
    }
    open_url(&app, &url)?;
    Ok(Shared { complete, url })
}

/// Opens this project's GitHub issue `issue` (a watch's, from the support list) in the browser
/// and returns its link. The demo build opens nothing.
#[tauri::command]
fn open_issue(app: AppHandle, issue: u32) -> Result<String, Failure> {
    let url = report::issue_url(issue);
    open_url(&app, &url)?;
    Ok(url)
}

/// The README's list of watches and how each does.
const WATCH_LIST: &str = "https://github.com/lubaretsi/georgian-for-garmin#supported-watches";

/// Opens the README's watch list in the browser and returns its link; no other page. The demo
/// build opens nothing.
#[tauri::command]
fn open_watch_list(app: AppHandle) -> Result<String, Failure> {
    open_url(&app, WATCH_LIST)?;
    Ok(WATCH_LIST.to_owned())
}

/// The phone test page in `language`, with its QR code.
#[tauri::command]
fn phone_page(language: String) -> share::PhonePage {
    share::phone_page(&language)
}

/// The test marks' pictures; empty in stable builds.
#[tauri::command]
fn test_marks(watch: State<'_, Arc<Watch>>) -> Vec<share::MarkPicture> {
    share::mark_pictures(&watch.fonts)
}

/// Whether this build talks to the in-memory demo watch.
#[tauri::command]
fn demo_mode() -> bool {
    cfg!(feature = "demo")
}

/// The diagnostic files this run saved: the only ones `reveal_file` shows.
#[derive(Default)]
struct Saved(Mutex<Vec<PathBuf>>);

/// The diagnostic info file's name for local time `now`:
/// `georgian-for-garmin-diagnostics-2026-11-24-031405.txt`.
fn diagnostics_name(now: chrono::NaiveDateTime) -> String {
    format!(
        "georgian-for-garmin-diagnostics-{}.txt",
        now.format("%Y-%m-%d-%H%M%S")
    )
}

/// Writes `text` to a new file at `path`; an existing file is never replaced.
fn write_new(path: &Path, text: &str) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(text.as_bytes())
}

/// Saves diagnostic info `text` to the Downloads folder as a new file named for the local
/// date and time, and returns its path. The demo build saves to a folder in the temp
/// directory.
#[tauri::command]
fn save_diagnostics(
    app: AppHandle,
    saved: State<'_, Saved>,
    text: String,
) -> Result<String, Failure> {
    #[cfg(feature = "demo")]
    let dir = {
        let _ = &app;
        demo::downloads()
    };
    #[cfg(not(feature = "demo"))]
    let dir = app
        .path()
        .download_dir()
        .map_err(|e| Failure::internal(format!("no Downloads folder: {e}")))?;
    let path = dir.join(diagnostics_name(chrono::Local::now().naive_local()));
    fs::create_dir_all(&dir)
        .and_then(|()| write_new(&path, &text))
        .map_err(|e| Failure::internal(format!("could not save {}: {e}", path.display())))?;
    let shown = path.to_string_lossy().into_owned();
    saved
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(path);
    Ok(shown)
}

/// Copies `text` to the clipboard. The demo build leaves the clipboard alone.
#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), Failure> {
    if cfg!(feature = "demo") {
        return Ok(());
    }
    app.clipboard()
        .write_text(text)
        .map_err(|e| Failure::internal(format!("could not copy: {e}")))
}

/// Shows `path`, a diagnostic file this run saved, selected in Finder, Explorer or the file
/// manager. The demo build opens nothing.
#[tauri::command]
fn reveal_file(app: AppHandle, saved: State<'_, Saved>, path: String) -> Result<(), Failure> {
    let path = PathBuf::from(path);
    let known = saved
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&path);
    if !known {
        return Err(Failure::from(Error::new(
            ErrorCode::InvalidInput,
            format!("{} is not a file this app saved", path.display()),
        )));
    }
    if cfg!(feature = "demo") {
        return Ok(());
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| Failure::internal(format!("could not show {}: {e}", path.display())))
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, Failure> {
    app.path()
        .app_config_dir()
        .map_err(|e| Failure::internal(format!("no config folder: {e}")))
}

/// Changes the saved settings with `change`.
fn update_settings(
    app: &AppHandle,
    change: impl FnOnce(&mut settings::Settings),
) -> Result<(), Failure> {
    let dir = config_dir(app)?;
    let mut saved = settings::load(&dir);
    change(&mut saved);
    settings::save(&dir, &saved).map_err(|e| {
        Failure::internal(format!(
            "could not save {}: {e}",
            dir.join(settings::FILE).display()
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
    update_settings(&app, |s| s.language = Some(language))
}

/// Whether the user already shared `key` (a model, its software and a result) from this app.
/// The demo build remembers its shares for this run only.
#[tauri::command]
fn shared_before(app: AppHandle, key: ShareKey) -> bool {
    #[cfg(feature = "demo")]
    {
        let _ = app;
        demo::shared_before(&key)
    }
    #[cfg(not(feature = "demo"))]
    config_dir(&app).is_ok_and(|dir| settings::load(&dir).shared.contains(&key))
}

/// Remembers that the user shared `key`.
#[tauri::command]
fn mark_shared(app: AppHandle, key: ShareKey) -> Result<(), Failure> {
    #[cfg(feature = "demo")]
    {
        let _ = app;
        demo::mark_shared(key);
        Ok(())
    }
    #[cfg(not(feature = "demo"))]
    update_settings(&app, |s| {
        s.mark_shared(key);
    })
}

/// The desktops the app is built for, as the frontend names them for platform-specific words.
#[cfg(any(test, feature = "demo"))]
const PLATFORMS: [&str; 3] = ["macos", "windows", "linux"];

/// The desktop this build runs on: `macos`, `windows` or `linux`. The demo build takes
/// `GEORGIAN_FOR_GARMIN_DEMO_PLATFORM` instead, to show another platform's words.
#[tauri::command]
fn platform() -> &'static str {
    #[cfg(feature = "demo")]
    if let Some(platform) = demo::platform() {
        return platform;
    }
    built_for()
}

/// This build's desktop; any other Unix gets Linux's words.
fn built_for() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    }
}

/// The JSON Schemas of the app's own types by name, which the frontend's `pnpm gen:types`
/// turns into TypeScript beside the core's.
#[cfg(debug_assertions)]
fn schemas() -> Value {
    json!({
        "Failure": schemars::schema_for!(Failure),
        "Progress": schemars::schema_for!(Progress),
        "Shared": schemars::schema_for!(Shared),
        "PhonePage": schemars::schema_for!(share::PhonePage),
        "MarkPicture": schemars::schema_for!(share::MarkPicture),
        "ShareKey": schemars::schema_for!(ShareKey),
    })
}

#[cfg(feature = "demo")]
const CONNECT: fn() -> Result<Box<dyn Mtp>, Error> = demo::connect;
#[cfg(not(feature = "demo"))]
const CONNECT: fn() -> Result<Box<dyn Mtp>, Error> = georgian_for_garmin_core::connect;

fn main() {
    #[cfg(debug_assertions)]
    if std::env::args().nth(1).as_deref() == Some("--schemas") {
        println!("{:#}", schemas());
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Watch::new(CONNECT)))
        .manage(Saved::default())
        .setup(|app| {
            match app.get_webview_window("main") {
                Some(main) => window::open(&main),
                None => eprintln!("the main window is missing"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            inspect,
            install,
            restore_stock,
            inspect_drive,
            diagnose,
            issue_link,
            share_report,
            open_issue,
            open_watch_list,
            phone_page,
            test_marks,
            demo_mode,
            platform,
            saved_language,
            save_language,
            shared_before,
            mark_shared,
            save_diagnostics,
            copy_text,
            reveal_file
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
            (
                "support",
                schemars::schema_for!(georgian_for_garmin_core::support::SupportStatus).to_value(),
            ),
            (
                "afterRestart",
                schemars::schema_for!(ops::Variant).to_value(),
            ),
            ("results", schemars::schema_for!(report::Seen).to_value()),
            ("views", schemars::schema_for!(report::View).to_value()),
            ("phones", schemars::schema_for!(report::Phone).to_value()),
            (
                "state",
                schemars::schema_for!(ops::GeorgianState).to_value(),
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
        // The app's answer list is the core's, in the form's order.
        let js = include_str!("../ui/app.js");
        let results: Vec<String> = report::Seen::ALL
            .iter()
            .map(|s| {
                serde_json::to_value(s)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        let listed = format!(
            "const RESULTS = [\n  \"{}\",\n];",
            results.join("\",\n  \"")
        );
        assert!(js.contains(&listed), "app.js RESULTS should be:\n{listed}");
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
        for key in [
            "untestedBody",
            "predictedBody",
            "stagedFiles",
            "stagedOneFile",
            "stagedPack",
            "stagedTrueTypePack",
        ] {
            assert!(used.contains(key), "{key} not found in the UI");
        }
        for (language, table) in strings() {
            let missing: Vec<_> = used.iter().filter(|k| table.get(*k).is_none()).collect();
            assert!(missing.is_empty(), "{language} lacks {missing:?}");
        }
    }

    #[test]
    fn every_platform_has_its_own_words() {
        assert!(PLATFORMS.contains(&built_for()));
        for (language, table) in strings() {
            let express = table["waitingHelpExpress"].as_object().expect("a table");
            let keys: BTreeSet<&str> = express.keys().map(String::as_str).collect();
            assert_eq!(keys, PLATFORMS.into(), "{language}.waitingHelpExpress");
        }
    }

    /// Words of one platform's steps, which no other platform's words may use.
    const PLATFORM_WORDS: [(&str, &[&str]); 3] = [
        (
            "macos",
            &[
                "Activity Monitor",
                "menu bar",
                "მენიუს ზოლ",
                "Mac",
                "Terminal",
            ],
        ),
        (
            "windows",
            &[
                "Task Manager",
                "notification area",
                "შეტყობინებების არე",
                "დროის გვერდით",
                "Windows",
            ],
        ),
        ("linux", &["file manager", "ფაილების მენეჯერ", "Linux"]),
    ];

    /// Each platform's strings under `key` in `table` (a table by platform).
    fn by_platform<'a>(table: &'a Value, key: &str) -> Vec<(&'a str, Vec<(String, String)>)> {
        let platforms = table[key].as_object().expect("a table by platform");
        platforms
            .iter()
            .map(|(platform, words)| {
                let mut out = Vec::new();
                leaves(&format!("{key}.{platform}."), words, &mut out);
                (platform.as_str(), out)
            })
            .collect()
    }

    #[test]
    fn no_platform_shows_another_platforms_steps() {
        for (language, table) in strings() {
            for key in ["busy", "waitingHelpExpress"] {
                for (platform, words) in by_platform(&table, key) {
                    assert!(PLATFORMS.contains(&platform), "{language}.{key}.{platform}");
                    for (other, foreign) in PLATFORM_WORDS.iter().filter(|(p, _)| *p != platform) {
                        for (path, text) in &words {
                            let found: Vec<_> =
                                foreign.iter().filter(|w| text.contains(*w)).collect();
                            assert!(
                                found.is_empty(),
                                "{language}.{path} has {other}'s {found:?}"
                            );
                        }
                    }
                    if platform == "linux" {
                        for (path, text) in &words {
                            assert!(!text.contains("Garmin Express"), "{language}.{path}");
                        }
                    }
                }
            }
            // The generic busy message is for any platform.
            let generic = table["errors"]["device_busy"].as_str().unwrap();
            for (platform, foreign) in PLATFORM_WORDS {
                assert!(
                    !foreign.iter().any(|w| generic.contains(w)),
                    "{language}: {platform}"
                );
            }
        }
    }

    #[test]
    fn every_platform_has_words_for_every_holder() {
        let schema = schemars::schema_for!(georgian_for_garmin_core::busy::HolderKind).to_value();
        let mut kinds = BTreeSet::new();
        constants(&schema, &mut kinds);
        assert!(kinds.contains("unknown"), "unexpected schema: {schema}");
        for (language, table) in strings() {
            let busy = table["busy"].as_object().expect("a busy table");
            let platforms: BTreeSet<&str> = busy.keys().map(String::as_str).collect();
            assert_eq!(platforms, PLATFORMS.into(), "{language}.busy");
            for (platform, words) in busy {
                let words = words.as_object().expect("a table by holder kind");
                // Kinds without their own words get `other` (named) or `unknown`; this app
                // itself is never "another program", so it has its own words everywhere.
                for key in ["this_app", "other", "unknown"] {
                    assert!(
                        words.contains_key(key),
                        "{language}.busy.{platform} lacks {key}"
                    );
                }
                assert!(words["other"].as_str().unwrap().contains("{name}"));
                let strays: Vec<_> = words.keys().filter(|k| !kinds.contains(*k)).collect();
                assert!(strays.is_empty(), "{language}.busy.{platform}: {strays:?}");
            }
        }
        // Busy words ask for no Terminal commands: steps in the app and sharing only.
        let json = include_str!("../ui/strings.json");
        for word in ["pkill", "Terminal", "sudo"] {
            assert!(!json.contains(word), "strings.json asks for {word}");
        }
    }

    #[test]
    fn failures_keep_core_codes() {
        let failure = Failure::from(Error::new(ErrorCode::DeviceBusy, "held"));
        assert_eq!(
            serde_json::to_value(failure).unwrap(),
            json!({"code": "device_busy", "message": "held"})
        );
        let busy = Failure::from(Error::busy(Busy::unknown()));
        let value = serde_json::to_value(busy).unwrap();
        assert_eq!(value["code"], "device_busy");
        assert_eq!(value["busy"]["holder"]["kind"], "unknown");
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
                thread::spawn(move || watch.call(operation, json!({}), None))
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap().unwrap_err().code, ErrorCode::NoDevice);
        }
        assert_eq!(MOST_ACTIVE.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn diagnostics_get_a_dated_name_and_never_replace_a_file() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 24)
            .unwrap()
            .and_hms_opt(3, 14, 5)
            .unwrap();
        assert_eq!(
            diagnostics_name(now),
            "georgian-for-garmin-diagnostics-2026-11-24-031405.txt"
        );
        let dir =
            std::env::temp_dir().join(format!("georgian-for-garmin-save-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(diagnostics_name(now));
        write_new(&path, "first").unwrap();
        let refused = write_new(&path, "second").unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(debug_assertions)]
    #[test]
    fn every_app_type_has_a_schema() {
        let schemas = schemas();
        for name in [
            "Failure",
            "Progress",
            "Shared",
            "PhonePage",
            "MarkPicture",
            "ShareKey",
        ] {
            assert_eq!(schemas[name]["title"], name, "{schemas:#}");
        }
        // The frontend's `Failure.code` is the core's error code, not any string.
        assert_eq!(
            schemas["Failure"]["properties"]["code"]["$ref"],
            "#/$defs/ErrorCode"
        );
    }
}
