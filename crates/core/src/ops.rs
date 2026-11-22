//! Operations: one declaration each (JSON Schema in and out, behavior hints), validated on
//! every call. CLI, app and any future adapter call [`call`].
//!
//! A watch gets Georgian in every part of this build's packs that it lists under
//! `Garmin/ExtData`, across all packs: `install`/`restore_stock` write each of them. A bitmap
//! part is staged as a new `<part>.tmp`; a TrueType part replaces the watch's `<part>.bin`.
//! `inspect_drive` reads a watch that connects as a USB drive instead, and `issue_link` makes
//! the link that shares a watch report in a GitHub issue (`report`).

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use schemars::{JsonSchema, Schema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::drive;
use crate::error::{Error, ErrorCode};
use crate::extdata::{self, ExtData, FoundBy, Layout, Object};
use crate::fonts::{Content, Font, FontSet, Hosts, Kind, Listed, Pack, sha256_hex};
use crate::garmin_device::{self, GarminDevice};
use crate::mtp::{CreateError, Mtp, MtpError};
use crate::ptp::{self, ObjectInfo, op};
use crate::report::{
    self, Connection, HiddenFile, HiddenListing, HiddenStatus, IssueLink, IssueLinkInput,
    ListedFile, ReportInput, ReportedHosts, ReportedPart, WatchReport,
};
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
    /// Bitmap: every target's `.tmp` is this build's Georgian, waiting for a restart.
    Waiting,
    /// TrueType: every target's `.bin` is this build's Georgian.
    Installed,
    /// Some targets hold this build's Georgian, the rest Garmin's or nothing: install adds the
    /// rest.
    Partly,
    /// A target holds a Georgian build from an earlier release: install replaces it.
    Beta,
    /// Bitmap: every target's `.tmp` is Garmin's original, waiting for a restart.
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

/// The one rule behind [`GeorgianState`], from the target parts' files.
pub fn georgian_state(kind: Option<Kind>, files: &[FileState]) -> GeorgianState {
    use FileState as F;
    let Some(kind) = kind.filter(|_| !files.is_empty()) else {
        return GeorgianState::Unsupported;
    };
    let any = |s: F| files.contains(&s);
    let all = |s: F| files.iter().all(|f| *f == s);
    if any(F::Unknown) {
        GeorgianState::Unknown
    } else if any(F::Other) || (kind == Kind::Bitmap && any(F::Georgian) && any(F::Stock)) {
        GeorgianState::Other
    } else if any(F::GeorgianBeta) {
        GeorgianState::Beta
    } else if any(F::NotVisible) {
        GeorgianState::NotVisible
    } else if all(F::Georgian) {
        match kind {
            Kind::Bitmap => GeorgianState::Waiting,
            Kind::Truetype => GeorgianState::Installed,
        }
    } else if kind == Kind::Bitmap && all(F::Stock) {
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
pub static OPERATIONS: [Operation; 5] = [
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
                      with unsupported_device. If a part fails, this call's changes are undone: \
                      new files deleted, replaced files written back from the bytes read before. \
                      Disconnect USB and restart the watch to load them.",
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
                      it, report_in_url is false, and the form's report field asks for a paste of \
                      report_text. Touches nothing.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(IssueLinkInput),
        output: || schema_for!(IssueLink),
    },
];

pub fn operation(name: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|o| o.name == name)
}

/// `{"tools": [...]}`, as MCP's `tools/list` returns them.
pub fn tools() -> Value {
    json!({"tools": OPERATIONS.iter().map(Operation::tool).collect::<Vec<_>>()})
}

/// Opens a session with the watch.
pub type Connect<'a> = dyn FnMut() -> Result<Box<dyn Mtp>, Error> + 'a;

/// Runs operation `name` with JSON `input`. Input is validated before the watch is touched;
/// output is validated before it is returned.
pub fn call(
    name: &str,
    input: Value,
    fonts: &FontSet,
    connect: &mut Connect<'_>,
) -> Result<Value, Error> {
    let op = operation(name).ok_or_else(|| {
        Error::new(
            ErrorCode::InvalidInput,
            format!("unknown operation {name:?}"),
        )
    })?;
    check(&op.input_schema(), &input).map_err(|m| Error::new(ErrorCode::InvalidInput, m))?;
    let output = match name {
        "inspect" => {
            let InspectInput {} = parse(input)?;
            to_value(inspect(&mut *connect()?, fonts)?)
        }
        "inspect_drive" => {
            let InspectDriveInput { path } = parse(input)?;
            to_value(inspect_drive(path.as_deref(), fonts)?)
        }
        "issue_link" => {
            let input: IssueLinkInput = parse(input)?;
            to_value(report::issue_link(&input)?)
        }
        _ => {
            let StageInput { parts } = parse(input)?;
            let variant = if name == "install" {
                Variant::Georgian
            } else {
                Variant::Stock
            };
            if let Some(part) = parts.iter().flatten().find(|p| !fonts.has_part(p)) {
                return Err(Error::new(
                    ErrorCode::FontUnavailable,
                    format!("This build has no font part {part}."),
                ));
            }
            to_value(stage(&mut *connect()?, fonts, variant, parts.as_ref())?)
        }
    }?;
    check(&op.output_schema(), &output).map_err(|m| {
        Error::new(
            ErrorCode::Internal,
            format!("{name} produced invalid output: {m}"),
        )
    })?;
    Ok(output)
}

fn parse<T: for<'de> Deserialize<'de>>(input: Value) -> Result<T, Error> {
    serde_json::from_value(input).map_err(|e| Error::new(ErrorCode::InvalidInput, e.to_string()))
}

fn to_value<T: Serialize>(value: T) -> Result<Value, Error> {
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

fn hex_handle(handle: u32) -> String {
    format!("{handle:#010x}")
}

/// MTP operations a watch must support for staging.
const WRITE_OPERATIONS: [u16; 4] = [
    op::GET_OBJECT,
    op::SEND_OBJECT_INFO,
    op::SEND_OBJECT,
    op::DELETE_OBJECT,
];

fn missing_write_operations(dev: &mut dyn Mtp) -> Result<Vec<&'static str>, Error> {
    let info = dev.device_info()?;
    Ok(WRITE_OPERATIONS
        .iter()
        .filter(|code| !info.operations_supported.contains(code))
        .map(|&code| ptp::operation_name(code))
        .collect())
}

fn read_garmin_device(dev: &mut dyn Mtp, layout: &Layout) -> Result<GarminDevice, Error> {
    let handle = layout.garmin_device_xml.ok_or_else(|| {
        Error::new(
            ErrorCode::GarminFolderNotFound,
            "The watch has no GARMIN/GarminDevice.xml.",
        )
    })?;
    garmin_device::parse(&dev.get_object(handle)?)
}

/// Objects in `ExtData` under `font`'s file name.
fn staged_in<'a>(ext: &'a ExtData, font: &Font) -> Vec<&'a Object> {
    let name = font.file_name();
    ext.files
        .iter()
        .filter(|o| o.info.filename.eq_ignore_ascii_case(&name))
        .collect()
}

fn staged_path(font: &Font) -> String {
    format!("GARMIN/ExtData/{}", font.file_name())
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

/// What the file named for `font` holds, by the one rule in [`FileState`]: `files` is the
/// `ExtData` listing (`None` when it wasn't found or read), `contents` the files read, and
/// `hidden` the children the normal listing leaves out: a file only there is identified from
/// it, as install and restore do.
fn file_state(
    font: &Font,
    device: &GarminDevice,
    files: Option<&[Object]>,
    contents: &HashMap<u32, Vec<u8>>,
    hidden: &[HiddenFile],
) -> FileState {
    let Some(files) = files else {
        return FileState::Unknown;
    };
    let name = font.file_name();
    let found: Vec<&Object> = files
        .iter()
        .filter(|o| o.info.filename.eq_ignore_ascii_case(&name))
        .collect();
    let hidden_state = || {
        let mut named = hidden.iter().filter(|h| h.name.eq_ignore_ascii_case(&name));
        match (named.next(), named.next()) {
            (Some(one), None) => one.file,
            (Some(_), Some(_)) => Some(FileState::Other),
            _ => None,
        }
    };
    match found.as_slice() {
        [] => match hidden_state() {
            Some(state) => state,
            None if font.kind == Kind::Truetype && device.declares_extdata_part(&font.part) => {
                FileState::NotVisible
            }
            None => FileState::None,
        },
        [one] => match contents.get(&one.handle).map(|bytes| font.identify(bytes)) {
            Some(Content::Georgian) => FileState::Georgian,
            Some(Content::EarlierGeorgian) => FileState::GeorgianBeta,
            Some(Content::Stock) => FileState::Stock,
            Some(Content::Other) | None => FileState::Other,
        },
        _ => FileState::Other,
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
    match dev.device_info() {
        Ok(info)
            if info
                .operations_supported
                .contains(&op::GARMIN_GET_OBJECT_IDS) => {}
        Ok(_) => {
            return HiddenListing {
                status: HiddenStatus::Unsupported,
                files: Vec::new(),
                note: None,
            };
        }
        Err(e) => return failed(short(e)),
    }
    let handles = match dev.garmin_object_ids(storage, ext.handle) {
        Ok(handles) => handles,
        Err(e) => return failed(short(e)),
    };
    let listed: BTreeSet<u32> = ext.files.iter().map(|o| o.handle).collect();
    let mut files = Vec::new();
    let mut note = None;
    for handle in handles.into_iter().collect::<BTreeSet<_>>() {
        if listed.contains(&handle) {
            continue;
        }
        let info = match dev.object_info(handle) {
            Ok(info) => info,
            Err(e) => {
                note.get_or_insert_with(|| short(format!("{}: {e}", hex_handle(handle))));
                continue;
            }
        };
        // ExtData's own files only: never another folder's names.
        if info.parent != ext.handle || info.storage_id != storage || info.is_folder() {
            continue;
        }
        let size = info.compressed_size as usize;
        let named = |f: &Font| {
            info.filename.eq_ignore_ascii_case(&f.file_name())
                || (f.kind == Kind::Bitmap
                    && info
                        .filename
                        .eq_ignore_ascii_case(&format!("{}.fnt", f.part)))
        };
        let font = fonts
            .fonts()
            .map(|(_, f)| f)
            .find(|f| chosen(restore, f) && named(f));
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
        let ours = fonts.fonts().any(|(_, f)| {
            o.info.filename.eq_ignore_ascii_case(&f.file_name()) && f.known_size(size)
        });
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
        reports.push(FontReport {
            pack: pack.id.clone(),
            part: font.part.clone(),
            kind: font.kind,
            size_px: font.size_px,
            listed: device.declares_extdata_part(&font.part),
            target: writable && chosen(&hosts, font),
            restore: writable && chosen(&restore, font),
            file: file_state(font, &device, files, &contents, &extdata_hidden.files),
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
    let targets: Vec<FileState> = reports
        .iter()
        .filter(|r| r.target)
        .map(|r| r.file)
        .collect();
    let state = georgian_state(hosts.kind, &targets);
    let restorable = writable && !restore.listed.is_empty();
    let support = support::support(fonts, &device, &hosts, tested.status);
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

/// Deletes the object this run created after a failed transfer and says so in `cause`.
fn discard(dev: &mut dyn Mtp, handle: u32, path: &str, cause: Error) -> Error {
    let message = match dev.delete_object(handle) {
        Ok(()) => format!(
            "{}. A new object was created as {path} ({}) and has been deleted.",
            cause.message,
            hex_handle(handle)
        ),
        Err(e) => format!(
            "{}. A new object was created as {path} ({}) and could not be deleted ({e}); \
             it may hold incomplete data.",
            cause.message,
            hex_handle(handle)
        ),
    };
    Error::new(cause.code, message)
}

/// The objects in `parent` named `name` (case-insensitively), from a fresh listing.
fn named_objects(
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
            return Error::new(cause.code, message);
        }
    };
    if created.is_empty() {
        let message = format!("{}. No object was created.", cause.message);
        return Error::new(cause.code, message);
    }
    created
        .into_iter()
        .fold(cause, |cause, handle| discard(dev, handle, path, cause))
}

/// Writes `bytes` as `font`'s file in `ExtData`, which holds none under that name, and reads it
/// back. On failure the new object is deleted and the error says how that went.
fn write_verified(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    font: &Font,
    bytes: &[u8],
) -> Result<u32, Error> {
    let name = font.file_name();
    let path = staged_path(font);
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
enum Undo<'a> {
    /// A file this call created (a bitmap part's `.tmp`, or a `.bin` where the normal listing
    /// showed none): deleted.
    Created { handle: u32, path: String },
    /// A file this call set out to replace: `original`, read from the watch before anything was
    /// written, is put back.
    Replaced { font: &'a Font, original: Vec<u8> },
}

/// Puts `font`'s file back as it was before this call: `original`. A single file already
/// holding it is left alone; anything else under the name is deleted first, and `original` is
/// written and read back. Returns what happened, for the error.
fn restore_original(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    font: &Font,
    original: &[u8],
) -> String {
    let path = staged_path(font);
    let current = match named_objects(dev, storage, ext.handle, &font.file_name()) {
        Ok(current) => current,
        Err(e) => {
            return format!(
                "whether {path} holds its original is unknown (listing ExtData failed: {e})"
            );
        }
    };
    if let [one] = current.as_slice()
        && dev.get_object(*one).is_ok_and(|bytes| bytes == original)
    {
        return format!("{path} still holds the original");
    }
    for handle in current {
        if let Err(e) = dev.delete_object(handle) {
            return format!(
                "{path} ({}) could not be deleted ({e}), so its original was not written back",
                hex_handle(handle)
            );
        }
    }
    match write_verified(dev, storage, ext, font, original) {
        Ok(handle) => format!(
            "{path} written back from the original and verified ({})",
            hex_handle(handle)
        ),
        Err(e) => format!("{path} could not be written back: {}", e.message),
    }
}

/// After a part failed: undoes this run's changes, in order, and adds each result to `cause`.
/// Files staged by earlier runs were never changed.
fn roll_back(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    undo: &[Undo<'_>],
    cause: Error,
) -> Error {
    if undo.is_empty() {
        return cause;
    }
    let (mut created, mut replaced) = (false, false);
    let mut results = Vec::with_capacity(undo.len());
    for step in undo {
        results.push(match step {
            Undo::Created { handle, path } => {
                created = true;
                match dev.delete_object(*handle) {
                    Ok(()) => format!("{path} ({}) deleted", hex_handle(*handle)),
                    Err(e) => format!(
                        "{path} ({}) could not be deleted ({e})",
                        hex_handle(*handle)
                    ),
                }
            }
            Undo::Replaced { font, original } => {
                replaced = true;
                restore_original(dev, storage, ext, font, original)
            }
        });
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
    Error::new(
        cause.code,
        format!("{} {what}: {}.{tail}", cause.message, results.join("; ")),
    )
}

/// What a target needs, decided before anything is written.
enum Plan {
    /// The identical file is already there (in the normal listing or, imported, hidden).
    Already(u32),
    /// Write a new file: a bitmap part's `<part>.tmp`, or a TrueType part's `<part>.bin` where
    /// the normal listing shows none. A hidden read-only object under that name (an import) is
    /// never deleted; the watch imports the new file over it, as on the Forerunner 165 (#5,
    /// #19).
    Create,
    /// Delete `existing`, a file in the normal listing identified as Garmin's original or one
    /// of this program's builds, and write the new one; `original` is its bytes, for rollback.
    Replace { existing: u32, original: Vec<u8> },
}

/// Why a target can't be written; nothing is.
enum Refusal {
    /// The file under the part's name is neither Garmin's original nor one of this program's
    /// builds, or several files share the name.
    Other(String),
    /// The file can't be read and the watch declares another version or file name than this
    /// operation accepts: perhaps a newer Garmin font.
    Version(String),
    /// Install: the file can't be read and isn't declared at Garmin's version, so it may be one
    /// of this program's trailer-less builds or something else; restore first.
    RestoreFirst(String),
}

/// `ExtData`'s children the normal listing leaves out, with their info, when the watch lists
/// Garmin's `GetObjectIds` (0x9002); `None` when it doesn't, or listing fails. On the Venu 2
/// these are exactly the read-only imports.
fn hidden_objects(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
) -> Option<Vec<(u32, ObjectInfo)>> {
    let info = dev.device_info().ok()?;
    if !info
        .operations_supported
        .contains(&op::GARMIN_GET_OBJECT_IDS)
    {
        return None;
    }
    let handles = dev.garmin_object_ids(storage, ext.handle).ok()?;
    let listed: BTreeSet<u32> = ext.files.iter().map(|o| o.handle).collect();
    let mut hidden = Vec::new();
    for handle in handles.into_iter().collect::<BTreeSet<_>>() {
        if listed.contains(&handle) {
            continue;
        }
        // An object whose info can't be read stays unseen, like one the watch doesn't list.
        if let Ok(info) = dev.object_info(handle)
            && info.parent == ext.handle
            && info.storage_id == storage
            && !info.is_folder()
        {
            hidden.push((handle, info));
        }
    }
    Some(hidden)
}

/// Decides `font`'s Plan from what its file holds, by content: the file in the normal listing,
/// else the hidden one (`hidden`, when the watch lists 0x9002). Garmin's original or one of
/// this program's builds is replaced (a hidden one by writing a new file); anything else is
/// refused. Only a file that can't be read (not shown, or a hidden one whose read fails) falls
/// back to what the watch declares, for a TrueType part: install accepts only Garmin's version,
/// restore also a version below 1.00 ([`below_one`]), since it only ever puts Garmin's own file
/// back. A bitmap part's `.tmp` that isn't there is simply staged. A failed read of a file in
/// the normal listing stops the call: replacing it would delete a file that couldn't be kept
/// for rollback.
fn plan(
    dev: &mut dyn Mtp,
    ext: &ExtData,
    hidden: Option<&[(u32, ObjectInfo)]>,
    device: &GarminDevice,
    font: &Font,
    variant: Variant,
) -> Result<Result<Plan, Refusal>, Error> {
    let target = variant.bytes(font);
    let path = staged_path(font);
    match staged_in(ext, font).as_slice() {
        [one] => {
            if !font.known_size(one.info.compressed_size as usize) {
                return Ok(Err(Refusal::Other(path)));
            }
            let current = dev.get_object(one.handle)?;
            return Ok(if current == target {
                Ok(Plan::Already(one.handle))
            } else if font.identify(&current) == Content::Other {
                Err(Refusal::Other(path))
            } else {
                Ok(Plan::Replace {
                    existing: one.handle,
                    original: current,
                })
            });
        }
        [_, _, ..] => return Ok(Err(Refusal::Other(path))),
        [] => {}
    }
    let name = font.file_name();
    let named: Vec<&(u32, ObjectInfo)> = hidden
        .into_iter()
        .flatten()
        .filter(|(_, info)| info.filename.eq_ignore_ascii_case(&name))
        .collect();
    match named.as_slice() {
        [(handle, info)] => {
            if !font.known_size(info.compressed_size as usize) {
                return Ok(Err(Refusal::Other(path)));
            }
            // A failed read leaves the file unidentified: the fallback below decides.
            if let Ok(current) = dev.get_object(*handle) {
                return Ok(if current == target {
                    Ok(Plan::Already(*handle))
                } else if font.identify(&current) == Content::Other {
                    Err(Refusal::Other(path))
                } else {
                    Ok(Plan::Create)
                });
            }
        }
        [_, _, ..] => return Ok(Err(Refusal::Other(path))),
        [] => {}
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
    let named_right = declared.file_name.eq_ignore_ascii_case(&name);
    let described = format!(
        "{} as version {} ({}), where this build has version {} ({})",
        font.part,
        md(version),
        md(&declared.file_name),
        font.version,
        name
    );
    Ok(match variant {
        _ if !named_right => Err(Refusal::Version(described)),
        _ if garmins => Ok(Plan::Create),
        Variant::Stock if below_one(version) => Ok(Plan::Create),
        Variant::Georgian if below_one(version) => Err(Refusal::RestoreFirst(format!(
            "{path} (declared as version {})",
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
pub fn stage(
    dev: &mut dyn Mtp,
    fonts: &FontSet,
    variant: Variant,
    parts: Option<&BTreeSet<String>>,
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
        .any(|(_, font)| staged_in(ext, font).is_empty());
    let hidden = if needs_hidden {
        hidden_objects(dev, layout.storage, ext)
    } else {
        None
    };
    let mut plans = Vec::with_capacity(targets.len());
    let (mut others, mut versions, mut restore_first) = (Vec::new(), Vec::new(), Vec::new());
    for (_, font) in &targets {
        match plan(dev, ext, hidden.as_deref(), &device, font, variant)? {
            Ok(plan) => plans.push(plan),
            Err(Refusal::Other(path)) => others.push(path),
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
    if !others.is_empty() {
        let waiting = if others.iter().any(|p| p.ends_with(".tmp")) {
            " A bitmap part's .tmp may be a file waiting for the watch to restart: unplug the \
             watch and restart it, then try again."
        } else {
            ""
        };
        return Err(Error::new(
            ErrorCode::Conflict,
            format!(
                "{} {} neither Garmin's original font nor a Georgian build of this program; this \
                 build replaces only those, so nothing was written.{waiting}",
                and_list(&others),
                if others.len() == 1 { "holds" } else { "hold" }
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
    let mut undo: Vec<Undo<'_>> = Vec::new();
    let mut packs: Vec<StagedPack> = Vec::new();
    for ((pack, font), plan) in targets.into_iter().zip(plans) {
        let bytes = variant.bytes(font);
        let path = staged_path(font);
        let (status, handle) = match plan {
            Plan::Already(handle) => (StageStatus::AlreadyStaged, handle),
            Plan::Create => match write_verified(dev, storage, ext, font, bytes) {
                Ok(handle) => {
                    undo.push(Undo::Created {
                        handle,
                        path: path.clone(),
                    });
                    (StageStatus::StagedVerified, handle)
                }
                Err(e) => return Err(roll_back(dev, storage, ext, &undo, e)),
            },
            Plan::Replace { existing, original } => {
                undo.push(Undo::Replaced { font, original });
                let deleted = dev.delete_object(existing).map_err(|e| {
                    let e = Error::from(e);
                    Error::new(
                        e.code,
                        format!(
                            "Could not delete {path} ({}) to replace it: {}.",
                            hex_handle(existing),
                            e.message
                        ),
                    )
                });
                match deleted.and_then(|()| write_verified(dev, storage, ext, font, bytes)) {
                    Ok(handle) => (StageStatus::StagedVerified, handle),
                    Err(e) => return Err(roll_back(dev, storage, ext, &undo, e)),
                }
            }
        };
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
