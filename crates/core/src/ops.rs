//! Operations: one declaration each (JSON Schema in and out, behavior hints), validated on
//! every call. CLI, app and any future adapter call [`call`].
//!
//! A watch gets Georgian in every part of this build's packs that it lists under
//! `Garmin/ExtData`, across all packs: `install`/`restore_stock` write each of them. A bitmap
//! part is staged as a new `<part>.tmp`; a TrueType part replaces the watch's `<part>.bin`.
//! `inspect_drive` reads a watch that connects as a USB drive instead, and `issue_link` makes
//! the link that shares a watch report in a GitHub issue (`report`).

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use schemars::{JsonSchema, Schema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::drive;
use crate::error::{Error, ErrorCode, RollbackAction, RollbackFile, RollbackStatus};
use crate::extdata::{self, ExtData, FoundBy, Layout, Object};
use crate::fonts::{Content, Font, FontSet, Hosts, Kind, Listed, Pack, sha256_hex};
use crate::garmin_device::{self, GarminDevice};
use crate::listing;
use crate::mtp::{CreateError, Mtp, MtpError};
use crate::names::{self, Role};
use crate::ptp::{self, ObjectInfo, op};
use crate::report::{
    self, Connection, HiddenFile, HiddenListing, HiddenStatus, IssueLink, IssueLinkInput,
    ListedFile, ReportInput, ReportedHosts, ReportedPart, WatchReport,
};
use crate::session::{self, Control, Session};
use crate::support::{self, Support};

/// This program's version, as reported in the watch report.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Whether `version`, as a watch declares a `.bin` file, is below 1.00. A watch reads a `.bin`'s
/// version from its 16-byte trailer; this program's Georgian builds before 0.3.0 had none, and
/// watches declared them below 1.00: 0.01 on the Forerunner 165 (#5), 0.00 on the fenix 6S Pro
/// (#6). Garmin's own fonts are 2.00 and up.
fn below_one(version: &str) -> bool {
    version
        .split_once('.')
        .is_some_and(|(major, _)| major.parse::<u32>() == Ok(0))
}

/// `inspect` takes no parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectInput {}

/// Where to look for a watch connected as a USB drive.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectDriveInput {
    /// The volume's root, e.g. "/Volumes/GARMIN" or "E:\\". Default: the one mounted volume
    /// that has `GARMIN/GarminDevice.xml`.
    #[serde(default)]
    pub path: Option<PathBuf>,
}

/// Which parts to stage.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageInput {
    /// Part numbers to stage, e.g. `["006-D9035-03"]`. Default: every part of this build's packs
    /// that the watch lists. A part this build doesn't carry is refused with `font_unavailable`,
    /// one the watch doesn't list with `unsupported_device`.
    #[serde(default)]
    #[schemars(length(min = 1), inner(pattern(r"^[0-9]{3}-[A-Z][0-9]{4}-[0-9]{2}$")))]
    pub parts: Option<BTreeSet<String>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeviceReport {
    /// Model name from `GarminDevice.xml`, e.g. "Venu 2".
    pub model: String,
    /// Garmin hardware part number, e.g. "006-B3703-00".
    pub part_number: String,
    /// Installed software version, e.g. "19.05".
    pub software_version: String,
}

/// Whether Georgian was seen working on this watch, for one part or for every part `install`
/// writes. Ordered from best to worst; the overall status is the worst of the parts'.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    /// Part: its pack's `tested` list has an entry for this model part number and software
    /// version that names the part. Overall: every target part is `tested`.
    Tested,
    /// Part: such entries exist for this model part number, but only on other software versions.
    /// Overall: every target part is `tested` or `tested_other_software`, at least one the latter.
    TestedOtherSoftware,
    /// Part: no entry for this model part number names it. Overall: at least one target part is
    /// `untested`, or the watch lists no part of this build's packs.
    Untested,
}

/// Whether every part `install` writes was verified on this watch.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TestReport {
    /// The worst of the target parts' statuses; `untested` if there are none.
    pub status: TestStatus,
    /// Software versions this model was verified on with any of the target parts, e.g.
    /// `["19.05"]`.
    pub verified_software: Vec<String>,
}

/// A part of a pack that the watch lists.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListedPart {
    pub part: String,
    /// Nominal pixel size of a bitmap part; null for a TrueType part.
    pub size_px: Option<u16>,
    /// Whether this part was verified on this model and software.
    pub tested: TestStatus,
}

/// A pack with a part the watch lists.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PackReport {
    pub id: String,
    /// Garmin's product name, e.g. "Armenian Font, NotoSans Regular".
    pub name: String,
    pub kind: Kind,
    /// The weight its stock glyphs are drawn at, e.g. 400; Georgian is drawn at it too.
    pub weight: u16,
    /// The pack's parts the watch lists, in pack order: what `install` and `restore_stock` write.
    pub parts: Vec<ListedPart>,
}

/// What a part's file in `GARMIN/ExtData` holds: `<part>.tmp` for a bitmap part (waiting to be
/// imported at the next restart; none afterwards), `<part>.bin` for a TrueType part (the font
/// the watch loads). Install, restore, inspect, the app and the report all use this one rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// No file has this name.
    None,
    /// This build's Georgian font.
    Georgian,
    /// A Georgian build an earlier release installed (a beta's test marks); install and
    /// restore replace it.
    GeorgianBeta,
    /// Garmin's stock font.
    Stock,
    /// A file this build doesn't know.
    Other,
    /// A TrueType part the watch declares, whose `.bin` it doesn't show over MTP: whether it is
    /// Garmin's or Georgian can't be seen (the Forerunner 165 and fenix 6S Pro show no files in
    /// `ExtData`).
    NotVisible,
    /// `ExtData` wasn't found or wasn't read (a watch mounted as a USB drive).
    Unknown,
}

/// Georgian on the watch, from the files of the parts `install` writes, summed up in one word.
/// The app's main screen, the CLI and the report all show this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GeorgianState {
    /// `install` writes nothing on this watch.
    Unsupported,
    /// None of the target parts holds Georgian: install it.
    NotInstalled,
    /// Bitmap: every target holds this build's Georgian and at least one `.tmp` waits for a
    /// restart.
    Waiting,
    /// Every target holds this build's Georgian in the font the watch uses: a TrueType part's
    /// `.bin`, or the `<part>.fnt` a bitmap part's `.tmp` was imported as.
    Installed,
    /// Some targets hold this build's Georgian, the rest Garmin's or nothing: install adds the
    /// rest.
    Partly,
    /// A target holds a Georgian build from an earlier release: install replaces it.
    Beta,
    /// Bitmap: Garmin's original waits as a `.tmp` for a restart, and no target holds Georgian.
    StockWaiting,
    /// The watch doesn't show a target's `.bin`, so whether Georgian is installed can't be
    /// seen; installing again is harmless.
    NotVisible,
    /// A target holds a file this build doesn't know, or bitmap targets wait with different
    /// fonts: install refuses until that is sorted out.
    Other,
    /// `ExtData` wasn't found or read.
    Unknown,
}

/// A target part's file for [`georgian_state`]: what it holds, and whether it is the font the
/// watch imported (from the hidden listing: a bitmap part's read-only `<part>.fnt`, a TrueType
/// part's hidden `.bin`) rather than one in the normal listing, where a bitmap part's `.tmp`
/// waits for the next restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFile {
    pub file: FileState,
    pub imported: bool,
}

/// The one rule behind [`GeorgianState`], from the target parts' files. A bitmap part's
/// waiting `.tmp` is what the watch loads at the next restart; without one, its imported
/// `<part>.fnt` is what it uses now, so imported Georgian is installed and imported stock is
/// Garmin's font. A file this build doesn't know is `other` whether it waits or is imported:
/// install would replace an imported one at the next restart, so it refuses either.
pub fn georgian_state(kind: Option<Kind>, targets: &[TargetFile]) -> GeorgianState {
    use FileState as F;
    let Some(kind) = kind.filter(|_| !targets.is_empty()) else {
        return GeorgianState::Unsupported;
    };
    let any = |s: F| targets.iter().any(|t| t.file == s);
    let all = |s: F| targets.iter().all(|t| t.file == s);
    let waiting = |s: F| kind == Kind::Bitmap && targets.iter().any(|t| !t.imported && t.file == s);
    if any(F::Unknown) {
        GeorgianState::Unknown
    } else if any(F::Other) || (waiting(F::Georgian) && waiting(F::Stock)) {
        GeorgianState::Other
    } else if any(F::GeorgianBeta) {
        GeorgianState::Beta
    } else if any(F::NotVisible) {
        GeorgianState::NotVisible
    } else if all(F::Georgian) {
        if waiting(F::Georgian) {
            GeorgianState::Waiting
        } else {
            GeorgianState::Installed
        }
    } else if waiting(F::Stock) && !any(F::Georgian) {
        GeorgianState::StockWaiting
    } else if any(F::Georgian) {
        GeorgianState::Partly
    } else {
        GeorgianState::NotInstalled
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FontReport {
    /// Id of the pack this font belongs to.
    pub pack: String,
    pub part: String,
    /// Its pack's kind.
    pub kind: Kind,
    /// Nominal pixel size of a bitmap part; null for a TrueType part.
    pub size_px: Option<u16>,
    /// The watch lists this part under `Garmin/ExtData`.
    pub listed: bool,
    /// `install` writes this part: the watch lists it, it is of the kind `hosts` chose, its pack
    /// is installable, and the watch supports the MTP writes needed.
    pub target: bool,
    /// `restore_stock` writes this part: as `target`, restore-only packs included.
    pub restore: bool,
    pub file: FileState,
    /// `file` is the font the watch imported, from the hidden listing: a bitmap part's
    /// read-only `<part>.fnt` (its `.tmp` after a restart) or a TrueType part's hidden `.bin`.
    /// False for a file in the normal listing, such as a `.tmp` waiting for a restart.
    pub imported: bool,
    /// Beta builds: the test mark's variant ("A", "B", …) drawn into this part's Georgian
    /// build; null in stable builds.
    pub mark: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StagedFile {
    pub name: String,
    /// MTP object handle, hexadecimal.
    pub handle: String,
    pub bytes: u32,
    /// SHA-256 of the file as read back from the watch. Only a file named for a part of this
    /// build, with the size of its stock, Georgian or an earlier Georgian build, is read; any
    /// other file (a multi-megabyte Chinese font, say) is null and never transferred, so
    /// inspecting stays fast.
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExtDataReport {
    /// MTP object handle of `GARMIN/ExtData`, hexadecimal.
    pub handle: String,
    /// MTP storage ID, hexadecimal.
    pub storage_id: String,
    /// Whether the watch lists the folder among `GARMIN`'s children (the Venu 2 does not).
    pub listed_under_garmin: bool,
    pub found_by: FoundBy,
    /// Every file in the folder.
    pub files: Vec<StagedFile>,
}

/// Which kind of font this build writes on this watch, and why.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HostChoice {
    /// `bitmap` when the watch lists any bitmap part of this build, else `truetype` when it
    /// lists a TrueType part; null when it lists neither.
    pub kind: Option<Kind>,
    /// The choice in a sentence, for people.
    pub reason: String,
    /// Parts of the other kind the watch lists, which `install` leaves alone, in registry order.
    pub skipped_parts: Vec<String>,
    /// Parts of restore-only packs the watch lists: `install` leaves them alone because their
    /// Georgian didn't render on a watch; `restore_stock` writes Garmin's originals.
    pub restore_only_parts: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InspectOutput {
    pub device: DeviceReport,
    /// The watch lists a part this build installs and supports the MTP writes needed.
    pub supported: bool,
    /// `restore_stock` has parts to write (restore-only packs included) and the watch supports
    /// the MTP writes needed.
    pub restorable: bool,
    /// Georgian on the watch, from the files of the parts `install` writes.
    pub state: GeorgianState,
    /// Whether every part `install` writes was verified on this model and software.
    pub tested: TestReport,
    /// The watch's status in the support list (tested, expected, unsupported, unknown), its
    /// issue, and whether a report from it adds information.
    pub support: Support,
    /// Which kind of font gets Georgian on this watch, and why.
    pub hosts: HostChoice,
    /// Every pack of that kind, in registry order, with a part the watch lists under
    /// `Garmin/ExtData`: what `install` writes. Empty if it lists none.
    pub packs: Vec<PackReport>,
    /// Every font part this build carries.
    pub fonts: Vec<FontReport>,
    /// The staging folder, or null if it could not be found.
    pub extdata: Option<ExtDataReport>,
    /// `ExtData`'s children the normal listing leaves out (Garmin's `GetObjectIds`). A part's
    /// file only this shows is identified from it, as install and restore do; none is ever
    /// deleted.
    pub extdata_hidden: HiddenListing,
    pub report: WatchReport,
}

/// A watch connected as a USB drive.
#[derive(Debug, Serialize, JsonSchema)]
pub struct InspectDriveOutput {
    /// The volume's root.
    pub path: String,
    pub device: DeviceReport,
    /// Which kind of font would get Georgian on this watch over MTP, and why.
    pub hosts: HostChoice,
    /// The watch's status in the support list, as in `inspect`.
    pub support: Support,
    /// Its report; `connection` is `usb_drive`, no part is a target and files are `unknown`.
    pub report: WatchReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    /// Written and read back byte for byte. A bitmap part's `.tmp` is imported when USB is
    /// disconnected and the watch restarts; a TrueType part's `.bin` replaced the watch's file
    /// (or was created where it had none), and a restart makes the watch reload it.
    StagedVerified,
    /// The identical file was already there; nothing was written.
    AlreadyStaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    Georgian,
    Stock,
}

impl Variant {
    fn bytes(self, font: &Font) -> &[u8] {
        match self {
            Variant::Georgian => &font.georgian,
            Variant::Stock => &font.stock,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StagedPart {
    pub part: String,
    /// Nominal pixel size of a bitmap part; null for a TrueType part.
    pub size_px: Option<u16>,
    pub status: StageStatus,
    /// Path on the watch.
    pub path: String,
    /// MTP object handle of the written file, hexadecimal.
    pub handle: String,
    pub bytes: u64,
    pub sha256: String,
}

/// The parts of one pack that a call staged.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StagedPack {
    pub id: String,
    /// Garmin's product name, e.g. "Armenian Font, NotoSans Regular".
    pub name: String,
    pub kind: Kind,
    /// The weight its stock glyphs are drawn at, e.g. 400.
    pub weight: u16,
    /// Each part, in pack order.
    pub parts: Vec<StagedPart>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StageOutput {
    /// `already_staged` when every part already was; otherwise `staged_verified`.
    pub status: StageStatus,
    pub variant: Variant,
    /// Each pack with a part staged, in registry order.
    pub packs: Vec<StagedPack>,
    /// What the user does next.
    pub next_step: String,
}

/// An operation's declaration.
pub struct Operation {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub open_world: bool,
    input: fn() -> Schema,
    output: fn() -> Schema,
}

impl Operation {
    pub fn input_schema(&self) -> Value {
        (self.input)().to_value()
    }

    pub fn output_schema(&self) -> Value {
        (self.output)().to_value()
    }

    /// The MCP tool definition.
    pub fn tool(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input_schema(),
            "outputSchema": self.output_schema(),
            "annotations": {
                "title": self.title,
                "readOnlyHint": self.read_only,
                "destructiveHint": self.destructive,
                "idempotentHint": self.idempotent,
                "openWorldHint": self.open_world,
            },
        })
    }
}

// Behavior hints: the operations touch one attached watch (closed world). Staging a bitmap
// part never replaces a file (a different one is a conflict). A TrueType part replaces the
// watch's `<part>.bin`, so install and restore are destructive, but only a file identical to
// Garmin's original or this build's Georgian one, both of which restore_stock and install can
// write back. Repeating either returns `already_staged`. `inspect_drive` only reads a mounted
// volume, and `issue_link` computes a link from its input without touching anything.
// `extdata_list` and `wait_for_watch` only read; `extdata_list` writes a local file only when
// asked to.
static OPERATIONS: [Operation; 7] = [
    Operation {
        name: "inspect",
        title: "Inspect watch",
        description: "Read the connected watch's model and software version, which kind of font \
                      gets Georgian on it and why (bitmap fonts when the watch lists any of this \
                      build's, otherwise TrueType), the font packs of that kind this build installs \
                      (in registry order) with each listed part and whether it was tested on this \
                      model and software, the overall tested status, the watch's status in the \
                      support list (tested, expected, unsupported or unknown, with its GitHub \
                      issue and whether a report from it adds information), \
                      what each part's file in the \
                      hidden GARMIN/ExtData folder holds (only files that may be this build's are \
                      read), and a shareable report of the watch's font list without its unit ID or \
                      serial number. Read-only.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(InspectInput),
        output: || schema_for!(InspectOutput),
    },
    Operation {
        name: "install",
        title: "Install Georgian font",
        description: "Write the Georgian build of every part the watch lists of this build's packs \
                      of the kind inspect's hosts names (bitmap if the watch lists any bitmap part, \
                      else TrueType), across those packs (or only parts), each verified by reading \
                      it back: a bitmap part is staged as GARMIN/ExtData/<part>.tmp, a TrueType \
                      part replaces GARMIN/ExtData/<part>.bin (or is written next to a hidden \
                      read-only import, which is never deleted). Every target's file is first \
                      identified by content, from the normal listing or Garmin's GetObjectIds: \
                      Garmin's original or one of this program's builds is replaced, identical \
                      files are skipped, and anything else fails the whole call before anything \
                      is written (conflict). A file that can't be read is written over only when \
                      the watch declares Garmin's version for it; below 1.00 (this program's \
                      trailer-less builds) the call fails with restore_first, any other version \
                      with unsupported_device. If a part fails or the call is cancelled, it tries \
                      to undo its changes (new files deleted, replaced files written back from \
                      the bytes read before) and the error's rollback says whether every step \
                      worked (complete or incomplete, per file). Disconnect USB and restart the \
                      watch to load them.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        input: || schema_for!(StageInput),
        output: || schema_for!(StageOutput),
    },
    Operation {
        name: "restore_stock",
        title: "Restore stock font",
        description: "Write Garmin's original font for every part of this build's packs that the \
                      watch lists, across all packs (or only parts), as GARMIN/ExtData/<part>.tmp \
                      (bitmap) or over GARMIN/ExtData/<part>.bin (TrueType), with the same \
                      identification, readback and rollback as install, except that a file it \
                      can't read is also written over when the watch declares it below 1.00, as \
                      watches declared this program's trailer-less builds: restore only ever \
                      writes Garmin's own file. Disconnect USB and restart the watch to load \
                      them.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        input: || schema_for!(StageInput),
        output: || schema_for!(StageOutput),
    },
    Operation {
        name: "inspect_drive",
        title: "Inspect watch connected as a USB drive",
        description: "Read a watch that connects as a USB drive (mass storage) instead of MTP: \
                      its GARMIN/GarminDevice.xml from the mounted volume (the one found, or \
                      path), the kind of font that would get Georgian over MTP and why, and the \
                      same shareable report as inspect (connection usb_drive; the ExtData listing \
                      without hashes). Nothing can be installed this way. Read-only.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(InspectDriveInput),
        output: || schema_for!(InspectDriveOutput),
    },
    Operation {
        name: "issue_link",
        title: "Make a watch report link",
        description: "Make the GitHub new-issue link that fills the watch report form from a \
                      report (from inspect or inspect_drive), the last install or restore outcome \
                      and the owner's answers (what the test notification showed, where, phone, \
                      beta marker). The report text is rebuilt from the report's allowlisted \
                      fields. If the link would pass 8000 characters, the report is left out of \
                      it, report_in_url is false, and the form's report field is left empty for \
                      a paste of report_text. Touches nothing.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(IssueLinkInput),
        output: || schema_for!(IssueLink),
    },
    Operation {
        name: "extdata_list",
        title: "List the watch's ExtData folder",
        description: "List every file in the watch's hidden GARMIN/ExtData folder: the normal \
                      MTP listing plus, when the watch lists Garmin's GetObjectIds (0x9002), the \
                      files it leaves out there (on the Venu 2 the read-only fonts it imported), \
                      each with its name, handle, size, whether it is hidden and whether the \
                      watch marks it read-only. hash names files to read and give a SHA-256 for \
                      (the watch sends their whole contents; hashes are kept for the session). \
                      The list is sorted by name and pages with offset and limit; output_file \
                      writes the whole list to a local JSON file instead. Read-only.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(listing::ExtdataListInput),
        output: || schema_for!(listing::ExtdataListOutput),
    },
    Operation {
        name: "wait_for_watch",
        title: "Wait for the watch",
        description: "Wait until a Garmin watch in MTP mode can be opened, for example after it \
                      was unplugged and restarted, checking twice a second for up to \
                      timeout_seconds. Returns at once when the session already holds a working \
                      connection. Other failures than no watch (device_busy, permission_denied, \
                      multiple_devices) end the wait with that error. Reports progress and stops \
                      on cancellation. Read-only.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(session::WaitInput),
        output: || schema_for!(session::WaitOutput),
    },
];

// Development operations, only in builds with the `dev` feature (debug builds only): reading a
// file to the computer, staging any file under a part the watch declares, and removing only
// what this tool staged.
#[cfg(feature = "dev")]
static DEV_OPERATIONS: [Operation; 3] = [
    Operation {
        name: "read_file",
        title: "Read a file from ExtData (dev)",
        description: "Copy one file from the watch's GARMIN/ExtData folder (normal or hidden \
                      listing, by name) to a new local file at path; an existing local file is \
                      never overwritten. Returns its size and SHA-256. Development builds only. \
                      Reads the watch only.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(crate::dev::ReadFileInput),
        output: || schema_for!(crate::dev::ReadFileOutput),
    },
    Operation {
        name: "stage_file",
        title: "Stage a local file in ExtData (dev)",
        description: "Write the local file source as GARMIN/ExtData/<name>, where name is \
                      <part>.tmp for a part the watch declares under Garmin/ExtData, as \
                      006-D8448-70.tmp or its short form D844870.tmp, and read it back by \
                      SHA-256. Every existing file for that part is checked first, like install: \
                      a file under name is replaced only when it is identical (already_staged), \
                      Garmin's original or this build's Georgian font (by content), one this tool \
                      staged, or has a SHA-256 in garmin_sha256; an imported <part>.fnt (in \
                      either listing) must be one of those too, and another waiting .tmp for the \
                      part is a conflict. A failed write is undone where possible; the error's \
                      rollback says if it couldn't be. dry_run checks everything and writes nothing. The \
                      returned id, logged on this computer, is what remove_staged takes. Reports \
                      progress and stops on cancellation. Development builds only.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        input: || schema_for!(crate::dev::StageFileInput),
        output: || schema_for!(crate::dev::StageFileOutput),
    },
    Operation {
        name: "remove_staged",
        title: "Remove a staged file (dev)",
        description: "Delete the file stage_file staged under id, from any session: only while \
                      the watch's file under that name still has the staged SHA-256 (otherwise \
                      conflict, nothing deleted). not_found when it is gone, for example imported \
                      at a restart; already_removed when this tool removed it before. Development \
                      builds only.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        input: || schema_for!(crate::dev::RemoveStagedInput),
        output: || schema_for!(crate::dev::RemoveStagedOutput),
    },
];

#[cfg(not(feature = "dev"))]
static DEV_OPERATIONS: [Operation; 0] = [];

/// Every operation this build has, in order: development ones only with the `dev` feature.
pub fn operations() -> impl Iterator<Item = &'static Operation> {
    OPERATIONS.iter().chain(DEV_OPERATIONS.iter())
}

pub fn operation(name: &str) -> Option<&'static Operation> {
    operations().find(|o| o.name == name)
}

/// `{"tools": [...]}`, as MCP's `tools/list` returns them.
pub fn tools() -> Value {
    json!({"tools": operations().map(Operation::tool).collect::<Vec<_>>()})
}

/// Opens a session with the watch.
pub type Connect<'a> = dyn FnMut() -> Result<Box<dyn Mtp>, Error> + 'a;

/// Runs operation `name` with JSON `input`, opening the watch for this call only. Input is
/// validated before the watch is touched; output is validated before it is returned.
pub fn call(
    name: &str,
    input: Value,
    fonts: &FontSet,
    connect: &mut Connect<'_>,
) -> Result<Value, Error> {
    Session::one_shot(Cow::Borrowed(fonts), Box::new(connect)).call(name, input, &Control::NONE)
}

/// Operation `name`, or `invalid_input` naming it.
pub(crate) fn known(name: &str) -> Result<&'static Operation, Error> {
    operation(name).ok_or_else(|| {
        Error::new(
            ErrorCode::InvalidInput,
            format!("unknown operation {name:?}"),
        )
    })
}

/// The `parts` of install and restore_stock, checked against this build.
pub(crate) fn stage_parts(
    input: Value,
    fonts: &FontSet,
) -> Result<Option<BTreeSet<String>>, Error> {
    let StageInput { parts } = parse(input)?;
    if let Some(part) = parts.iter().flatten().find(|p| !fonts.has_part(p)) {
        return Err(Error::new(
            ErrorCode::FontUnavailable,
            format!("This build has no font part {part}."),
        ));
    }
    Ok(parts)
}

/// Checks `input` against `op`'s input schema.
pub(crate) fn validate_input(op: &Operation, input: &Value) -> Result<(), Error> {
    check(&op.input_schema(), input).map_err(|m| Error::new(ErrorCode::InvalidInput, m))
}

/// Checks `output` against `op`'s output schema.
pub(crate) fn validate_output(op: &Operation, output: &Value) -> Result<(), Error> {
    check(&op.output_schema(), output).map_err(|m| {
        Error::new(
            ErrorCode::Internal,
            format!("{} produced invalid output: {m}", op.name),
        )
    })
}

pub(crate) fn parse<T: for<'de> Deserialize<'de>>(input: Value) -> Result<T, Error> {
    serde_json::from_value(input).map_err(|e| Error::new(ErrorCode::InvalidInput, e.to_string()))
}

pub(crate) fn to_value<T: Serialize>(value: T) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(|e| Error::new(ErrorCode::Internal, e.to_string()))
}

fn check(schema: &Value, instance: &Value) -> Result<(), String> {
    let validator =
        jsonschema::validator_for(schema).map_err(|e| format!("invalid schema: {e}"))?;
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| {
            let message = match e.kind() {
                jsonschema::error::ValidationErrorKind::Enum { options } => {
                    format!("{} is not one of {options}", e.instance())
                }
                _ => e.to_string(),
            };
            let path = e.instance_path().to_string();
            if path.is_empty() {
                message
            } else {
                format!("{path}: {message}")
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub(crate) fn hex_handle(handle: u32) -> String {
    format!("{handle:#010x}")
}

/// MTP operations a watch must support for staging.
const WRITE_OPERATIONS: [u16; 4] = [
    op::GET_OBJECT,
    op::SEND_OBJECT_INFO,
    op::SEND_OBJECT,
    op::DELETE_OBJECT,
];

pub(crate) fn missing_write_operations(dev: &mut dyn Mtp) -> Result<Vec<&'static str>, Error> {
    let info = dev.device_info()?;
    Ok(WRITE_OPERATIONS
        .iter()
        .filter(|code| !info.operations_supported.contains(code))
        .map(|&code| ptp::operation_name(code))
        .collect())
}

pub(crate) fn read_garmin_device(
    dev: &mut dyn Mtp,
    layout: &Layout,
) -> Result<GarminDevice, Error> {
    let handle = layout.garmin_device_xml.ok_or_else(|| {
        Error::new(
            ErrorCode::GarminFolderNotFound,
            "The watch has no GARMIN/GarminDevice.xml.",
        )
    })?;
    garmin_device::parse(&dev.get_object(handle)?)
}

/// What the watch imports `font`'s file as: a bitmap part's `.tmp` becomes `<part>.fnt` at a
/// restart (the Venu 2 hides it; the watch in #38 lists it); a TrueType part keeps its `.bin`,
/// which only the hidden listing shows when the normal one doesn't.
fn imported_role(font: &Font) -> Role {
    match font.kind {
        Kind::Bitmap => Role::Imported,
        Kind::Truetype => Role::Bin,
    }
}

/// Whether `name` is one of `font`'s files, its own or its import, in either name form.
fn names_font(name: &str, font: &Font) -> bool {
    names::resolve(name).is_some_and(|f| {
        f.part == font.part && (f.role == font.target_role() || f.role == imported_role(font))
    })
}

/// `font`'s own files in the normal listing `files`, under either name form: what install and
/// restore replace, and what inspect reports.
fn staged_in<'a>(files: &'a [Object], font: &Font) -> Vec<&'a Object> {
    files
        .iter()
        .filter(|o| names::is(&o.info.filename, &font.part, font.target_role()))
        .collect()
}

/// A bitmap part's imports in the normal listing `files` (a TrueType part's import is its
/// own file, [`staged_in`]).
fn imported_in<'a>(files: &'a [Object], font: &Font) -> impl Iterator<Item = &'a Object> {
    let bitmap = font.kind == Kind::Bitmap;
    files
        .iter()
        .filter(move |o| bitmap && names::is(&o.info.filename, &font.part, Role::Imported))
}

pub(crate) fn extdata_path(name: &str) -> String {
    format!("GARMIN/ExtData/{name}")
}

/// Where install and restore write `font` on the watch `device` describes: the name it declares
/// ([`Font::file_name`]).
fn staged_path(font: &Font, device: &GarminDevice) -> String {
    extdata_path(&font.file_name(device))
}

/// Whether `part` of `pack` was verified on this watch.
fn part_tested(pack: &Pack, part: &str, device: &GarminDevice) -> TestStatus {
    pack.tested
        .iter()
        .filter(|t| {
            t.model_part.eq_ignore_ascii_case(&device.part_number)
                && t.parts.iter().any(|p| p == part)
        })
        .map(|t| {
            if t.software == device.software_version {
                TestStatus::Tested
            } else {
                TestStatus::TestedOtherSoftware
            }
        })
        .min()
        .unwrap_or(TestStatus::Untested)
}

/// Every listed pack with its listed parts and their tested status.
fn pack_reports(listed: &[Listed<'_>], device: &GarminDevice) -> Vec<PackReport> {
    listed
        .iter()
        .map(|l| PackReport {
            id: l.pack.id.clone(),
            name: l.pack.name.clone(),
            kind: l.pack.kind,
            weight: l.pack.weight,
            parts: l
                .fonts
                .iter()
                .map(|f| ListedPart {
                    part: f.part.clone(),
                    size_px: f.size_px,
                    tested: part_tested(l.pack, &f.part, device),
                })
                .collect(),
        })
        .collect()
}

/// Whether every listed part was verified on this watch: the worst part's status, `untested`
/// without parts, and the software versions this model was verified on with any of them.
fn test_report(listed: &[Listed<'_>], packs: &[PackReport], device: &GarminDevice) -> TestReport {
    let status = packs
        .iter()
        .flat_map(|p| &p.parts)
        .map(|p| p.tested)
        .max()
        .unwrap_or(TestStatus::Untested);
    let verified_software = listed
        .iter()
        .flat_map(|l| {
            l.pack.tested.iter().filter(|t| {
                t.model_part.eq_ignore_ascii_case(&device.part_number)
                    && l.fonts.iter().any(|f| t.parts.contains(&f.part))
            })
        })
        .map(|t| t.software.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    TestReport {
        status,
        verified_software,
    }
}

/// A watch-provided value as one line of text.
fn md(value: &str) -> String {
    let line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        "(none)".into()
    } else {
        line
    }
}

/// The shareable report of a watch whose fonts `reports` describes (one per font of `fonts`,
/// in order). Built only from allowlisted fields (`report`).
#[allow(clippy::too_many_arguments)]
fn report_of(
    device: &GarminDevice,
    fonts: &FontSet,
    hosts: &HostChoice,
    packs: &[PackReport],
    tested: TestStatus,
    support: &Support,
    state: GeorgianState,
    reports: &[FontReport],
    connection: Connection,
    extdata_listing: Option<Vec<ListedFile>>,
    extdata_hidden: HiddenListing,
) -> WatchReport {
    let parts = fonts
        .fonts()
        .zip(reports)
        .filter(|(_, r)| r.listed)
        .map(|((pack, font), r)| ReportedPart {
            part: r.part.clone(),
            target: r.target,
            file: r.file,
            tested: part_tested(pack, &font.part, device),
            mark: r.mark.clone(),
        })
        .collect();
    report::watch_report(
        device,
        ReportInput {
            connection,
            packs: packs.iter().map(|p| p.id.clone()).collect(),
            tested,
            support: support.clone(),
            state,
            hosts: ReportedHosts {
                kind: hosts.kind,
                reason: hosts.reason.clone(),
            },
            parts,
            test_marks: fonts.test_marks(),
            extdata_listing,
            extdata_hidden,
        },
    )
}

/// What the file named for `font` holds, by the one rule in [`FileState`], and whether it is
/// the imported one: `files` is the `ExtData` listing (`None` when it wasn't found or read),
/// `contents` the files read, and `hidden` the children the normal listing leaves out. Without
/// a file in the normal listing, the part's file is identified from the imported one: a
/// TrueType part's hidden `<part>.bin`, or the `<part>.fnt` the watch imported a bitmap part's
/// `.tmp` as, in either listing (the Venu 2 keeps every import hidden and read-only; the watch
/// in #38 lists it).
fn file_state(
    font: &Font,
    device: &GarminDevice,
    files: Option<&[Object]>,
    contents: &HashMap<u32, Vec<u8>>,
    hidden: &[HiddenFile],
) -> (FileState, bool) {
    let Some(files) = files else {
        return (FileState::Unknown, false);
    };
    let read = |o: &Object| match contents.get(&o.handle).map(|bytes| font.identify(bytes)) {
        Some(Content::Georgian) => FileState::Georgian,
        Some(Content::EarlierGeorgian) => FileState::GeorgianBeta,
        Some(Content::Stock) => FileState::Stock,
        Some(Content::Other) | None => FileState::Other,
    };
    let found = staged_in(files, font);
    let imported_state = || {
        let listed = imported_in(files, font).map(|o| Some(read(o)));
        let mut named = listed.chain(
            hidden
                .iter()
                .filter(|h| names::is(&h.name, &font.part, imported_role(font)))
                .map(|h| h.file),
        );
        match (named.next(), named.next()) {
            (Some(one), None) => one,
            (Some(_), Some(_)) => Some(FileState::Other),
            _ => None,
        }
    };
    match found.as_slice() {
        [] => match imported_state() {
            Some(state) => (state, true),
            None if font.kind == Kind::Truetype && device.declares_extdata_part(&font.part) => {
                (FileState::NotVisible, false)
            }
            None => (FileState::None, false),
        },
        [one] => (read(one), false),
        _ => (FileState::Other, false),
    }
}

/// `inspect`'s `hosts`: the choice `FontSet::hosts` made, in words.
fn host_choice(hosts: &Hosts<'_>) -> HostChoice {
    let skipped_parts: Vec<String> = hosts.skipped.iter().map(|f| f.part.clone()).collect();
    let restore_only_parts: Vec<String> =
        hosts.restore_only.iter().map(|f| f.part.clone()).collect();
    let mut reason = match hosts.kind {
        Some(Kind::Bitmap) if skipped_parts.is_empty() => {
            "This watch lists bitmap fonts this build adds Georgian to, so Georgian goes into \
             those."
                .to_owned()
        }
        Some(Kind::Bitmap) => format!(
            "This watch lists bitmap fonts this build adds Georgian to, so Georgian goes into \
             those, as before TrueType support; its TrueType fonts {} stay Garmin's. TrueType \
             fonts get Georgian only on watches that list none of this build's bitmap fonts.",
            and_list(&skipped_parts)
        ),
        Some(Kind::Truetype) => "This watch lists none of this build's bitmap fonts, so Georgian \
                                 goes into the TrueType fonts it lists."
            .to_owned(),
        None => "This watch lists none of the fonts this build adds Georgian to.".to_owned(),
    };
    if !restore_only_parts.is_empty() {
        reason.push_str(&format!(
            " Its fonts {} didn't show Georgian on a watch, so this build only restores \
             Garmin's originals there.",
            and_list(&restore_only_parts)
        ));
    }
    HostChoice {
        kind: hosts.kind,
        reason,
        skipped_parts,
        restore_only_parts,
    }
}

/// Whether `font` is among `hosts`' parts.
fn chosen(hosts: &Hosts<'_>, font: &Font) -> bool {
    hosts
        .listed
        .iter()
        .any(|l| l.fonts.iter().any(|f| f.part == font.part))
}

/// A failure's message, short enough for the report's one line.
fn short(message: impl std::fmt::Display) -> String {
    let text = message.to_string();
    match text.char_indices().nth(80) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
    }
}

/// `ExtData`'s children the normal listing leaves out, from Garmin's vendor `GetObjectIds`: on
/// the Venu 2 exactly the read-only objects, such as imported fonts. Install and restore read
/// the same listing ([`hidden_objects`]) for a part's file the normal one doesn't show. Only a
/// file named for a part `restore` covers (every part install or restore writes on this watch;
/// a bitmap part also as the `<part>.fnt` the watch imports its `.tmp` as), with a size this
/// build knows, is read; any failure becomes a note and never fails `inspect`.
fn hidden_listing(
    dev: &mut dyn Mtp,
    fonts: &FontSet,
    storage: u32,
    ext: Option<&ExtData>,
    restore: &Hosts<'_>,
) -> HiddenListing {
    let failed = |note: String| HiddenListing {
        status: HiddenStatus::Error,
        files: Vec::new(),
        note: Some(note),
    };
    let Some(ext) = ext else {
        return HiddenListing::not_read();
    };
    // ExtData's own files only: never another folder's names.
    let (objects, mut note) = match scan_hidden(dev, storage, ext) {
        HiddenScan::Unsupported => {
            return HiddenListing {
                status: HiddenStatus::Unsupported,
                files: Vec::new(),
                note: None,
            };
        }
        HiddenScan::Failed(note) => return failed(note),
        HiddenScan::Listed { objects, note } => (objects, note),
    };
    let mut files = Vec::new();
    for (handle, info) in objects {
        let size = info.compressed_size as usize;
        let font = fonts
            .fonts()
            .map(|(_, f)| f)
            .find(|f| chosen(restore, f) && names_font(&info.filename, f));
        let file = match font {
            None => None,
            Some(font) if !font.known_size(size) => Some(FileState::Other),
            Some(font) => match dev.get_object(handle) {
                Ok(bytes) => Some(match font.identify(&bytes) {
                    Content::Georgian => FileState::Georgian,
                    Content::EarlierGeorgian => FileState::GeorgianBeta,
                    Content::Stock => FileState::Stock,
                    Content::Other => FileState::Other,
                }),
                Err(e) => {
                    note.get_or_insert_with(|| short(format!("{}: {e}", info.filename)));
                    None
                }
            },
        };
        let read_only = dev.read_only(handle, &info);
        files.push(HiddenFile {
            name: info.filename,
            bytes: info.compressed_size.into(),
            read_only,
            file,
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    HiddenListing {
        status: if note.is_some() {
            HiddenStatus::Error
        } else {
            HiddenStatus::Listed
        },
        files,
        note,
    }
}

pub fn inspect(dev: &mut dyn Mtp, fonts: &FontSet) -> Result<InspectOutput, Error> {
    let writable = missing_write_operations(dev)?.is_empty();
    let layout = extdata::discover(dev)?;
    let device = read_garmin_device(dev, &layout)?;
    let hosts = fonts.hosts(&device);
    let restore = fonts.restore_hosts(&device);
    // Read only the files that may be this build's: named for one of its parts, with the size
    // of that part's stock, Georgian or earlier Georgian build. Everything else (multi-megabyte
    // Chinese fonts on TrueType watches) is listed without being transferred; the app polls
    // inspect.
    let mut contents: HashMap<u32, Vec<u8>> = HashMap::new();
    for o in layout.extdata.iter().flat_map(|ext| &ext.files) {
        let size = o.info.compressed_size as usize;
        let ours = fonts
            .fonts()
            .any(|(_, f)| names_font(&o.info.filename, f) && f.known_size(size));
        if ours {
            contents.insert(o.handle, dev.get_object(o.handle)?);
        }
    }
    let files = layout.extdata.as_ref().map(|ext| ext.files.as_slice());
    let extdata_hidden = hidden_listing(
        dev,
        fonts,
        layout.storage,
        layout.extdata.as_ref(),
        &restore,
    );
    let mut reports = Vec::new();
    for (pack, font) in fonts.fonts() {
        let (file, imported) = file_state(font, &device, files, &contents, &extdata_hidden.files);
        reports.push(FontReport {
            pack: pack.id.clone(),
            part: font.part.clone(),
            kind: font.kind,
            size_px: font.size_px,
            listed: device.declares_extdata_part(&font.part),
            target: writable && chosen(&hosts, font),
            restore: writable && chosen(&restore, font),
            file,
            imported,
            mark: font.mark.as_ref().map(|m| m.variant.clone()),
        });
    }
    let extdata = layout.extdata.map(|ext| ExtDataReport {
        handle: hex_handle(ext.handle),
        storage_id: hex_handle(layout.storage),
        listed_under_garmin: ext.listed_under_garmin,
        found_by: ext.found_by,
        files: ext
            .files
            .into_iter()
            .map(|o| StagedFile {
                sha256: contents.get(&o.handle).map(|bytes| sha256_hex(bytes)),
                name: o.info.filename,
                handle: hex_handle(o.handle),
                bytes: o.info.compressed_size,
            })
            .collect(),
    });
    let packs = pack_reports(&hosts.listed, &device);
    let tested = test_report(&hosts.listed, &packs, &device);
    let targets: Vec<TargetFile> = reports
        .iter()
        .filter(|r| r.target)
        .map(|r| TargetFile {
            file: r.file,
            imported: r.imported,
        })
        .collect();
    let state = georgian_state(hosts.kind, &targets);
    let restorable = writable && !restore.listed.is_empty();
    let mut support = support::support(fonts, &device, &hosts, tested.status);
    // A font this build doesn't recognise under a target's name is news, whatever the status.
    if targets.iter().any(|t| t.file == FileState::Other) {
        support.report_wanted = true;
    }
    let hosts = host_choice(&hosts);
    let listing = extdata.as_ref().map(|ext| {
        ext.files
            .iter()
            .map(|f| ListedFile {
                name: f.name.clone(),
                bytes: f.bytes.into(),
                sha256: f.sha256.clone(),
            })
            .collect()
    });
    let report = report_of(
        &device,
        fonts,
        &hosts,
        &packs,
        tested.status,
        &support,
        state,
        &reports,
        Connection::Mtp,
        listing,
        extdata_hidden.clone(),
    );
    Ok(InspectOutput {
        supported: writable && !packs.is_empty(),
        restorable,
        state,
        tested,
        support,
        hosts,
        packs,
        fonts: reports,
        extdata,
        extdata_hidden,
        report,
        device: DeviceReport {
            model: device.description,
            part_number: device.part_number,
            software_version: device.software_version,
        },
    })
}

/// Reads a watch that connects as a USB drive: the volume at `path`, or the one mounted.
pub fn inspect_drive(
    path: Option<&std::path::Path>,
    fonts: &FontSet,
) -> Result<InspectDriveOutput, Error> {
    let root = drive::locate(path)?;
    let (device, listing) = drive::read(&root)?;
    let hosts = fonts.hosts(&device);
    let reports: Vec<FontReport> = fonts
        .fonts()
        .map(|(pack, font)| FontReport {
            pack: pack.id.clone(),
            part: font.part.clone(),
            kind: font.kind,
            size_px: font.size_px,
            listed: device.declares_extdata_part(&font.part),
            target: false,
            restore: false,
            file: FileState::Unknown,
            imported: false,
            mark: font.mark.as_ref().map(|m| m.variant.clone()),
        })
        .collect();
    let packs = pack_reports(&hosts.listed, &device);
    let tested = test_report(&hosts.listed, &packs, &device);
    let support = support::support(fonts, &device, &hosts, tested.status);
    let hosts = host_choice(&hosts);
    let report = report_of(
        &device,
        fonts,
        &hosts,
        &packs,
        tested.status,
        &support,
        GeorgianState::Unknown,
        &reports,
        Connection::UsbDrive,
        listing,
        HiddenListing::not_read(),
    );
    Ok(InspectDriveOutput {
        path: root.display().to_string(),
        device: DeviceReport {
            model: device.description,
            part_number: device.part_number,
            software_version: device.software_version,
        },
        hosts,
        support,
        report,
    })
}

/// Deletes the object this run created after a failed transfer and says so in `cause` and its
/// rollback.
pub(crate) fn discard(dev: &mut dyn Mtp, handle: u32, path: &str, cause: Error) -> Error {
    let (done, result, message) = match dev.delete_object(handle) {
        Ok(()) => (
            true,
            format!("{path} ({}) deleted", hex_handle(handle)),
            format!(
                "{}. A new object was created as {path} ({}) and has been deleted.",
                cause.message,
                hex_handle(handle)
            ),
        ),
        Err(e) => (
            false,
            format!(
                "{path} ({}) could not be deleted ({e}); it may hold incomplete data",
                hex_handle(handle)
            ),
            format!(
                "{}. A new object was created as {path} ({}) and could not be deleted ({e}); \
                 it may hold incomplete data.",
                cause.message,
                hex_handle(handle)
            ),
        ),
    };
    Error { message, ..cause }.with_rollback([RollbackFile {
        path: path.to_owned(),
        action: RollbackAction::Delete,
        done,
        result,
    }])
}

/// The objects in `parent` named `name` (case-insensitively), from a fresh listing.
pub(crate) fn named_objects(
    dev: &mut dyn Mtp,
    storage: u32,
    parent: u32,
    name: &str,
) -> Result<Vec<u32>, MtpError> {
    let mut found = Vec::new();
    for handle in dev.object_handles(storage, parent)? {
        if dev.object_info(handle)?.filename.eq_ignore_ascii_case(name) {
            found.push(handle);
        }
    }
    Ok(found)
}

/// After creating an object failed unconfirmed: the watch may still have created it. ExtData
/// held nothing under `name` before (callers check, or delete the old file first), so an
/// object there now is this run's and is deleted.
fn discard_unconfirmed(
    dev: &mut dyn Mtp,
    storage: u32,
    parent: u32,
    name: &str,
    path: &str,
    cause: Error,
) -> Error {
    let created = match named_objects(dev, storage, parent, name) {
        Ok(created) => created,
        Err(e) => {
            let message = format!(
                "{}. Whether the watch created {path} is unknown: listing ExtData failed ({e}).",
                cause.message
            );
            return Error { message, ..cause }.with_rollback([RollbackFile {
                path: path.to_owned(),
                action: RollbackAction::Delete,
                done: false,
                result: format!(
                    "whether the watch created {path} is unknown (listing ExtData failed: {e})"
                ),
            }]);
        }
    };
    if created.is_empty() {
        let message = format!("{}. No object was created.", cause.message);
        return Error { message, ..cause };
    }
    created
        .into_iter()
        .fold(cause, |cause, handle| discard(dev, handle, path, cause))
}

/// Writes `bytes` as `name` in `ExtData`, which holds no file under that name, and reads it
/// back. On failure the new object is deleted and the error says how that went.
pub(crate) fn write_verified(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    name: &str,
    bytes: &[u8],
) -> Result<u32, Error> {
    let name = name.to_owned();
    let path = extdata_path(&name);
    let size = u32::try_from(bytes.len()).map_err(|_| {
        Error::new(
            ErrorCode::Internal,
            "font exceeds the MTP object size limit",
        )
    })?;
    let info = ObjectInfo {
        storage_id: storage,
        format: ptp::format::UNDEFINED,
        compressed_size: size,
        parent: ext.handle,
        filename: name.clone(),
        ..ObjectInfo::default()
    };
    let new = match dev.create_object(storage, ext.handle, &info, bytes) {
        Ok(new) => new,
        Err(CreateError::Unconfirmed(e)) => {
            let cause = Error::from(e);
            return Err(discard_unconfirmed(
                dev, storage, ext.handle, &name, &path, cause,
            ));
        }
        Err(CreateError::Misplaced(new)) => {
            let cause = Error::new(
                ErrorCode::DeviceError,
                format!(
                    "The watch placed {name} outside GARMIN/ExtData (parent {})",
                    hex_handle(new.parent)
                ),
            );
            return Err(discard(dev, new.handle, &name, cause));
        }
        Err(CreateError::Data { handle, error }) => {
            return Err(discard(dev, handle, &path, error.into()));
        }
    };
    match dev.get_object(new.handle) {
        Err(e) => Err(discard(dev, new.handle, &path, e.into())),
        Ok(readback) if readback != bytes => {
            let cause = Error::new(
                ErrorCode::VerificationFailed,
                format!(
                    "{path} read back as {} bytes that differ from the {} sent",
                    readback.len(),
                    bytes.len()
                ),
            );
            Err(discard(dev, new.handle, &path, cause))
        }
        Ok(_) => Ok(new.handle),
    }
}

/// A change a failed call undoes.
pub(crate) enum Undo {
    /// A file this call created (a bitmap part's `.tmp`, or a `.bin` where the normal listing
    /// showed none): deleted.
    Created { handle: u32, path: String },
    /// The file `name` this call set out to replace: `original`, read from the watch before
    /// anything was written, is put back.
    Replaced { name: String, original: Vec<u8> },
}

/// Puts the file `name` back as it was before this call: `original`. A single file already
/// holding it is left alone; anything else under the name is deleted first, and `original` is
/// written and read back. Returns what happened, for the error.
fn restore_original(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    name: &str,
    original: &[u8],
) -> RollbackFile {
    let path = extdata_path(name);
    let step = |done: bool, result: String| RollbackFile {
        path: path.clone(),
        action: RollbackAction::Restore,
        done,
        result,
    };
    let current = match named_objects(dev, storage, ext.handle, name) {
        Ok(current) => current,
        Err(e) => {
            return step(
                false,
                format!(
                    "whether {path} holds its original is unknown (listing ExtData failed: {e})"
                ),
            );
        }
    };
    if let [one] = current.as_slice()
        && dev.get_object(*one).is_ok_and(|bytes| bytes == original)
    {
        return step(true, format!("{path} still holds the original"));
    }
    for handle in current {
        if let Err(e) = dev.delete_object(handle) {
            return step(
                false,
                format!(
                    "{path} ({}) could not be deleted ({e}), so its original was not written back",
                    hex_handle(handle)
                ),
            );
        }
    }
    match write_verified(dev, storage, ext, name, original) {
        Ok(handle) => step(
            true,
            format!(
                "{path} written back from the original and verified ({})",
                hex_handle(handle)
            ),
        ),
        Err(e) => step(
            false,
            format!("{path} could not be written back: {}", e.message),
        ),
    }
}

/// What to do after install or restore couldn't undo all of its changes.
pub(crate) const RESTORE_HINT: &str = "Connect the watch and run Restore original font \
    (georgian-for-garmin restore-stock), then restart the watch; if that reports a conflict, \
    share the watch report in an issue.";

/// After a part failed: tries to undo this run's changes, in order, and adds each result to
/// `cause`, in words and in its `rollback`. Files staged by earlier runs were never changed. When
/// a step (or the cleanup of a failed transfer, already in `cause`) failed, the message says the
/// rollback was incomplete, names what may be left, and ends with `recovery`.
pub(crate) fn roll_back(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    undo: &[Undo],
    cause: Error,
    recovery: &str,
) -> Error {
    let (mut created, mut replaced) = (false, false);
    let mut files = Vec::with_capacity(undo.len());
    for step in undo {
        files.push(match step {
            Undo::Created { handle, path } => {
                created = true;
                let (done, result) = match dev.delete_object(*handle) {
                    Ok(()) => (true, format!("{path} ({}) deleted", hex_handle(*handle))),
                    Err(e) => (
                        false,
                        format!(
                            "{path} ({}) could not be deleted ({e})",
                            hex_handle(*handle)
                        ),
                    ),
                };
                RollbackFile {
                    path: path.clone(),
                    action: RollbackAction::Delete,
                    done,
                    result,
                }
            }
            Undo::Replaced { name, original } => {
                replaced = true;
                restore_original(dev, storage, ext, name, original)
            }
        });
    }
    let results: Vec<String> = files.iter().map(|f| f.result.clone()).collect();
    let error = cause.with_rollback(files);
    let Some(rollback) = error.rollback.as_deref() else {
        return error;
    };
    let message = if rollback.status == RollbackStatus::Complete {
        if results.is_empty() {
            return error;
        }
        let what = if replaced {
            "Rolled back this run's changes"
        } else {
            "Rolled back the files this run had staged"
        };
        let tail = if created {
            " Files staged before this run were left in place."
        } else {
            ""
        };
        format!("{} {what}: {}.{tail}", error.message, results.join("; "))
    } else {
        let left: Vec<String> = rollback
            .files
            .iter()
            .filter(|f| !f.done)
            .map(|f| f.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let tried = if results.is_empty() {
            String::new()
        } else {
            format!(" Tried to undo this run's changes: {}.", results.join("; "))
        };
        format!(
            "{}{tried} The rollback was incomplete: {} may not be as before this run. \
             {recovery}",
            error.message,
            and_list(&left)
        )
    };
    Error { message, ..error }
}

/// What a target needs, decided before anything is written.
enum Plan {
    /// The identical file is already there under this build's name, `name` (in the normal
    /// listing or, imported, hidden).
    Already(u32, String),
    /// Write a new file: a bitmap part's `<part>.tmp`, or a TrueType part's `<part>.bin` where
    /// the normal listing shows none. A hidden read-only object under that name (an import) is
    /// never deleted; the watch imports the new file over it, as on the Forerunner 165 (#5,
    /// #19).
    Create,
    /// Delete `existing`, the part's file `name` in the normal listing (either name form),
    /// identified as Garmin's original or one of this program's builds, and write the new one
    /// under this build's name; `original` is its bytes, for rollback.
    Replace {
        existing: u32,
        name: String,
        original: Vec<u8>,
    },
}

/// Why a target can't be written; nothing is.
enum Refusal {
    /// The part's file is neither Garmin's original nor one of this program's builds.
    Other(String),
    /// Several files are the part's, under either name form: which, in words.
    Several(String),
    /// The file can't be read and the watch declares another version or file name than this
    /// operation accepts: perhaps a newer Garmin font.
    Version(String),
    /// Install: the file can't be read and isn't declared at Garmin's version, so it may be one
    /// of this program's trailer-less builds or something else; restore first.
    RestoreFirst(String),
}

/// "GARMIN/ExtData/a and GARMIN/ExtData/b are both waiting files of part p", for several files
/// of one part.
fn several(names: &[&str], what: &str, part: &str) -> Refusal {
    let paths: Vec<String> = names.iter().map(|n| extdata_path(n)).collect();
    let both = if paths.len() == 2 { "both" } else { "all" };
    Refusal::Several(format!(
        "{} are {both} {what} of part {part}",
        and_list(&paths)
    ))
}

/// `ExtData`'s children the normal listing leaves out, from Garmin's `GetObjectIds` (0x9002).
pub(crate) enum HiddenScan {
    /// The watch doesn't list the operation.
    Unsupported,
    /// Asking failed, with why.
    Failed(String),
    /// The children with their info; `note` says why some info couldn't be read (those stay
    /// unseen, like objects the watch doesn't list).
    Listed {
        objects: Vec<(u32, ObjectInfo)>,
        note: Option<String>,
    },
}

/// Reads `ExtData`'s hidden children: on the Venu 2 exactly the read-only imports. Folders and
/// objects outside `ExtData` are left out.
pub(crate) fn scan_hidden(dev: &mut dyn Mtp, storage: u32, ext: &ExtData) -> HiddenScan {
    match dev.device_info() {
        Ok(info)
            if info
                .operations_supported
                .contains(&op::GARMIN_GET_OBJECT_IDS) => {}
        Ok(_) => return HiddenScan::Unsupported,
        Err(e) => return HiddenScan::Failed(short(e)),
    }
    let handles = match dev.garmin_object_ids(storage, ext.handle) {
        Ok(handles) => handles,
        Err(e) => return HiddenScan::Failed(short(e)),
    };
    let listed: BTreeSet<u32> = ext.files.iter().map(|o| o.handle).collect();
    let mut objects = Vec::new();
    let mut note = None;
    for handle in handles.into_iter().collect::<BTreeSet<_>>() {
        if listed.contains(&handle) {
            continue;
        }
        match dev.object_info(handle) {
            Ok(info)
                if info.parent == ext.handle && info.storage_id == storage && !info.is_folder() =>
            {
                objects.push((handle, info));
            }
            Ok(_) => {}
            Err(e) => {
                note.get_or_insert_with(|| short(format!("{}: {e}", hex_handle(handle))));
            }
        }
    }
    HiddenScan::Listed { objects, note }
}

/// `ExtData`'s children the normal listing leaves out, with their info, when the watch lists
/// Garmin's `GetObjectIds` (0x9002); `None` when it doesn't, or listing fails. On the Venu 2
/// these are exactly the read-only imports.
fn hidden_objects(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
) -> Option<Vec<(u32, ObjectInfo)>> {
    match scan_hidden(dev, storage, ext) {
        HiddenScan::Listed { objects, .. } => Some(objects),
        HiddenScan::Unsupported | HiddenScan::Failed(_) => None,
    }
}

/// Decides `font`'s Plan from what its file holds, by content: the part's file in the normal
/// listing, else its import (a bitmap part's `.fnt`, in either listing: the Venu 2 hides it,
/// other watches list it, as in #38; a TrueType part's hidden `.bin`, from `hidden` when the
/// watch lists 0x9002), each under either name form ([`names`]). Garmin's original or one of
/// this program's builds is replaced (an imported one by writing a new file, which the watch
/// imports over it at the next restart; one under the other name form by deleting it and
/// writing the name the watch declares, [`Font::file_name`], so a part never has two waiting
/// files); anything else is refused, imported or not, since a new file would replace it one
/// restart later, and so are several files for one part. Only a file that can't be read (not
/// shown, or a hidden import whose read fails) falls back to what the watch declares, for a
/// TrueType part: install accepts only Garmin's version, restore also a version below 1.00
/// ([`below_one`]), since it only ever puts Garmin's own file back. A bitmap part with no file
/// to read is simply staged. A failed read of a file in the normal listing stops the call:
/// replacing it would delete a file that couldn't be kept for rollback, and an import there is
/// never left unidentified.
fn plan(
    dev: &mut dyn Mtp,
    ext: &ExtData,
    hidden: Option<&[(u32, ObjectInfo)]>,
    device: &GarminDevice,
    font: &Font,
    variant: Variant,
) -> Result<Result<Plan, Refusal>, Error> {
    let target = variant.bytes(font);
    let own_name = font.file_name(device);
    match staged_in(&ext.files, font).as_slice() {
        [one] => {
            let name = &one.info.filename;
            let path = extdata_path(name);
            if !font.known_size(one.info.compressed_size as usize) {
                return Ok(Err(Refusal::Other(path)));
            }
            let current = dev.get_object(one.handle)?;
            return Ok(
                if current == target && name.eq_ignore_ascii_case(&own_name) {
                    Ok(Plan::Already(one.handle, name.clone()))
                } else if font.identify(&current) == Content::Other {
                    Err(Refusal::Other(path))
                } else {
                    Ok(Plan::Replace {
                        existing: one.handle,
                        name: name.clone(),
                        original: current,
                    })
                },
            );
        }
        [] => {}
        many => {
            let names: Vec<&str> = many.iter().map(|o| o.info.filename.as_str()).collect();
            let what = match font.kind {
                Kind::Bitmap => "waiting files",
                Kind::Truetype => "files",
            };
            return Ok(Err(several(&names, what, &font.part)));
        }
    }
    // (handle, info, hidden). A TrueType part's import is its own file, handled above.
    let named: Vec<(u32, &ObjectInfo, bool)> = imported_in(&ext.files, font)
        .map(|o| (o.handle, &o.info, false))
        .chain(
            hidden
                .into_iter()
                .flatten()
                .filter(|(_, info)| names::is(&info.filename, &font.part, imported_role(font)))
                .map(|(handle, info)| (*handle, info, true)),
        )
        .collect();
    match named.as_slice() {
        [(handle, info, is_hidden)] => {
            let imported_path = extdata_path(&info.filename);
            if !font.known_size(info.compressed_size as usize) {
                return Ok(Err(Refusal::Other(imported_path)));
            }
            // A failed read of a hidden import leaves it unidentified: the fallback below
            // decides. A bitmap part identical to its import is staged again as before; the
            // watch imports the same bytes.
            let current = match dev.get_object(*handle) {
                Ok(current) => Some(current),
                Err(e) if !is_hidden => return Err(e.into()),
                Err(_) => None,
            };
            if let Some(current) = current {
                return Ok(if current == target && font.kind == Kind::Truetype {
                    Ok(Plan::Already(*handle, info.filename.clone()))
                } else if font.identify(&current) == Content::Other {
                    Err(Refusal::Other(imported_path))
                } else {
                    Ok(Plan::Create)
                });
            }
        }
        [] => {}
        many => {
            let names: Vec<&str> = many.iter().map(|(_, i, _)| i.filename.as_str()).collect();
            return Ok(Err(several(&names, "imports", &font.part)));
        }
    }
    if font.kind == Kind::Bitmap {
        return Ok(Ok(Plan::Create));
    }
    // A TrueType file that can't be read: what the watch declares.
    let Some(declared) = device.extdata_files().find(|d| d.part_number == font.part) else {
        return Ok(Ok(Plan::Create));
    };
    let version = declared.version.as_deref().unwrap_or("unknown");
    let garmins = version == font.version;
    // The name this build writes must be the one the watch declares: it loads that file.
    let named_right = declared.file_name.eq_ignore_ascii_case(&own_name);
    let described = format!(
        "{} as version {} ({}), where this build has version {} ({})",
        font.part,
        md(version),
        md(&declared.file_name),
        font.version,
        own_name
    );
    Ok(match variant {
        _ if !named_right => Err(Refusal::Version(described)),
        _ if garmins => Ok(Plan::Create),
        Variant::Stock if below_one(version) => Ok(Plan::Create),
        Variant::Georgian if below_one(version) => Err(Refusal::RestoreFirst(format!(
            "{} (declared as version {})",
            staged_path(font, device),
            md(version)
        ))),
        _ => Err(Refusal::Version(described)),
    })
}

/// "a, b and c" from a list of names.
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Stages `variant` of every part of this build's packs that the watch lists, across all packs,
/// or of `parts`: a bitmap part as a new `<part>.tmp`, a TrueType part over `<part>.bin`.
/// `control` gets one progress step per part and can cancel between files: cancelled before
/// the first write, nothing is written; after it, this call's changes are rolled back as far as
/// possible, and the error's `rollback` says whether every step worked.
pub fn stage(
    dev: &mut dyn Mtp,
    fonts: &FontSet,
    variant: Variant,
    parts: Option<&BTreeSet<String>>,
    control: &Control<'_>,
) -> Result<StageOutput, Error> {
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
    let watch = format!("{} ({})", device.description, device.part_number);
    // Restore also covers restore-only packs, so an earlier release's Georgian can go back.
    let hosts = match variant {
        Variant::Georgian => fonts.hosts(&device),
        Variant::Stock => fonts.restore_hosts(&device),
    };
    if let Some(part) = parts
        .into_iter()
        .flatten()
        .find(|&p| hosts.restore_only.iter().any(|f| &f.part == p))
    {
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "This build only restores part {part}: Georgian in it didn't show on a watch. \
                 Nothing was written."
            ),
        ));
    }
    let listed = hosts.listed;
    if listed.is_empty() {
        let message = if hosts.restore_only.is_empty() {
            format!(
                "{watch} lists none of this build's font packs under {}; nothing was written.",
                garmin_device::EXTDATA_PATH
            )
        } else {
            let parts: Vec<String> = hosts.restore_only.iter().map(|f| f.part.clone()).collect();
            format!(
                "{watch} lists only fonts this build no longer adds Georgian to ({}), because \
                 Georgian in them didn't show on a watch; nothing was written.",
                and_list(&parts)
            )
        };
        return Err(Error::new(ErrorCode::UnsupportedDevice, message));
    }
    let mut targets: Vec<(&Pack, &Font)> = listed
        .iter()
        .flat_map(|l| l.fonts.iter().map(move |&f| (l.pack, f)))
        .collect();
    if let Some(parts) = parts {
        if let Some(part) = parts
            .iter()
            .find(|&p| hosts.skipped.iter().any(|f| &f.part == p))
        {
            return Err(Error::new(
                ErrorCode::UnsupportedDevice,
                format!(
                    "{watch} gets Georgian in its bitmap fonts, so this build leaves its TrueType \
                     part {part} alone. Nothing was written."
                ),
            ));
        }
        if let Some(part) = parts
            .iter()
            .find(|&p| !targets.iter().any(|(_, f)| &f.part == p))
        {
            let names: Vec<&str> = targets.iter().map(|(_, f)| f.part.as_str()).collect();
            return Err(Error::new(
                ErrorCode::UnsupportedDevice,
                format!(
                    "{watch} doesn't list part {part}; of this build's parts it lists {}. \
                     Nothing was written.",
                    names.join(", ")
                ),
            ));
        }
        targets.retain(|(_, f)| parts.contains(&f.part));
    }

    let ext = layout.extdata.as_ref().ok_or_else(|| {
        Error::new(
            ErrorCode::ExtdataNotFound,
            "Could not find the watch's GARMIN/ExtData folder; nothing was written.",
        )
    })?;

    // Check every target before writing anything. Files are identified by content; the hidden
    // listing is read only when a target isn't in the normal one.
    let needs_hidden = targets
        .iter()
        .any(|(_, font)| staged_in(&ext.files, font).is_empty());
    let hidden = if needs_hidden {
        hidden_objects(dev, layout.storage, ext)
    } else {
        None
    };
    let mut plans = Vec::with_capacity(targets.len());
    let (mut others, mut versions, mut restore_first) = (Vec::new(), Vec::new(), Vec::new());
    let mut several = Vec::new();
    for (_, font) in &targets {
        control.check_nothing_written()?;
        match plan(dev, ext, hidden.as_deref(), &device, font, variant)? {
            Ok(plan) => plans.push(plan),
            Err(Refusal::Other(path)) => others.push(path),
            Err(Refusal::Several(described)) => several.push(described),
            Err(Refusal::Version(described)) => versions.push(described),
            Err(Refusal::RestoreFirst(path)) => restore_first.push(path),
        }
    }
    if !versions.is_empty() {
        // Another version may be a newer Garmin font.
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "{watch} lists {}. Nothing was written.",
                and_list(&versions)
            ),
        ));
    }
    if !others.is_empty() || !several.is_empty() {
        let mut sentences = Vec::new();
        if !others.is_empty() {
            sentences.push(format!(
                "{} {} neither Garmin's original font nor a Georgian build of this program; this \
                 build replaces only those",
                and_list(&others),
                if others.len() == 1 { "holds" } else { "hold" }
            ));
        }
        if !several.is_empty() {
            sentences.push(format!("{}: a part takes one file", several.join("; ")));
        }
        let all = others.iter().chain(&several);
        let waiting = if all.clone().any(|p| p.contains(".tmp")) {
            " A bitmap part's .tmp may be a file waiting for the watch to restart: unplug the \
             watch and restart it, then try again."
        } else {
            ""
        };
        let imported = if all.clone().any(|p| p.contains(".fnt")) {
            " A .fnt is the font the watch imported and uses for that part: a new .tmp would \
             replace it at the next restart."
        } else {
            ""
        };
        return Err(Error::new(
            ErrorCode::Conflict,
            format!(
                "{}, so nothing was written.{waiting}{imported}",
                sentences.join("; ")
            ),
        ));
    }
    if !restore_first.is_empty() {
        return Err(Error::new(
            ErrorCode::RestoreFirst,
            format!(
                "The watch doesn't let {} be read, and declares a version other than Garmin's, \
                 as it does for this program's builds before 0.3.0; install never writes over a \
                 file it can't identify. Restore the original font first, restart the watch, \
                 then install again. Nothing was written.",
                and_list(&restore_first)
            ),
        ));
    }

    let storage = layout.storage;
    let total = targets.len();
    control.progress(0, total, &format!("Checked {total} files; writing"));
    let mut undo: Vec<Undo> = Vec::new();
    let mut packs: Vec<StagedPack> = Vec::new();
    for (done, ((pack, font), plan)) in targets.into_iter().zip(plans).enumerate() {
        if let Err(cause) = control.check_nothing_written() {
            let cause = if undo.is_empty() {
                cause
            } else {
                Error::new(ErrorCode::Cancelled, "Cancelled.")
            };
            return Err(roll_back(dev, storage, ext, &undo, cause, RESTORE_HINT));
        }
        let bytes = variant.bytes(font);
        let own_name = font.file_name(&device);
        let own_path = extdata_path(&own_name);
        let (status, handle, path) = match plan {
            Plan::Already(handle, name) => {
                (StageStatus::AlreadyStaged, handle, extdata_path(&name))
            }
            Plan::Create => match write_verified(dev, storage, ext, &own_name, bytes) {
                Ok(handle) => {
                    undo.push(Undo::Created {
                        handle,
                        path: own_path.clone(),
                    });
                    (StageStatus::StagedVerified, handle, own_path)
                }
                Err(e) => return Err(roll_back(dev, storage, ext, &undo, e, RESTORE_HINT)),
            },
            Plan::Replace {
                existing,
                name,
                original,
            } => {
                let existing_path = extdata_path(&name);
                let renamed = !name.eq_ignore_ascii_case(&own_name);
                undo.push(Undo::Replaced { name, original });
                let deleted = dev.delete_object(existing).map_err(|e| {
                    let e = Error::from(e);
                    Error::new(
                        e.code,
                        format!(
                            "Could not delete {existing_path} ({}) to replace it: {}.",
                            hex_handle(existing),
                            e.message
                        ),
                    )
                });
                match deleted.and_then(|()| write_verified(dev, storage, ext, &own_name, bytes)) {
                    Ok(handle) => {
                        // Written under the declared name beside the other name form: rollback
                        // puts the old one back and deletes this.
                        if renamed {
                            undo.push(Undo::Created {
                                handle,
                                path: own_path.clone(),
                            });
                        }
                        (StageStatus::StagedVerified, handle, own_path)
                    }
                    Err(e) => return Err(roll_back(dev, storage, ext, &undo, e, RESTORE_HINT)),
                }
            }
        };
        let word = match status {
            StageStatus::AlreadyStaged => "already staged",
            StageStatus::StagedVerified => "staged and verified",
        };
        control.progress(done + 1, total, &format!("{path}: {word}"));
        if packs.last().is_none_or(|p| p.id != pack.id) {
            packs.push(StagedPack {
                id: pack.id.clone(),
                name: pack.name.clone(),
                kind: pack.kind,
                weight: pack.weight,
                parts: Vec::new(),
            });
        }
        packs
            .last_mut()
            .expect("pushed above")
            .parts
            .push(StagedPart {
                part: font.part.clone(),
                size_px: font.size_px,
                status,
                path,
                handle: hex_handle(handle),
                bytes: bytes.len() as u64,
                sha256: sha256_hex(bytes),
            });
    }
    let status = if undo.is_empty() {
        StageStatus::AlreadyStaged
    } else {
        StageStatus::StagedVerified
    };
    Ok(StageOutput {
        status,
        variant,
        packs,
        next_step:
            "Disconnect the USB cable, then restart the watch; it loads the fonts on restart."
                .into(),
    })
}
