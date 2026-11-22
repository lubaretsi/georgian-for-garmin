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
use sha2::{Digest, Sha256};

use crate::drive;
use crate::error::{Error, ErrorCode};
use crate::extdata::{self, ExtData, FoundBy, Layout, Object};
use crate::fonts::{Font, FontSet, Hosts, Kind, Listed, Pack};
use crate::garmin_device::{self, GarminDevice};
use crate::mtp::{CreateError, Mtp, MtpError};
use crate::ptp::{self, ObjectInfo, op};
use crate::report::{
    self, Connection, IssueLink, IssueLinkInput, ListedFile, ReportInput, ReportedHosts,
    ReportedPart, WatchReport,
};

/// This program's version, as reported in the watch report.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// What the part's file in `GARMIN/ExtData` holds now: `<part>.tmp` for a bitmap part,
/// `<part>.bin` for a TrueType part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Staged {
    /// No file has this name.
    None,
    /// This build's Georgian font.
    Georgian,
    /// Garmin's stock font.
    Stock,
    /// Some other file.
    Other,
    /// `ExtData` was not found, so its contents are unknown.
    Unknown,
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
    /// `install` and `restore_stock` write this part: the watch lists it, it is of the kind
    /// `hosts` chose, and the watch supports the MTP writes needed.
    pub target: bool,
    pub staged: Staged,
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
    /// build, with the size of its stock or Georgian version, is read; any other file (a
    /// multi-megabyte Chinese font, say) is null and never transferred, so inspecting stays
    /// fast.
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
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InspectOutput {
    pub device: DeviceReport,
    /// The watch lists a part of this build's packs and supports the MTP writes needed.
    pub supported: bool,
    /// Whether every part `install` writes was verified on this model and software.
    pub tested: TestReport,
    /// Which kind of font gets Georgian on this watch, and why.
    pub hosts: HostChoice,
    /// Every pack of that kind, in registry order, with a part the watch lists under
    /// `Garmin/ExtData`: what `install` writes. Empty if it lists none.
    pub packs: Vec<PackReport>,
    /// Every font part this build carries.
    pub fonts: Vec<FontReport>,
    /// The staging folder, or null if it could not be found.
    pub extdata: Option<ExtDataReport>,
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
    /// Its report; `connection` is `usb_drive`, no part is a target and staged states are
    /// `unknown`.
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

    /// The bytes of the other variant: what a TrueType part may be replacing.
    fn other_bytes(self, font: &Font) -> &[u8] {
        match self {
            Variant::Georgian => &font.stock,
            Variant::Stock => &font.georgian,
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
                      model and software, the overall tested status, what each part's file in the \
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
                      it back: a \
                      bitmap part is staged as GARMIN/ExtData/<part>.tmp, a TrueType part replaces \
                      GARMIN/ExtData/<part>.bin (or creates it where the watch shows none). Every \
                      target is checked first, and the whole call \
                      fails before anything is written if a staged .tmp differs from the build, if \
                      a .bin is neither Garmin's original nor this build's Georgian font, or if the \
                      watch lists a TrueType part at another version or file name than this build \
                      carries; identical files are skipped. If a part fails, this call's changes are \
                      undone: new .tmp files deleted, replaced .bin files written back from the \
                      bytes read before. Disconnect USB and restart the watch to load them.",
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
                      (bitmap) or over GARMIN/ExtData/<part>.bin (TrueType), with the same checks, \
                      readback and rollback as install. Disconnect USB and restart the watch to load \
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

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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
    if line.is_empty() { "(none)".into() } else { line }
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
    reports: &[FontReport],
    connection: Connection,
    extdata_listing: Option<Vec<ListedFile>>,
) -> WatchReport {
    let parts = fonts
        .fonts()
        .zip(reports)
        .filter(|(_, r)| r.listed)
        .map(|((pack, font), r)| ReportedPart {
            part: r.part.clone(),
            target: r.target,
            staged: r.staged,
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
            hosts: ReportedHosts {
                kind: hosts.kind,
                reason: hosts.reason.clone(),
            },
            parts,
            test_marks: fonts.test_marks(),
            extdata_listing,
        },
    )
}

/// `inspect`'s `hosts`: the choice `FontSet::hosts` made, in words.
fn host_choice(hosts: &Hosts<'_>) -> HostChoice {
    let skipped_parts: Vec<String> = hosts.skipped.iter().map(|f| f.part.clone()).collect();
    let reason = match hosts.kind {
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
        None => "This watch lists none of this build's fonts.".to_owned(),
    };
    HostChoice {
        kind: hosts.kind,
        reason,
        skipped_parts,
    }
}

pub fn inspect(dev: &mut dyn Mtp, fonts: &FontSet) -> Result<InspectOutput, Error> {
    let writable = missing_write_operations(dev)?.is_empty();
    let layout = extdata::discover(dev)?;
    let device = read_garmin_device(dev, &layout)?;
    let hosts = fonts.hosts(&device);
    // Read only the files that may be this build's: named for one of its parts, with the size
    // of that part's stock or Georgian version. Everything else (multi-megabyte Chinese fonts
    // on TrueType watches) is listed without being transferred; the app polls inspect.
    let mut contents: HashMap<u32, Vec<u8>> = HashMap::new();
    for o in layout.extdata.iter().flat_map(|ext| &ext.files) {
        let size = o.info.compressed_size as usize;
        let ours = fonts.fonts().any(|(_, f)| {
            o.info.filename.eq_ignore_ascii_case(&f.file_name())
                && (size == f.stock.len() || size == f.georgian.len())
        });
        if ours {
            contents.insert(o.handle, dev.get_object(o.handle)?);
        }
    }
    let mut reports = Vec::new();
    for (pack, font) in fonts.fonts() {
        let staged = match layout.extdata.as_ref().map(|ext| staged_in(ext, font)) {
            None => Staged::Unknown,
            Some(found) => match found.as_slice() {
                [] => Staged::None,
                [one] => match contents.get(&one.handle) {
                    Some(bytes) if *bytes == *font.georgian => Staged::Georgian,
                    Some(bytes) if *bytes == *font.stock => Staged::Stock,
                    _ => Staged::Other,
                },
                _ => Staged::Other,
            },
        };
        let listed = device.declares_extdata_part(&font.part);
        let chosen = hosts
            .listed
            .iter()
            .any(|l| l.fonts.iter().any(|f| f.part == font.part));
        reports.push(FontReport {
            pack: pack.id.clone(),
            part: font.part.clone(),
            kind: font.kind,
            size_px: font.size_px,
            listed,
            target: writable && chosen,
            staged,
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
        &reports,
        Connection::Mtp,
        listing,
    );
    Ok(InspectOutput {
        supported: writable && !packs.is_empty(),
        tested,
        hosts,
        packs,
        fonts: reports,
        extdata,
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
            staged: Staged::Unknown,
            mark: font.mark.as_ref().map(|m| m.variant.clone()),
        })
        .collect();
    let packs = pack_reports(&hosts.listed, &device);
    let tested = test_report(&hosts.listed, &packs, &device);
    let hosts = host_choice(&hosts);
    let report = report_of(
        &device,
        fonts,
        &hosts,
        &packs,
        tested.status,
        &reports,
        Connection::UsbDrive,
        listing,
    );
    Ok(InspectDriveOutput {
        path: root.display().to_string(),
        device: DeviceReport {
            model: device.description,
            part_number: device.part_number,
            software_version: device.software_version,
        },
        hosts,
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
    /// A bitmap part's `.tmp` this call created: deleted.
    Created { handle: u32, path: String },
    /// A TrueType part's `.bin` this call set out to replace: `original` (read from the watch
    /// before anything was written; `None` where it showed no file) is put back.
    Replaced {
        font: &'a Font,
        original: Option<Vec<u8>>,
    },
}

/// Puts `font`'s TrueType file back as it was before this call: `original`, or no file. A single
/// file already holding `original` is left alone; anything else under the name is deleted
/// first, and `original` is written and read back. Returns what happened, for the error.
fn restore_original(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    font: &Font,
    original: Option<&[u8]>,
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
    if let (Some(original), [one]) = (original, current.as_slice())
        && dev.get_object(*one).is_ok_and(|bytes| bytes == original)
    {
        return format!("{path} still holds the original");
    }
    let removed = !current.is_empty();
    for handle in current {
        if let Err(e) = dev.delete_object(handle) {
            return format!(
                "{path} ({}) could not be deleted ({e}), so its original was not written back",
                hex_handle(handle)
            );
        }
    }
    match original {
        None if removed => format!("{path} removed; the watch showed none before"),
        None => format!("{path} not left behind; the watch showed none before"),
        Some(original) => match write_verified(dev, storage, ext, font, original) {
            Ok(handle) => format!(
                "{path} written back from the original and verified ({})",
                hex_handle(handle)
            ),
            Err(e) => format!("{path} could not be written back: {}", e.message),
        },
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
                restore_original(dev, storage, ext, font, original.as_deref())
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
    /// The identical file is already there.
    Already(u32),
    /// A bitmap part: stage a new `<part>.tmp`.
    Stage,
    /// A TrueType part: delete `existing` (if the watch shows a file) and write the new one;
    /// `original` is the deleted file's bytes, for rollback.
    Replace {
        existing: Option<u32>,
        original: Option<Vec<u8>>,
    },
}

/// Decides `font`'s Plan from what `ExtData` holds under its name, or `None` for a conflict: a
/// different staged `.tmp`, more than one file, or a `.bin` that is neither Garmin's original
/// nor this build's Georgian font.
fn plan(
    dev: &mut dyn Mtp,
    ext: &ExtData,
    font: &Font,
    variant: Variant,
) -> Result<Option<Plan>, Error> {
    let bytes = variant.bytes(font);
    // Only a TrueType file may be replaced, and only by the other variant.
    let replaceable = (font.kind == Kind::Truetype).then(|| variant.other_bytes(font));
    Ok(match staged_in(ext, font).as_slice() {
        [] => Some(match font.kind {
            Kind::Bitmap => Plan::Stage,
            Kind::Truetype => Plan::Replace {
                existing: None,
                original: None,
            },
        }),
        [one] => {
            let size = one.info.compressed_size as usize;
            if size != bytes.len() && replaceable.is_none_or(|other| size != other.len()) {
                None
            } else {
                let current = dev.get_object(one.handle)?;
                if current == bytes {
                    Some(Plan::Already(one.handle))
                } else if replaceable.is_some_and(|other| current == other) {
                    Some(Plan::Replace {
                        existing: Some(one.handle),
                        original: Some(current),
                    })
                } else {
                    None
                }
            }
        }
        _ => None,
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
    let hosts = fonts.hosts(&device);
    let listed = hosts.listed;
    if listed.is_empty() {
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "{watch} lists none of this build's font packs under {}; nothing was written.",
                garmin_device::EXTDATA_PATH
            ),
        ));
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

    // A TrueType file is replaced only where the watch lists the version and file name this
    // build's original has: another version may be a newer Garmin font.
    let mismatched: Vec<String> = targets
        .iter()
        .filter(|(_, f)| f.kind == Kind::Truetype)
        .filter_map(|(_, font)| {
            let declared = device
                .extdata_files()
                .find(|d| d.part_number == font.part)?;
            let same = declared.version.as_deref() == Some(font.version.as_str())
                && declared.file_name.eq_ignore_ascii_case(&font.file_name());
            (!same).then(|| {
                format!(
                    "{} as version {} ({}), where this build has version {} ({})",
                    font.part,
                    md(declared.version.as_deref().unwrap_or("unknown")),
                    md(&declared.file_name),
                    font.version,
                    font.file_name()
                )
            })
        })
        .collect();
    if !mismatched.is_empty() {
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "{watch} lists {}. Nothing was written.",
                and_list(&mismatched)
            ),
        ));
    }

    let ext = layout.extdata.as_ref().ok_or_else(|| {
        Error::new(
            ErrorCode::ExtdataNotFound,
            "Could not find the watch's GARMIN/ExtData folder; nothing was written.",
        )
    })?;

    // Check every target before writing anything.
    let mut plans = Vec::with_capacity(targets.len());
    let (mut staged_conflicts, mut unknown) = (Vec::new(), Vec::new());
    for (_, font) in &targets {
        match plan(dev, ext, font, variant)? {
            Some(plan) => plans.push(plan),
            None if font.kind == Kind::Bitmap => staged_conflicts.push(staged_path(font)),
            None => unknown.push(staged_path(font)),
        }
    }
    if !staged_conflicts.is_empty() || !unknown.is_empty() {
        let mut sentences = Vec::new();
        if !staged_conflicts.is_empty() {
            sentences.push(format!(
                "A different file is already staged as {}; nothing was written. Unplug the watch \
                 and restart it so it loads that file, then try again.",
                staged_conflicts.join(", ")
            ));
        }
        if !unknown.is_empty() {
            sentences.push(format!(
                "{} {} neither Garmin's original font nor this build's Georgian one; this build \
                 replaces only those, so nothing was written.",
                and_list(&unknown),
                if unknown.len() == 1 { "holds" } else { "hold" }
            ));
        }
        return Err(Error::new(ErrorCode::Conflict, sentences.join(" ")));
    }

    let storage = layout.storage;
    let mut undo: Vec<Undo<'_>> = Vec::new();
    let mut packs: Vec<StagedPack> = Vec::new();
    for ((pack, font), plan) in targets.into_iter().zip(plans) {
        let bytes = variant.bytes(font);
        let path = staged_path(font);
        let (status, handle) = match plan {
            Plan::Already(handle) => (StageStatus::AlreadyStaged, handle),
            Plan::Stage => match write_verified(dev, storage, ext, font, bytes) {
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
                let deleted = match existing {
                    None => Ok(()),
                    Some(old) => dev.delete_object(old).map_err(|e| {
                        let e = Error::from(e);
                        Error::new(
                            e.code,
                            format!(
                                "Could not delete {path} ({}) to replace it: {}.",
                                hex_handle(old),
                                e.message
                            ),
                        )
                    }),
                };
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
