//! Operations: one declaration each (JSON Schema in and out, behavior hints), validated on
//! every call. CLI, app and any future adapter call [`call`].
//!
//! A watch gets the first pack, in registry order, that it lists under `Garmin/ExtData`, and
//! `install`/`restore_stock` write every size of that pack the watch lists.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use schemars::{JsonSchema, Schema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::{Error, ErrorCode};
use crate::extdata::{self, ExtData, FoundBy, Layout, Object};
use crate::fonts::{Font, FontSet, Pack, Selection};
use crate::garmin_device::{self, GarminDevice};
use crate::mtp::{CreateError, Mtp};
use crate::ptp::{self, ObjectInfo, op};

/// This program's version, as reported in the watch report.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `inspect` takes no parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectInput {}

/// Which sizes of the selected pack to stage.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageInput {
    /// Pixel sizes to stage, e.g. `[27]`. Default: every size of the selected pack that the
    /// watch lists. A size the watch doesn't list is refused with `unsupported_device`.
    #[serde(default)]
    #[schemars(length(min = 1))]
    pub sizes_px: Option<BTreeSet<u16>>,
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

/// Whether a pack's Georgian build was seen working on this watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    /// Verified on this model part number and software version.
    Tested,
    /// Verified on this model part number, but only on other software versions.
    TestedOtherSoftware,
    /// Not verified on this model.
    Untested,
}

impl TestStatus {
    fn words(self) -> &'static str {
        match self {
            TestStatus::Tested => "tested on this model and software",
            TestStatus::TestedOtherSoftware => "tested on this model with other software",
            TestStatus::Untested => "untested on this model",
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TestReport {
    pub status: TestStatus,
    /// The pack's parts seen rendering Georgian on this model: on this software version when
    /// `tested`, on any version when `tested_other_software`; empty when `untested`.
    pub verified_parts: Vec<String>,
    /// Software versions this model was verified on with this pack, e.g. `["19.05"]`.
    pub verified_software: Vec<String>,
}

/// The pack this build installs on the watch.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PackReport {
    pub id: String,
    /// Garmin's product name, e.g. "Armenian Font, NotoSans Regular".
    pub name: String,
    /// The pack's parts the watch lists: what `install` and `restore_stock` write.
    pub parts: Vec<String>,
    /// Their pixel sizes, in the same order.
    pub sizes_px: Vec<u16>,
    pub tested: TestReport,
}

/// What `GARMIN/ExtData/<part>.tmp` holds now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Staged {
    /// No file is staged under this name.
    None,
    /// This build's Georgian font.
    Georgian,
    /// Garmin's stock font.
    Stock,
    /// Some other file.
    Other,
    /// `ExtData` was not found, so staging state is unknown.
    Unknown,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FontReport {
    /// Id of the pack this font belongs to.
    pub pack: String,
    pub part: String,
    pub size_px: u16,
    /// The watch lists this part under `Garmin/ExtData`.
    pub listed: bool,
    /// `install` and `restore_stock` write this part: the watch lists it, it's in the selected
    /// pack, and the watch supports the MTP writes needed.
    pub target: bool,
    pub staged: Staged,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StagedFile {
    pub name: String,
    /// MTP object handle, hexadecimal.
    pub handle: String,
    pub bytes: u32,
    /// SHA-256 of the file as read back from the watch.
    pub sha256: String,
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
    /// Files currently staged in the folder.
    pub files: Vec<StagedFile>,
}

/// A `Garmin/ExtData` update file from `GarminDevice.xml`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReportedFile {
    pub part_number: String,
    /// e.g. "2.10"; null if the watch gives none.
    pub version: Option<String>,
    pub file_name: String,
}

/// The watch's font list for sharing, e.g. in a GitHub issue so a new watch can be added. It
/// never includes the unit ID, serial number or any other `Id` element.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WatchReport {
    /// This program's version.
    pub app_version: String,
    pub model: String,
    pub model_part: String,
    pub software_version: String,
    /// Id of the selected pack, or null if the watch lists none of this build's packs.
    pub pack: Option<String>,
    pub tested: TestStatus,
    /// Every `Garmin/ExtData` update file the watch lists, in its order.
    pub extdata_files: Vec<ReportedFile>,
    /// A title for the issue.
    pub title: String,
    /// The report as Markdown for the issue body.
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InspectOutput {
    pub device: DeviceReport,
    /// A pack was selected and the watch supports the MTP writes needed.
    pub supported: bool,
    /// The pack `install` uses: the first, in registry order, with a part the watch lists
    /// under `Garmin/ExtData`. Null if it lists none.
    pub pack: Option<PackReport>,
    /// Every font part this build carries.
    pub fonts: Vec<FontReport>,
    /// The staging folder, or null if it could not be found.
    pub extdata: Option<ExtDataReport>,
    pub report: WatchReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    /// Written and read back byte for byte. The watch imports it after USB is disconnected and
    /// the watch restarts.
    StagedVerified,
    /// The identical file was already staged; nothing was written.
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
    pub size_px: u16,
    pub status: StageStatus,
    /// Path on the watch.
    pub path: String,
    /// MTP object handle of the staged file, hexadecimal.
    pub handle: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StageOutput {
    /// `already_staged` when every part already was; otherwise `staged_verified`.
    pub status: StageStatus,
    pub variant: Variant,
    /// Id of the selected pack.
    pub pack: String,
    /// Each part, in pack order.
    pub parts: Vec<StagedPart>,
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

// Behavior hints: the operations touch one attached watch (closed world). Staging never
// replaces an existing file (a different one is a conflict), so writes are not destructive,
// and repeating one returns `already_staged`.
pub static OPERATIONS: [Operation; 3] = [
    Operation {
        name: "inspect",
        title: "Inspect watch",
        description: "Read the connected watch's model and software version, the font pack this build \
                      installs on it (the first, in registry order, with a part the watch lists under \
                      Garmin/ExtData) and whether it was tested on this model and software, what is \
                      staged in the hidden GARMIN/ExtData folder, and a shareable report of the watch's \
                      font list without its unit ID or serial number. Read-only.",
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
        description: "Stage the Georgian build of every size of the selected pack that the watch lists \
                      (or only sizes_px) as GARMIN/ExtData/<part>.tmp, each verified by reading it back. \
                      Every target is checked first: an identical staged file is skipped, a different \
                      one fails the whole call with conflict before anything is written. If a part \
                      fails, the files this call created are deleted. Disconnect USB and restart the \
                      watch to load them.",
        read_only: false,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(StageInput),
        output: || schema_for!(StageOutput),
    },
    Operation {
        name: "restore_stock",
        title: "Restore stock font",
        description: "Stage Garmin's original font for every size of the selected pack that the watch \
                      lists (or only sizes_px) as GARMIN/ExtData/<part>.tmp, with the same checks, \
                      readback and rollback as install. Disconnect USB and restart the watch to load \
                      them.",
        read_only: false,
        destructive: false,
        idempotent: true,
        open_world: false,
        input: || schema_for!(StageInput),
        output: || schema_for!(StageOutput),
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
        _ => {
            let StageInput { sizes_px } = parse(input)?;
            let variant = if name == "install" {
                Variant::Georgian
            } else {
                Variant::Stock
            };
            if let Some(px) = sizes_px.iter().flatten().find(|&&px| !fonts.has_size(px)) {
                return Err(Error::new(
                    ErrorCode::FontUnavailable,
                    format!("This build has no {px} px font."),
                ));
            }
            to_value(stage(&mut *connect()?, fonts, variant, sizes_px.as_ref())?)
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

/// Objects in `ExtData` under `font`'s staged name.
fn staged_in<'a>(ext: &'a ExtData, font: &Font) -> Vec<&'a Object> {
    let name = font.staged_name();
    ext.files
        .iter()
        .filter(|o| o.info.filename.eq_ignore_ascii_case(&name))
        .collect()
}

fn staged_path(font: &Font) -> String {
    format!("GARMIN/ExtData/{}", font.staged_name())
}

/// Whether `pack` was verified on this watch, and on which parts and software versions.
fn test_report(pack: &Pack, device: &GarminDevice) -> TestReport {
    let same_model: Vec<_> = pack
        .tested
        .iter()
        .filter(|t| t.model_part.eq_ignore_ascii_case(&device.part_number))
        .collect();
    let exact: Vec<_> = same_model
        .iter()
        .copied()
        .filter(|t| t.software == device.software_version)
        .collect();
    let (status, verified) = if !exact.is_empty() {
        (TestStatus::Tested, exact)
    } else if !same_model.is_empty() {
        (TestStatus::TestedOtherSoftware, same_model.clone())
    } else {
        (TestStatus::Untested, Vec::new())
    };
    TestReport {
        status,
        verified_parts: pack
            .fonts
            .iter()
            .filter(|f| verified.iter().any(|t| t.parts.contains(&f.part)))
            .map(|f| f.part.clone())
            .collect(),
        verified_software: same_model
            .iter()
            .map(|t| t.software.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

fn pack_report(selection: &Selection<'_>, device: &GarminDevice) -> PackReport {
    PackReport {
        id: selection.pack.id.clone(),
        name: selection.pack.name.clone(),
        parts: selection.fonts.iter().map(|f| f.part.clone()).collect(),
        sizes_px: selection.fonts.iter().map(|f| f.size_px).collect(),
        tested: test_report(selection.pack, device),
    }
}

/// A watch-provided value as one line of Markdown text.
fn md(value: &str) -> String {
    let line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let escaped = line.replace('\\', "\\\\").replace('|', "\\|");
    if escaped.is_empty() {
        "(none)".into()
    } else {
        escaped
    }
}

/// The shareable report. Built only from the fields named here, so the unit ID, serial number
/// and every other `Id` element in `GarminDevice.xml` stay out of it.
fn watch_report(device: &GarminDevice, pack: Option<&PackReport>) -> WatchReport {
    let tested = pack.map_or(TestStatus::Untested, |p| p.tested.status);
    let extdata_files: Vec<ReportedFile> = device
        .extdata_files()
        .map(|f| ReportedFile {
            part_number: f.part_number.clone(),
            version: f.version.clone(),
            file_name: f.file_name.clone(),
        })
        .collect();
    let title = format!(
        "Watch report: {} ({}), software {}",
        md(&device.description),
        md(&device.part_number),
        md(&device.software_version)
    );
    let mut text = String::from(
        "<!-- Shared from Georgian for Garmin. It lists no unit ID or serial number. -->\n\n## Watch\n\n",
    );
    let pack_line = match pack {
        Some(p) => format!("{} ({})", md(&p.id), tested.words()),
        None => "none of this build's packs".into(),
    };
    for (label, value) in [
        ("Model", md(&device.description)),
        ("Model part number", md(&device.part_number)),
        ("Software version", md(&device.software_version)),
        ("Georgian for Garmin", md(APP_VERSION)),
        ("Font pack", pack_line),
    ] {
        writeln!(text, "- {label}: {value}").unwrap();
    }
    text.push_str("\n## Fonts the watch lists in Garmin/ExtData\n\n");
    if extdata_files.is_empty() {
        text.push_str("None.\n");
    } else {
        text.push_str("| Part number | Version | File name |\n| --- | --- | --- |\n");
        for f in &extdata_files {
            writeln!(
                text,
                "| {} | {} | {} |",
                md(&f.part_number),
                md(f.version.as_deref().unwrap_or("")),
                md(&f.file_name)
            )
            .unwrap();
        }
    }
    text.push_str(
        "\n## Result\n\n<!-- If you installed Georgian: after unplugging and restarting the watch, did \
         a notification with ქართული ᲥᲐᲠᲗᲣᲚᲘ show letters instead of boxes? -->\n",
    );
    WatchReport {
        app_version: APP_VERSION.into(),
        model: device.description.clone(),
        model_part: device.part_number.clone(),
        software_version: device.software_version.clone(),
        pack: pack.map(|p| p.id.clone()),
        tested,
        extdata_files,
        title,
        text,
    }
}

pub fn inspect(dev: &mut dyn Mtp, fonts: &FontSet) -> Result<InspectOutput, Error> {
    let writable = missing_write_operations(dev)?.is_empty();
    let layout = extdata::discover(dev)?;
    let device = read_garmin_device(dev, &layout)?;
    let selection = fonts.select(&device);
    let mut reports = Vec::new();
    for (pack, font) in fonts.fonts() {
        let staged = match layout.extdata.as_ref().map(|ext| staged_in(ext, font)) {
            None => Staged::Unknown,
            Some(found) => match found.as_slice() {
                [] => Staged::None,
                [one] => {
                    let size = one.info.compressed_size as usize;
                    if size != font.georgian.len() && size != font.stock.len() {
                        Staged::Other
                    } else {
                        let bytes = dev.get_object(one.handle)?;
                        if bytes == *font.georgian {
                            Staged::Georgian
                        } else if bytes == *font.stock {
                            Staged::Stock
                        } else {
                            Staged::Other
                        }
                    }
                }
                _ => Staged::Other,
            },
        };
        let listed = device.declares_extdata_part(&font.part);
        reports.push(FontReport {
            pack: pack.id.clone(),
            part: font.part.clone(),
            size_px: font.size_px,
            listed,
            target: writable && listed && selection.as_ref().is_some_and(|s| s.pack.id == pack.id),
            staged,
        });
    }
    let extdata = match layout.extdata {
        None => None,
        Some(ext) => {
            let mut files = Vec::new();
            for o in ext.files {
                let sha256 = sha256_hex(&dev.get_object(o.handle)?);
                files.push(StagedFile {
                    name: o.info.filename,
                    handle: hex_handle(o.handle),
                    bytes: o.info.compressed_size,
                    sha256,
                });
            }
            Some(ExtDataReport {
                handle: hex_handle(ext.handle),
                storage_id: hex_handle(layout.storage),
                listed_under_garmin: ext.listed_under_garmin,
                found_by: ext.found_by,
                files,
            })
        }
    };
    let pack = selection.as_ref().map(|s| pack_report(s, &device));
    let report = watch_report(&device, pack.as_ref());
    Ok(InspectOutput {
        supported: writable && pack.is_some(),
        pack,
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

/// After creating an object failed unconfirmed: the watch may still have created it. ExtData
/// held nothing under `name` before, so an object there now is this run's and is deleted.
fn discard_unconfirmed(
    dev: &mut dyn Mtp,
    storage: u32,
    parent: u32,
    name: &str,
    path: &str,
    cause: Error,
) -> Error {
    let mut created = Vec::new();
    let listing = dev.object_handles(storage, parent).and_then(|handles| {
        for handle in handles {
            if dev.object_info(handle)?.filename.eq_ignore_ascii_case(name) {
                created.push(handle);
            }
        }
        Ok(())
    });
    if let Err(e) = listing {
        let message = format!(
            "{}. Whether the watch created {path} is unknown: listing ExtData failed ({e}).",
            cause.message
        );
        return Error::new(cause.code, message);
    }
    if created.is_empty() {
        let message = format!("{}. No object was created.", cause.message);
        return Error::new(cause.code, message);
    }
    created
        .into_iter()
        .fold(cause, |cause, handle| discard(dev, handle, path, cause))
}

/// Writes `bytes` as `font`'s staged file in `ExtData` and reads it back. On failure the new
/// object is deleted and the error says how that went.
fn write_verified(
    dev: &mut dyn Mtp,
    storage: u32,
    ext: &ExtData,
    font: &Font,
    bytes: &[u8],
) -> Result<u32, Error> {
    let name = font.staged_name();
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

/// After a part failed: deletes every object this run created before it, and adds each
/// result to `cause`. Files staged by earlier runs were never in `created`.
fn roll_back(dev: &mut dyn Mtp, created: &[(u32, String)], cause: Error) -> Error {
    if created.is_empty() {
        return cause;
    }
    let results: Vec<String> = created
        .iter()
        .map(|(handle, path)| match dev.delete_object(*handle) {
            Ok(()) => format!("{path} ({}) deleted", hex_handle(*handle)),
            Err(e) => format!(
                "{path} ({}) could not be deleted ({e})",
                hex_handle(*handle)
            ),
        })
        .collect();
    Error::new(
        cause.code,
        format!(
            "{} Rolled back the files this run had staged: {}. Files staged before this run were \
             left in place.",
            cause.message,
            results.join("; ")
        ),
    )
}

/// Stages `variant` of every size of the selected pack the watch lists, or of `sizes_px`.
pub fn stage(
    dev: &mut dyn Mtp,
    fonts: &FontSet,
    variant: Variant,
    sizes_px: Option<&BTreeSet<u16>>,
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
    let selection = fonts.select(&device).ok_or_else(|| {
        Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "{watch} lists none of this build's font packs under {}; nothing was written.",
                garmin_device::EXTDATA_PATH
            ),
        )
    })?;
    let targets: Vec<&Font> = match sizes_px {
        None => selection.fonts.clone(),
        Some(sizes) => {
            let listed: Vec<u16> = selection.fonts.iter().map(|f| f.size_px).collect();
            if let Some(px) = sizes.iter().find(|px| !listed.contains(px)) {
                return Err(Error::new(
                    ErrorCode::UnsupportedDevice,
                    format!(
                        "{watch} doesn't list the {px} px part of {}; it lists {listed:?} px. \
                         Nothing was written.",
                        selection.pack.name
                    ),
                ));
            }
            selection
                .fonts
                .iter()
                .copied()
                .filter(|f| sizes.contains(&f.size_px))
                .collect()
        }
    };
    let ext = layout.extdata.as_ref().ok_or_else(|| {
        Error::new(
            ErrorCode::ExtdataNotFound,
            "Could not find the watch's GARMIN/ExtData folder; nothing was written.",
        )
    })?;

    // Check every target before writing anything.
    let mut already = Vec::with_capacity(targets.len());
    let mut conflicts = Vec::new();
    for font in &targets {
        let bytes = variant.bytes(font);
        match staged_in(ext, font).as_slice() {
            [] => already.push(None),
            [one]
                if one.info.compressed_size as usize == bytes.len()
                    && dev.get_object(one.handle)? == bytes =>
            {
                already.push(Some(one.handle));
            }
            _ => conflicts.push(staged_path(font)),
        }
    }
    if !conflicts.is_empty() {
        return Err(Error::new(
            ErrorCode::Conflict,
            format!(
                "A different file is already staged as {}; nothing was written.",
                conflicts.join(", ")
            ),
        ));
    }

    let mut created: Vec<(u32, String)> = Vec::new();
    let mut parts = Vec::with_capacity(targets.len());
    for (font, already) in targets.into_iter().zip(already) {
        let bytes = variant.bytes(font);
        let path = staged_path(font);
        let (status, handle) = match already {
            Some(handle) => (StageStatus::AlreadyStaged, handle),
            None => match write_verified(dev, layout.storage, ext, font, bytes) {
                Ok(handle) => {
                    created.push((handle, path.clone()));
                    (StageStatus::StagedVerified, handle)
                }
                Err(e) => return Err(roll_back(dev, &created, e)),
            },
        };
        parts.push(StagedPart {
            part: font.part.clone(),
            size_px: font.size_px,
            status,
            path,
            handle: hex_handle(handle),
            bytes: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        });
    }
    let status = if created.is_empty() {
        StageStatus::AlreadyStaged
    } else {
        StageStatus::StagedVerified
    };
    Ok(StageOutput {
        status,
        variant,
        pack: selection.pack.id.clone(),
        parts,
        next_step:
            "Disconnect the USB cable, then restart the watch; it loads the fonts on restart."
                .into(),
    })
}
