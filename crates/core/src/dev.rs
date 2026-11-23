//! Development operations (cargo feature `dev`, debug builds only): `read_file`, `stage_file`
//! and `remove_staged`, for experiments with fonts this build doesn't install.
//!
//! `stage_file` logs every id it hands out, with the file's name and SHA-256, as JSON lines in
//! `staged.jsonl` under `GEORGIAN_FOR_GARMIN_STATE_DIR` (default: the platform's per-user state
//! folder), so a later session or one-shot call can remove the file. `remove_staged` deletes
//! only a file whose name and contents match a logged id.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorCode};
use crate::extdata::{self, ExtData, Layout};
use crate::fonts::{Content, FontSet, sha256_hex};
use crate::mtp::Mtp;
use crate::names::{self, Role};
use crate::ops::{
    HiddenScan, Undo, extdata_path, hex_handle, missing_write_operations, read_garmin_device,
    roll_back, scan_hidden, write_verified,
};
use crate::ptp::ObjectInfo;
use crate::session::Control;

/// `staged.jsonl` in the per-user state folder: `GEORGIAN_FOR_GARMIN_STATE_DIR`, else
/// `~/Library/Application Support/georgian-for-garmin` (macOS),
/// `%LOCALAPPDATA%\georgian-for-garmin` (Windows) or `$XDG_STATE_HOME/georgian-for-garmin`
/// (`~/.local/state`, elsewhere).
pub fn default_log() -> Option<PathBuf> {
    let var = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = var("GEORGIAN_FOR_GARMIN_STATE_DIR").or_else(|| {
        let base = if cfg!(target_os = "macos") {
            var("HOME").map(|h| h.join("Library/Application Support"))
        } else if cfg!(windows) {
            var("LOCALAPPDATA")
        } else {
            var("XDG_STATE_HOME").or_else(|| var("HOME").map(|h| h.join(".local/state")))
        };
        base.map(|b| b.join("georgian-for-garmin"))
    })?;
    Some(dir.join("staged.jsonl"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// One line of the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Entry {
    Staged {
        id: String,
        at: u64,
        model: String,
        model_part_number: String,
        name: String,
        part: String,
        sha256: String,
        bytes: u64,
    },
    Removed {
        id: String,
        at: u64,
    },
}

fn log_error(log: &Path, e: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!("staged-file log {}: {e}", log.display()),
    )
}

/// The log's entries; none when it doesn't exist yet. Lines that don't parse are skipped.
fn read_log(log: &Path) -> Result<Vec<Entry>, Error> {
    match std::fs::read_to_string(log) {
        Ok(text) => Ok(text
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(log_error(log, e)),
    }
}

fn append(log: &Path, entry: &Entry) -> Result<(), Error> {
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir).map_err(|e| log_error(log, e))?;
    }
    let mut line = serde_json::to_string(entry).map_err(|e| log_error(log, e))?;
    line.push('\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| log_error(log, e))
}

fn removed_ids(entries: &[Entry]) -> BTreeSet<&str> {
    entries
        .iter()
        .filter_map(|e| match e {
            Entry::Removed { id, .. } => Some(id.as_str()),
            Entry::Staged { .. } => None,
        })
        .collect()
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::InvalidInput, message)
}

fn extdata(layout: &Layout) -> Result<&ExtData, Error> {
    layout.extdata.as_ref().ok_or_else(|| {
        Error::new(
            ErrorCode::ExtdataNotFound,
            "Could not find the watch's GARMIN/ExtData folder; nothing was written.",
        )
    })
}

/// Every file in `ExtData`, normal listing then hidden, with whether it is hidden.
fn all_files(dev: &mut dyn Mtp, layout: &Layout, ext: &ExtData) -> Vec<(u32, ObjectInfo, bool)> {
    let mut files: Vec<(u32, ObjectInfo, bool)> = ext
        .files
        .iter()
        .filter(|o| !o.info.is_folder())
        .map(|o| (o.handle, o.info.clone(), false))
        .collect();
    if let HiddenScan::Listed { objects, .. } = scan_hidden(dev, layout.storage, ext) {
        files.extend(objects.into_iter().map(|(h, i)| (h, i, true)));
    }
    files
}

/// Which file to read, and where to put it.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadFileInput {
    /// The file's name in GARMIN/ExtData (case-insensitive), from either listing.
    pub name: String,
    /// The local file to create; it must not exist.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadFileOutput {
    /// The name as the watch gives it.
    pub name: String,
    pub handle: String,
    pub hidden: bool,
    pub bytes: u64,
    pub sha256: String,
    pub path: String,
}

pub fn read_file(dev: &mut dyn Mtp, input: &ReadFileInput) -> Result<ReadFileOutput, Error> {
    let layout = extdata::discover(dev)?;
    let ext = extdata(&layout)?;
    let files = all_files(dev, &layout, ext);
    let named: Vec<&(u32, ObjectInfo, bool)> = files
        .iter()
        .filter(|f| f.1.filename.eq_ignore_ascii_case(&input.name))
        .collect();
    let (handle, info, hidden) = match named.as_slice() {
        [one] => *one,
        [] => {
            return Err(invalid(format!(
                "GARMIN/ExtData has no file named {}",
                input.name
            )));
        }
        _ => {
            return Err(Error::new(
                ErrorCode::Conflict,
                format!("GARMIN/ExtData has several files named {}", input.name),
            ));
        }
    };
    let bytes = dev.get_object(*handle)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&input.path)
        .and_then(|mut f| f.write_all(&bytes))
        .map_err(|e| invalid(format!("cannot create {}: {e}", input.path)))?;
    Ok(ReadFileOutput {
        name: info.filename.clone(),
        handle: hex_handle(*handle),
        hidden: *hidden,
        bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
        path: input.path.clone(),
    })
}

/// What to stage, and under which name.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageFileInput {
    /// The local file to stage.
    pub source: String,
    /// The name in GARMIN/ExtData: `<part>.tmp` for a part the watch declares under
    /// Garmin/ExtData, as `006-D8448-70.tmp`, or its short form `D844870.tmp`.
    #[schemars(regex(pattern = r"^(006-[Dd][0-9]{4}-[0-9]{2}|[Dd][0-9]{6})\.[Tt][Mm][Pp]$"))]
    pub name: String,
    /// Check everything and write nothing. Default false.
    pub dry_run: Option<bool>,
    /// SHA-256s (lowercase hex) of Garmin's own file for this part, from a pinned catalog: an
    /// existing file with one of them may be replaced.
    #[schemars(inner(regex(pattern = r"^[0-9a-f]{64}$")))]
    pub garmin_sha256: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageFileStatus {
    /// Written and read back with the same SHA-256.
    StagedVerified,
    /// The file under name already holds these bytes; nothing was written.
    AlreadyStaged,
    /// dry_run: every check passed; nothing was written.
    DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageAction {
    /// A new file (none under name).
    Create,
    /// The file under name is deleted and the new one written.
    Replace,
    /// Nothing to write.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Identified {
    /// Garmin's original: this build's stock font for the part, or a garmin_sha256.
    Garmin,
    /// One of this program's Georgian builds of the part.
    Georgian,
    /// A file this tool staged (in the log).
    Staged,
    /// The same bytes as source, waiting under the part's other name form.
    Same,
}

/// The file a stage replaces.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Replaced {
    pub name: String,
    pub handle: String,
    pub sha256: String,
    pub identified_as: Identified,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StageFileOutput {
    pub status: StageFileStatus,
    pub action: StageAction,
    /// What remove_staged takes: new for a write, the logged one for already_staged (null if
    /// this tool didn't stage that file), null for a dry run.
    pub id: Option<String>,
    pub name: String,
    /// "GARMIN/ExtData/<name>".
    pub path: String,
    /// The part number, e.g. "006-D8448-70".
    pub part: String,
    /// The staged file's handle; null for a dry run.
    pub handle: Option<String>,
    pub bytes: u64,
    pub sha256: String,
    pub replaces: Option<Replaced>,
}

/// What `stage_file` found in a file of the part.
enum Found {
    Same,
    Known(Identified),
    Unknown,
}

/// The file under the target name, if one is replaced: its handle and bytes.
struct Existing {
    handle: u32,
    original: Vec<u8>,
    replaced: Replaced,
}

pub fn stage_file(
    dev: &mut dyn Mtp,
    fonts: &FontSet,
    log: &Path,
    input: &StageFileInput,
    control: &Control<'_>,
) -> Result<StageFileOutput, Error> {
    let name = input.name.as_str();
    let path = extdata_path(name);
    let part = names::resolve(name)
        .filter(|f| f.role == Role::Waiting)
        .map(|f| f.part)
        .ok_or_else(|| {
            invalid(format!(
                "{name} is not <part>.tmp, as 006-D8448-70.tmp or D844870.tmp"
            ))
        })?;
    let bytes = std::fs::read(&input.source)
        .map_err(|e| invalid(format!("cannot read {}: {e}", input.source)))?;
    let sha256 = sha256_hex(&bytes);
    control.progress(0, 2, &format!("Checking {path}"));

    let missing = missing_write_operations(dev)?;
    if !missing.is_empty() {
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "The watch does not support MTP {}; nothing was written.",
                missing.join(", ")
            ),
        ));
    }
    let layout = extdata::discover(dev)?;
    let device = read_garmin_device(dev, &layout)?;
    if !device.declares_extdata_part(&part) {
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "{} ({}) doesn't declare part {part} under Garmin/ExtData; nothing was written.",
                device.description, device.part_number
            ),
        ));
    }
    let ext = extdata(&layout)?;

    let entries = read_log(log)?;
    let staged_hashes: BTreeSet<&str> = entries
        .iter()
        .filter_map(|e| match e {
            Entry::Staged {
                part: p, sha256, ..
            } if *p == part => Some(sha256.as_str()),
            _ => None,
        })
        .collect();
    let garmin: BTreeSet<String> = input
        .garmin_sha256
        .iter()
        .flatten()
        .map(|h| h.to_ascii_lowercase())
        .collect();
    let font = fonts.fonts().map(|(_, f)| f).find(|f| f.part == part);
    let identify = |current: &[u8]| -> Found {
        if current == bytes.as_slice() {
            return Found::Same;
        }
        match font.map(|f| f.identify(current)) {
            Some(Content::Stock) => return Found::Known(Identified::Garmin),
            Some(Content::Georgian | Content::EarlierGeorgian) => {
                return Found::Known(Identified::Georgian);
            }
            Some(Content::Other) | None => {}
        }
        let hash = sha256_hex(current);
        if garmin.contains(&hash) {
            Found::Known(Identified::Garmin)
        } else if staged_hashes.contains(hash.as_str()) {
            Found::Known(Identified::Staged)
        } else {
            Found::Unknown
        }
    };
    // The part's files, by the one name rule ([`names`]) in either name form. Of its waiting
    // files there may be one, under either name: identical (under name), identified (replaced)
    // or unknown (refused). An import in the normal listing (some watches show their `.fnt`
    // there) must be identified like a hidden one, but a failed read stops the call, as
    // install's does for a file in the normal listing.
    let mut existing: Option<Existing> = None;
    let mut already: Option<u32> = None;
    let mut waiting = Vec::new();
    for o in ext.files.iter().filter(|o| !o.info.is_folder()) {
        let Some(file) = names::resolve(&o.info.filename).filter(|f| f.part == part) else {
            continue;
        };
        match file.role {
            Role::Waiting => waiting.push(o),
            Role::Imported => {
                if matches!(identify(&dev.get_object(o.handle)?), Found::Unknown) {
                    return Err(unknown(&extdata_path(&o.info.filename), &part));
                }
            }
            Role::Bin => {}
        }
    }
    match waiting.as_slice() {
        [] => {}
        [one] => {
            let current = dev.get_object(one.handle)?;
            let under_name = one.info.filename.eq_ignore_ascii_case(name);
            let identified_as = match identify(&current) {
                Found::Same if under_name => {
                    already = Some(one.handle);
                    None
                }
                Found::Same => Some(Identified::Same),
                Found::Known(identified_as) => Some(identified_as),
                Found::Unknown => {
                    return Err(unknown(&extdata_path(&one.info.filename), &part));
                }
            };
            if let Some(identified_as) = identified_as {
                existing = Some(Existing {
                    handle: one.handle,
                    replaced: Replaced {
                        name: one.info.filename.clone(),
                        handle: hex_handle(one.handle),
                        sha256: sha256_hex(&current),
                        identified_as,
                    },
                    original: current,
                });
            }
        }
        many => {
            let paths: Vec<String> = many
                .iter()
                .map(|o| extdata_path(&o.info.filename))
                .collect();
            return Err(Error::new(
                ErrorCode::Conflict,
                format!(
                    "{} all wait for part {part}: a part takes one waiting file, so nothing was \
                     written. Remove the ones this tool staged with remove_staged, or unplug and \
                     restart the watch.",
                    paths.join(", ")
                ),
            ));
        }
    }
    // The imported fonts the next restart would replace: a file that can't be read is left to
    // the watch, as install does for a bitmap part.
    if let HiddenScan::Listed { objects, .. } = scan_hidden(dev, layout.storage, ext) {
        for (handle, info) in objects {
            let ours = names::resolve(&info.filename)
                .is_some_and(|f| f.part == part && f.role != Role::Bin);
            if ours
                && let Ok(current) = dev.get_object(handle)
                && matches!(identify(&current), Found::Unknown)
            {
                return Err(unknown(&extdata_path(&info.filename), &part));
            }
        }
    }

    let action = if already.is_some() {
        StageAction::None
    } else if existing.is_some() {
        StageAction::Replace
    } else {
        StageAction::Create
    };
    let logged_id = || {
        let removed = removed_ids(&entries);
        entries.iter().rev().find_map(|e| match e {
            Entry::Staged {
                id,
                name: n,
                sha256: s,
                ..
            } if n.eq_ignore_ascii_case(name) && *s == sha256 && !removed.contains(id.as_str()) => {
                Some(id.clone())
            }
            _ => None,
        })
    };
    let output = |status, id, handle: Option<u32>, replaces| StageFileOutput {
        status,
        action,
        id,
        name: name.to_owned(),
        path: path.clone(),
        part: part.clone(),
        handle: handle.map(hex_handle),
        bytes: bytes.len() as u64,
        sha256: sha256.clone(),
        replaces,
    };
    if input.dry_run.unwrap_or(false) {
        let id = if already.is_some() { logged_id() } else { None };
        return Ok(output(
            StageFileStatus::DryRun,
            id,
            None,
            existing.map(|e| e.replaced),
        ));
    }
    if let Some(handle) = already {
        let handle = Some(handle);
        control.progress(2, 2, &format!("{path}: already staged"));
        return Ok(output(
            StageFileStatus::AlreadyStaged,
            logged_id(),
            handle,
            None,
        ));
    }
    control.check_nothing_written()?;
    control.progress(1, 2, &format!("Writing {path}"));

    // Logged before writing: an id whose write failed is simply not found later.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let id = format!(
        "stg-{}",
        &sha256_hex(format!("{nanos}:{name}:{sha256}").as_bytes())[..12]
    );
    append(
        log,
        &Entry::Staged {
            id: id.clone(),
            at: now(),
            model: device.description.clone(),
            model_part_number: device.part_number.clone(),
            name: name.to_owned(),
            part: part.clone(),
            sha256: sha256.clone(),
            bytes: bytes.len() as u64,
        },
    )?;
    let storage = layout.storage;
    let (handle, replaces) = match existing {
        None => (
            write_verified(dev, storage, ext, name, &bytes)
                .map_err(|e| roll_back(dev, storage, ext, &[], e, STAGE_HINT))?,
            None,
        ),
        Some(Existing {
            handle,
            original,
            replaced,
        }) => {
            let undo = [Undo::Replaced {
                name: replaced.name.clone(),
                original,
            }];
            let written = dev
                .delete_object(handle)
                .map_err(|e| {
                    let e = Error::from(e);
                    Error::new(
                        e.code,
                        format!(
                            "Could not delete {} ({}) to replace it: {}.",
                            extdata_path(&replaced.name),
                            hex_handle(handle),
                            e.message
                        ),
                    )
                })
                .and_then(|()| write_verified(dev, storage, ext, name, &bytes));
            match written {
                Ok(new) => (new, Some(replaced)),
                Err(e) => return Err(roll_back(dev, storage, ext, &undo, e, STAGE_HINT)),
            }
        }
    };
    // A staged file this call replaced is gone: its ids are removed.
    if let Some(replaced) = replaces
        .as_ref()
        .filter(|r| r.identified_as == Identified::Staged)
    {
        let removed = removed_ids(&entries);
        for entry in &entries {
            if let Entry::Staged {
                id: old,
                name: n,
                sha256: s,
                ..
            } = entry
                && n.eq_ignore_ascii_case(&replaced.name)
                && *s == replaced.sha256
                && !removed.contains(old.as_str())
            {
                let entry = Entry::Removed {
                    id: old.clone(),
                    at: now(),
                };
                append(log, &entry)?;
            }
        }
    }
    control.progress(2, 2, &format!("{path}: staged and verified"));
    Ok(output(
        StageFileStatus::StagedVerified,
        Some(id),
        Some(handle),
        replaces,
    ))
}

/// What to do after stage_file couldn't undo all of its changes.
const STAGE_HINT: &str = "Connect the watch and look at GARMIN/ExtData with extdata_list: \
    remove a file this tool staged with remove_staged, or stage Garmin's original for the part \
    again with stage_file.";

fn unknown(path: &str, part: &str) -> Error {
    Error::new(
        ErrorCode::Conflict,
        format!(
            "{path} is neither Garmin's original for part {part} (by this build's fonts or \
             garmin_sha256), one of this program's Georgian builds, nor a file this tool staged; \
             nothing was written."
        ),
    )
}

/// Which staged file to remove.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveStagedInput {
    /// The id stage_file returned.
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemoveStatus {
    /// Deleted from the watch.
    Removed,
    /// This tool removed it before.
    AlreadyRemoved,
    /// The watch has no file with that name and contents (imported at a restart, deleted
    /// elsewhere, or never written).
    NotFound,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RemoveStagedOutput {
    pub id: String,
    pub status: RemoveStatus,
    pub name: String,
    pub path: String,
}

/// The logged file for `id`: its name and SHA-256, and whether it was removed.
fn logged(log: &Path, id: &str) -> Result<(String, String, bool), Error> {
    let entries = read_log(log)?;
    let removed = removed_ids(&entries).contains(id);
    entries
        .iter()
        .find_map(|e| match e {
            Entry::Staged {
                id: i,
                name,
                sha256,
                ..
            } if i == id => Some((name.clone(), sha256.clone(), removed)),
            _ => None,
        })
        .ok_or_else(|| {
            invalid(format!(
                "{id} is not an id stage_file logged in {}",
                log.display()
            ))
        })
}

/// `already_removed` for an id this tool removed before, without touching the watch.
pub fn already_removed(
    log: &Path,
    input: &RemoveStagedInput,
) -> Result<Option<RemoveStagedOutput>, Error> {
    let (name, _, removed) = logged(log, &input.id)?;
    Ok(removed.then(|| RemoveStagedOutput {
        id: input.id.clone(),
        status: RemoveStatus::AlreadyRemoved,
        path: extdata_path(&name),
        name,
    }))
}

/// Deletes the part's waiting files (either name form, [`names`]) that hold the bytes `id`
/// staged. A file under the logged name with other bytes is refused; another waiting file of
/// the part is left alone.
pub fn remove_staged(
    dev: &mut dyn Mtp,
    log: &Path,
    input: &RemoveStagedInput,
) -> Result<RemoveStagedOutput, Error> {
    let (name, sha256, _) = logged(log, &input.id)?;
    let path = extdata_path(&name);
    let part = names::resolve(&name).map(|f| f.part);
    let layout = extdata::discover(dev)?;
    let ext = extdata(&layout)?;
    let waiting = ext.files.iter().filter(|o| {
        part.as_deref()
            .is_some_and(|p| names::is(&o.info.filename, p, Role::Waiting))
    });
    let (mut ours, mut changed) = (Vec::new(), None);
    for o in waiting {
        if sha256_hex(&dev.get_object(o.handle)?) == sha256 {
            ours.push(o);
        } else if o.info.filename.eq_ignore_ascii_case(&name) {
            changed = Some(o.handle);
        }
    }
    if let (true, Some(handle)) = (ours.is_empty(), changed) {
        return Err(Error::new(
            ErrorCode::Conflict,
            format!(
                "{path} ({}) now holds other bytes than {} staged; nothing was deleted.",
                hex_handle(handle),
                input.id
            ),
        ));
    }
    let output = |status| RemoveStagedOutput {
        id: input.id.clone(),
        status,
        name: name.clone(),
        path: path.clone(),
    };
    if ours.is_empty() {
        return Ok(output(RemoveStatus::NotFound));
    }
    for o in ours {
        dev.delete_object(o.handle)
            .map_err(|e| discard_note(&extdata_path(&o.info.filename), o.handle, Error::from(e)))?;
    }
    append(
        log,
        &Entry::Removed {
            id: input.id.clone(),
            at: now(),
        },
    )?;
    Ok(output(RemoveStatus::Removed))
}

fn discard_note(path: &str, handle: u32, cause: Error) -> Error {
    Error::new(
        cause.code,
        format!(
            "Could not delete {path} ({}): {}",
            hex_handle(handle),
            cause.message
        ),
    )
}
