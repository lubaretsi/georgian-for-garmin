//! Operations: one declaration each (JSON Schema in and out, behavior hints), validated on
//! every call. CLI, app and any future adapter call [`call`].

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema, schema_for};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::{Error, ErrorCode};
use crate::extdata::{self, FoundBy, Layout, Object};
use crate::fonts::{Font, FontSet, PARTS};
use crate::garmin_device::{self, GarminDevice};
use crate::mtp::Mtp;
use crate::ptp::{self, ObjectInfo, op};

/// A font pixel size with a Garmin part: 21, 24, 27 or 31.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizePx(u16);

impl SizePx {
    pub const DEFAULT: SizePx = SizePx(27);

    pub fn new(px: u16) -> Option<SizePx> {
        PARTS
            .iter()
            .any(|&(size, _)| size == px)
            .then_some(SizePx(px))
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl Default for SizePx {
    fn default() -> Self {
        SizePx::DEFAULT
    }
}

impl Serialize for SizePx {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u16(self.0)
    }
}

impl<'de> Deserialize<'de> for SizePx {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let px = u16::deserialize(d)?;
        SizePx::new(px)
            .ok_or_else(|| serde::de::Error::custom(format!("unsupported font size {px} px")))
    }
}

impl JsonSchema for SizePx {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "SizePx".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let sizes: Vec<u16> = PARTS.iter().map(|&(size, _)| size).collect();
        json_schema!({"type": "integer", "enum": sizes})
    }
}

/// `inspect` takes no parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectInput {}

/// Which font size to stage.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageInput {
    /// Font size in pixels. 27 is the size whose Georgian build was verified on a Venu 2.
    #[serde(default)]
    pub size_px: SizePx,
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
    pub part: String,
    pub size_px: u16,
    /// The size `install` uses when none is given.
    pub default: bool,
    /// The watch declares this part under `Garmin/ExtData` and supports the MTP writes needed.
    pub supported: bool,
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

#[derive(Debug, Serialize, JsonSchema)]
pub struct InspectOutput {
    pub device: DeviceReport,
    /// The watch supports at least one font part in this build.
    pub supported: bool,
    /// Every font part this build carries.
    pub fonts: Vec<FontReport>,
    /// The staging folder, or null if it could not be found.
    pub extdata: Option<ExtDataReport>,
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

#[derive(Debug, Serialize, JsonSchema)]
pub struct StageOutput {
    pub status: StageStatus,
    pub variant: Variant,
    pub part: String,
    pub size_px: u16,
    /// Path on the watch.
    pub path: String,
    /// MTP object handle of the staged file, hexadecimal.
    pub handle: String,
    pub bytes: u64,
    pub sha256: String,
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
        description: "Read the connected watch's model, software version, which font sizes it supports, \
                      and what is staged in its hidden GARMIN/ExtData folder. Read-only.",
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
        description: "Stage the Georgian-extended font as GARMIN/ExtData/<part>.tmp on the connected watch \
                      and verify it by reading it back. Disconnect USB and restart the watch to load it.",
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
        description: "Stage Garmin's original font as GARMIN/ExtData/<part>.tmp on the connected watch and \
                      verify it by reading it back. Disconnect USB and restart the watch to load it.",
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
            let StageInput { size_px } = parse(input)?;
            let variant = if name == "install" {
                Variant::Georgian
            } else {
                Variant::Stock
            };
            let font = fonts.by_size(size_px.get()).ok_or_else(|| {
                Error::new(
                    ErrorCode::FontUnavailable,
                    format!("This build does not include the {} px font.", size_px.get()),
                )
            })?;
            to_value(stage(&mut *connect()?, font, variant)?)
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

fn staged_files<'a>(layout: &'a Layout, font: &Font) -> Option<Vec<&'a Object>> {
    let name = font.staged_name();
    layout.extdata.as_ref().map(|ext| {
        ext.files
            .iter()
            .filter(|o| o.info.filename.eq_ignore_ascii_case(&name))
            .collect()
    })
}

pub fn inspect(dev: &mut dyn Mtp, fonts: &FontSet) -> Result<InspectOutput, Error> {
    let writable = missing_write_operations(dev)?.is_empty();
    let layout = extdata::discover(dev)?;
    let device = read_garmin_device(dev, &layout)?;
    let mut reports = Vec::new();
    for font in &fonts.fonts {
        let staged = match staged_files(&layout, font).as_deref() {
            None => Staged::Unknown,
            Some([]) => Staged::None,
            Some([one]) => {
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
            Some(_) => Staged::Other,
        };
        reports.push(FontReport {
            part: font.part.clone(),
            size_px: font.size_px,
            default: font.default,
            supported: writable && device.declares_extdata_part(&font.part),
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
    Ok(InspectOutput {
        supported: reports.iter().any(|f| f.supported),
        fonts: reports,
        extdata,
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

/// After `SendObjectInfo` failed: the watch may still have created the object. ExtData held
/// nothing under `name` before, so an object there now is this run's and is deleted.
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

pub fn stage(dev: &mut dyn Mtp, font: &Font, variant: Variant) -> Result<StageOutput, Error> {
    let bytes: &[u8] = match variant {
        Variant::Georgian => &font.georgian,
        Variant::Stock => &font.stock,
    };
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
    if !device.declares_extdata_part(&font.part) {
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "{} ({}) does not accept font part {} under {}; nothing was written.",
                device.description,
                device.part_number,
                font.part,
                garmin_device::EXTDATA_PATH
            ),
        ));
    }
    let name = font.staged_name();
    let path = format!("GARMIN/ExtData/{name}");
    let output = |status, handle| StageOutput {
        status,
        variant,
        part: font.part.clone(),
        size_px: font.size_px,
        path: path.clone(),
        handle: hex_handle(handle),
        bytes: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        next_step:
            "Disconnect the USB cable, then restart the watch; it loads the font on restart.".into(),
    };
    let conflict = || {
        Error::new(
            ErrorCode::Conflict,
            format!("A different file is already staged as {path}; nothing was written."),
        )
    };
    match staged_files(&layout, font).as_deref() {
        None => {
            return Err(Error::new(
                ErrorCode::ExtdataNotFound,
                "Could not find the watch's GARMIN/ExtData folder; nothing was written.",
            ));
        }
        Some([]) => {}
        Some([one]) => {
            if one.info.compressed_size as usize == bytes.len()
                && dev.get_object(one.handle)? == bytes
            {
                return Ok(output(StageStatus::AlreadyStaged, one.handle));
            }
            return Err(conflict());
        }
        Some(_) => return Err(conflict()),
    }
    let ext = layout.extdata.as_ref().expect("checked above");
    let size = u32::try_from(bytes.len()).map_err(|_| {
        Error::new(
            ErrorCode::Internal,
            "font exceeds the MTP object size limit",
        )
    })?;
    let info = ObjectInfo {
        storage_id: layout.storage,
        format: ptp::format::UNDEFINED,
        compressed_size: size,
        parent: ext.handle,
        filename: name.clone(),
        ..ObjectInfo::default()
    };
    let new = match dev.send_object_info(layout.storage, ext.handle, &info) {
        Ok(new) => new,
        Err(e) => {
            let cause = Error::from(e);
            return Err(discard_unconfirmed(
                dev,
                layout.storage,
                ext.handle,
                &name,
                &path,
                cause,
            ));
        }
    };
    if new.parent != ext.handle || new.storage != layout.storage {
        let cause = Error::new(
            ErrorCode::DeviceError,
            format!(
                "The watch placed {name} outside GARMIN/ExtData (parent {})",
                hex_handle(new.parent)
            ),
        );
        return Err(discard(dev, new.handle, &name, cause));
    }
    if let Err(e) = dev.send_object(bytes) {
        return Err(discard(dev, new.handle, &path, e.into()));
    }
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
        Ok(_) => Ok(output(StageStatus::StagedVerified, new.handle)),
    }
}
