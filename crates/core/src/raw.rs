//! Raw MTP operations (cargo feature `dev`, debug builds only): the [`Mtp`] primitives that
//! read, a read-only raw PTP transaction, and the session's live trace, so an experiment is a
//! script rather than a new operation. The only writes, `mtp_create` and `mtp_delete`, are in
//! [`crate::dev`], behind the same checks as `stage_file`.

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorCode};
use crate::fonts::sha256_hex;
use crate::mtp::Mtp;
use crate::ptp::{self, op};

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::InvalidInput, message)
}

pub(crate) fn decode_base64(text: &str, field: &str) -> Result<Vec<u8>, Error> {
    STANDARD
        .decode(text)
        .map_err(|e| invalid(format!("{field} is not base64: {e}")))
}

/// Bytes returned inline unless the caller sets `max_bytes`.
const DEFAULT_MAX_INLINE: u64 = 1 << 20;

/// Bytes the watch sent: inline as base64, or written to a new local file.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Data {
    pub bytes: u64,
    pub sha256: String,
    /// The bytes, base64; null when they went to path.
    pub base64: Option<String>,
    /// The local file they were written to; null when inline.
    pub path: Option<String>,
}

/// Returns `bytes` inline as base64 up to `max_bytes`, or writes them to a new local file at
/// `path`.
fn deliver(bytes: Vec<u8>, path: Option<&str>, max_bytes: Option<u64>) -> Result<Data, Error> {
    let size = bytes.len() as u64;
    let sha256 = sha256_hex(&bytes);
    if let Some(path) = path {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|mut f| f.write_all(&bytes))
            .map_err(|e| invalid(format!("cannot create {path}: {e}")))?;
        return Ok(Data {
            bytes: size,
            sha256,
            base64: None,
            path: Some(path.to_owned()),
        });
    }
    let max = max_bytes.unwrap_or(DEFAULT_MAX_INLINE);
    if size > max {
        return Err(invalid(format!(
            "The watch sent {size} bytes, more than max_bytes ({max}) inline: give a path, or \
             read a range with offset and length."
        )));
    }
    Ok(Data {
        bytes: size,
        sha256,
        base64: Some(STANDARD.encode(&bytes)),
        path: None,
    })
}

/// No parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoInput {}

/// The `GetDeviceInfo` dataset, as the session read it when it opened.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DeviceInfoOutput {
    pub standard_version: u16,
    pub vendor_extension_id: u32,
    pub vendor_extension_version: u16,
    pub vendor_extension_desc: String,
    pub functional_mode: u16,
    pub operations_supported: Vec<u16>,
    pub events_supported: Vec<u16>,
    pub device_properties_supported: Vec<u16>,
    pub capture_formats: Vec<u16>,
    pub playback_formats: Vec<u16>,
    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
}

pub fn device_info(dev: &mut dyn Mtp) -> Result<DeviceInfoOutput, Error> {
    let i = dev.device_info()?;
    Ok(DeviceInfoOutput {
        standard_version: i.standard_version,
        vendor_extension_id: i.vendor_extension_id,
        vendor_extension_version: i.vendor_extension_version,
        vendor_extension_desc: i.vendor_extension_desc,
        functional_mode: i.functional_mode,
        operations_supported: i.operations_supported,
        events_supported: i.events_supported,
        device_properties_supported: i.device_properties_supported,
        capture_formats: i.capture_formats,
        playback_formats: i.playback_formats,
        manufacturer: i.manufacturer,
        model: i.model,
        device_version: i.device_version,
        serial_number: i.serial_number,
    })
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StorageIdsOutput {
    pub storage_ids: Vec<u32>,
}

pub fn storage_ids(dev: &mut dyn Mtp) -> Result<StorageIdsOutput, Error> {
    Ok(StorageIdsOutput {
        storage_ids: dev.storage_ids()?,
    })
}

/// Which storage.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorageInput {
    pub storage: u32,
}

/// The `GetStorageInfo` dataset.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StorageInfoOutput {
    pub storage: u32,
    pub storage_type: u16,
    pub filesystem_type: u16,
    pub access_capability: u16,
    pub max_capacity: u64,
    pub free_space_in_bytes: u64,
    pub free_space_in_objects: u32,
    pub storage_description: String,
    pub volume_identifier: String,
}

pub fn storage_info(dev: &mut dyn Mtp, input: &StorageInput) -> Result<StorageInfoOutput, Error> {
    let i = dev.storage_info(input.storage)?;
    Ok(StorageInfoOutput {
        storage: input.storage,
        storage_type: i.storage_type,
        filesystem_type: i.filesystem_type,
        access_capability: i.access_capability,
        max_capacity: i.max_capacity,
        free_space_in_bytes: i.free_space_in_bytes,
        free_space_in_objects: i.free_space_in_objects,
        storage_description: i.storage_description,
        volume_identifier: i.volume_identifier,
    })
}

/// Which objects to list.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HandlesInput {
    pub storage: u32,
    /// A folder's handle; 4294967295 (0xFFFFFFFF) for the storage's root, 0 for every object on
    /// it (GetObjectHandles only).
    pub parent: u32,
    /// List with Garmin's GetObjectIds (0x9002), which includes children GetObjectHandles
    /// leaves out, such as imported fonts; the watch must list the operation. Default false.
    pub hidden: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HandlesOutput {
    pub handles: Vec<u32>,
}

pub fn handles(dev: &mut dyn Mtp, input: &HandlesInput) -> Result<HandlesOutput, Error> {
    let handles = if input.hidden.unwrap_or(false) {
        let listed = dev
            .device_info()?
            .operations_supported
            .contains(&op::GARMIN_GET_OBJECT_IDS);
        if !listed {
            return Err(Error::new(
                ErrorCode::UnsupportedDevice,
                "The watch doesn't list Garmin's GetObjectIds (0x9002), which hidden needs.",
            ));
        }
        dev.garmin_object_ids(input.storage, input.parent)?
    } else {
        dev.object_handles(input.storage, input.parent)?
    };
    Ok(HandlesOutput { handles })
}

/// Which object.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HandleInput {
    pub handle: u32,
}

/// The `GetObjectInfo` dataset, and whether the object is read-only.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ObjectInfoOutput {
    pub handle: u32,
    pub storage_id: u32,
    pub format: u16,
    pub protection_status: u16,
    pub compressed_size: u32,
    pub parent: u32,
    pub association_type: u16,
    pub association_desc: u32,
    pub sequence_number: u32,
    pub filename: String,
    pub capture_date: String,
    pub modification_date: String,
    pub keywords: String,
    /// ProtectionStatus other than No Protection; null when the backend can't tell.
    pub read_only: Option<bool>,
}

pub fn object_info(dev: &mut dyn Mtp, input: &HandleInput) -> Result<ObjectInfoOutput, Error> {
    let i = dev.object_info(input.handle)?;
    let read_only = dev.read_only(input.handle, &i);
    Ok(ObjectInfoOutput {
        handle: input.handle,
        storage_id: i.storage_id,
        format: i.format,
        protection_status: i.protection_status,
        compressed_size: i.compressed_size,
        parent: i.parent,
        association_type: i.association_type,
        association_desc: i.association_desc,
        sequence_number: i.sequence_number,
        filename: i.filename,
        capture_date: i.capture_date,
        modification_date: i.modification_date,
        keywords: i.keywords,
        read_only,
    })
}

/// Which object to read, which bytes, and where they go.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadInput {
    pub handle: u32,
    /// The first byte to read. Default 0.
    pub offset: Option<u32>,
    /// Bytes to read at most. Default: to the end.
    pub length: Option<u32>,
    /// A local file to create for the data instead of returning it inline; it must not exist.
    pub path: Option<String>,
    /// The most bytes returned inline, as base64; more is an error asking for path or a range.
    /// Default 1048576.
    #[schemars(range(max = 16_777_216))]
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadOutput {
    pub handle: u32,
    pub offset: u32,
    pub data: Data,
}

/// The whole object, or with `offset` or `length` a range: GetPartialObject when the watch
/// lists it (USB), else the whole object cut to the range.
pub fn read(dev: &mut dyn Mtp, input: &ReadInput) -> Result<ReadOutput, Error> {
    let bytes = if input.offset.is_none() && input.length.is_none() {
        dev.get_object(input.handle)?
    } else {
        let (offset, length) = (input.offset.unwrap_or(0), input.length.unwrap_or(u32::MAX));
        dev.get_partial_object(input.handle, offset, length)?
    };
    Ok(ReadOutput {
        handle: input.handle,
        offset: input.offset.unwrap_or(0),
        data: deliver(bytes, input.path.as_deref(), input.max_bytes)?,
    })
}

/// The operations `ptp_transaction` sends: PTP's and MTP's reads, and Garmin's GetObjectIds.
pub const READ_OPERATIONS: [u16; 17] = [
    op::GET_DEVICE_INFO,
    op::GET_STORAGE_IDS,
    op::GET_STORAGE_INFO,
    op::GET_NUM_OBJECTS,
    op::GET_OBJECT_HANDLES,
    op::GET_OBJECT_INFO,
    op::GET_OBJECT,
    op::GET_THUMB,
    op::GET_PARTIAL_OBJECT,
    op::GET_DEVICE_PROP_DESC,
    op::GET_DEVICE_PROP_VALUE,
    op::GET_OBJECT_PROPS_SUPPORTED,
    op::GET_OBJECT_PROP_DESC,
    op::GET_OBJECT_PROP_VALUE,
    op::GET_OBJECT_PROP_LIST,
    op::GET_OBJECT_REFERENCES,
    op::GARMIN_GET_OBJECT_IDS,
];

/// `invalid_input` for an operation `ptp_transaction` doesn't send, saying why.
fn refuse(code: u16) -> Error {
    let name = ptp::known_operation_name(code).unwrap_or("operation");
    let why = match code {
        op::OPEN_SESSION | op::CLOSE_SESSION => {
            "the session opens and closes the watch's MTP session itself".to_owned()
        }
        op::FORMAT_STORE | op::RESET_DEVICE | op::POWER_DOWN => {
            "it erases, resets or switches off the watch".to_owned()
        }
        _ => {
            let reads: Vec<&str> = READ_OPERATIONS
                .iter()
                .map(|&c| ptp::operation_name(c))
                .collect();
            format!(
                "it isn't one of the reads it sends ({}); unknown vendor operations aren't either. \
                 Writes go through mtp_create and mtp_delete, which check what they touch",
                reads.join(", ")
            )
        }
    };
    invalid(format!(
        "ptp_transaction doesn't send {name} ({code:#06x}): {why}. Nothing was sent."
    ))
}

/// One read-only PTP transaction.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PtpTransactionInput {
    /// The operation code; only reads are sent (GetDeviceInfo, GetStorageIDs, GetStorageInfo,
    /// GetNumObjects, GetObjectHandles, GetObjectInfo, GetObject, GetThumb, GetPartialObject,
    /// GetDevicePropDesc, GetDevicePropValue, MTP's GetObjectPropsSupported, GetObjectPropDesc,
    /// GetObjectPropValue, GetObjectPropList and GetObjectReferences, and Garmin's 0x9002).
    pub opcode: u16,
    /// Up to five operation parameters. Default none.
    #[schemars(length(max = 5))]
    pub params: Option<Vec<u32>>,
    /// A local file to create for the data instead of returning it inline; it must not exist.
    pub path: Option<String>,
    /// The most bytes returned inline, as base64; more is an error asking for path. Default
    /// 1048576.
    #[schemars(range(max = 16_777_216))]
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PtpTransactionOutput {
    pub opcode: u16,
    /// The operation's name; null for one this program doesn't name.
    pub operation: Option<String>,
    /// The response code, OK (8193, 0x2001) or not.
    pub response_code: u16,
    /// The response code's name; null for one this program doesn't name.
    pub response: Option<String>,
    pub response_params: Vec<u32>,
    /// The data phase from the watch; zero bytes when it sent none.
    pub data: Data,
}

/// Whether `ptp_transaction` sends `code`, before anything touches the watch.
pub fn check_read(input: &PtpTransactionInput) -> Result<(), Error> {
    if !READ_OPERATIONS.contains(&input.opcode) {
        return Err(refuse(input.opcode));
    }
    Ok(())
}

pub fn ptp_transaction(
    dev: &mut dyn Mtp,
    input: &PtpTransactionInput,
) -> Result<PtpTransactionOutput, Error> {
    check_read(input)?;
    let params = input.params.as_deref().unwrap_or_default();
    let response = dev.ptp_transaction(input.opcode, params)?;
    Ok(PtpTransactionOutput {
        opcode: input.opcode,
        operation: ptp::known_operation_name(input.opcode).map(str::to_owned),
        response_code: response.code,
        response: ptp::response_name(response.code).map(str::to_owned),
        response_params: response.params,
        data: deliver(response.data, input.path.as_deref(), input.max_bytes)?,
    })
}

/// Whether the session sends every PTP transaction as `notifications/trace`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceInput {
    pub enabled: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TraceOutput {
    pub enabled: bool,
}
