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

/// What a masked identifier reads as.
pub const REDACTED: &str = "<redacted>";

/// Whether to return the watch's identifiers unmasked.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceInfoInput {
    /// Return the serial number instead of "<redacted>". Never paste such a result anywhere
    /// shared (issues, pull requests, lab records). Default false.
    pub reveal_identifiers: Option<bool>,
}

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
    /// "<redacted>" unless reveal_identifiers.
    pub serial_number: String,
}

pub fn device_info(dev: &mut dyn Mtp, input: &DeviceInfoInput) -> Result<DeviceInfoOutput, Error> {
    let i = dev.device_info()?;
    let serial_number = if input.reveal_identifiers == Some(true) {
        i.serial_number
    } else {
        REDACTED.to_owned()
    };
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
        serial_number,
    })
}

/// The byte that replaces an identifier's characters: `*`, as one byte in XML and one UCS-2
/// unit in a PTP string, so the data keeps its length and still parses.
const MASK: u8 = b'*';
/// MTP device properties that can hold the owner's name.
const SYNCHRONIZATION_PARTNER: u32 = 0xD401;
const DEVICE_FRIENDLY_NAME: u32 = 0xD402;
/// The PTP datatype code of a string.
const STR: u16 = 0xFFFF;
/// The file whose `<Id>` is the watch's unit ID.
const GARMIN_DEVICE_XML: &str = "GarminDevice.xml";
/// The GarminDevice.xml elements whose values are masked: `<Id>`, the unit ID, and `<Ifix>` and
/// `<AppId>`, whose values no second sample shows aren't per watch.
const MASKED_ELEMENTS: [&str; 3] = ["Id", "Ifix", "AppId"];

/// Masks the characters of the PTP string at `at` (its count byte), keeping its length;
/// returns the offset after it, or `None` when the bytes end first.
fn mask_ptp_string(data: &mut [u8], at: usize) -> Option<usize> {
    let count = usize::from(*data.get(at)?);
    let end = at + 1 + 2 * count;
    for unit in data.get_mut(at + 1..end)?.chunks_exact_mut(2) {
        if unit != [0, 0] {
            unit.copy_from_slice(&u16::from(MASK).to_le_bytes());
        }
    }
    Some(end)
}

/// The byte ranges of the values of every [`MASKED_ELEMENTS`] element in GarminDevice.xml.
fn unit_id_spans(xml: &[u8]) -> Vec<std::ops::Range<usize>> {
    let find = |from: usize, what: &[u8]| {
        xml.get(from..)?
            .windows(what.len())
            .position(|w| w == what)
            .map(|i| from + i)
    };
    let mut spans = Vec::new();
    for element in MASKED_ELEMENTS {
        let (open, close) = (format!("<{element}>"), format!("</{element}>"));
        let mut at = 0;
        while let Some(start) = find(at, open.as_bytes()).map(|i| i + open.len()) {
            let end = find(start, close.as_bytes()).unwrap_or(xml.len());
            spans.push(start..end);
            at = end;
        }
    }
    spans
}

/// Masks `spans` (offsets in the whole object) in `data`, which starts at `offset` of it.
fn mask_spans(data: &mut [u8], offset: usize, spans: &[std::ops::Range<usize>]) {
    for span in spans {
        let start = span.start.saturating_sub(offset).min(data.len());
        let end = span.end.saturating_sub(offset).min(data.len());
        data[start..end].fill(MASK);
    }
}

/// Whether object `handle` is GarminDevice.xml.
fn holds_unit_id(dev: &mut dyn Mtp, handle: u32) -> Result<bool, Error> {
    let name = dev.object_info(handle)?.filename;
    Ok(name.eq_ignore_ascii_case(GARMIN_DEVICE_XML))
}

/// Masks the identifiers ([`MASKED_ELEMENTS`]) in `bytes`, the whole object, when `name` is
/// GarminDevice.xml (any case); the rest stays byte for byte.
pub(crate) fn mask_unit_id(name: &str, bytes: &mut [u8]) {
    if name.eq_ignore_ascii_case(GARMIN_DEVICE_XML) {
        let spans = unit_id_spans(bytes);
        mask_spans(bytes, 0, &spans);
    }
}

fn unmaskable(what: &str) -> Error {
    Error::new(
        ErrorCode::DeviceError,
        format!(
            "Could not find the {what} in the watch's data to mask it, so nothing is returned; \
             reveal_identifiers returns the data as it is."
        ),
    )
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
    /// Return GarminDevice.xml's <Id>, <Ifix> and <AppId> instead of asterisks. Never paste such a
    /// result anywhere shared (issues, pull requests, lab records). Default false.
    pub reveal_identifiers: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadOutput {
    pub handle: u32,
    pub offset: u32,
    pub data: Data,
}

/// The whole object, or with `offset` or `length` a range: GetPartialObject when the watch
/// lists it (USB), else the whole object cut to the range. GarminDevice.xml's unit ID is masked
/// unless `reveal_identifiers`; that file is always read whole to find it.
pub fn read(dev: &mut dyn Mtp, input: &ReadInput) -> Result<ReadOutput, Error> {
    let whole = input.offset.is_none() && input.length.is_none();
    let (offset, length) = (input.offset.unwrap_or(0), input.length.unwrap_or(u32::MAX));
    let bytes = if input.reveal_identifiers != Some(true) && holds_unit_id(dev, input.handle)? {
        let mut xml = dev.get_object(input.handle)?;
        mask_unit_id(GARMIN_DEVICE_XML, &mut xml);
        crate::mtp::byte_range(xml, offset, length)
    } else if whole {
        dev.get_object(input.handle)?
    } else {
        dev.get_partial_object(input.handle, offset, length)?
    };
    Ok(ReadOutput {
        handle: input.handle,
        offset,
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
    /// Return identifiers unmasked: the serial number in GetDeviceInfo, the owner's names in
    /// GetDevicePropDesc/GetDevicePropValue of SynchronizationPartner (0xD401) and
    /// DeviceFriendlyName (0xD402), and the <Id>, <Ifix> and <AppId> in GetObject/GetPartialObject of
    /// GarminDevice.xml. By default their characters read as asterisks, keeping the data's
    /// length. Never paste such a result anywhere shared (issues, pull requests, lab records).
    /// Default false.
    pub reveal_identifiers: Option<bool>,
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
    let mut response = dev.ptp_transaction(input.opcode, params)?;
    if input.reveal_identifiers != Some(true) && !response.data.is_empty() {
        mask_identifiers(dev, input.opcode, params, &mut response.data)?;
    }
    Ok(PtpTransactionOutput {
        opcode: input.opcode,
        operation: ptp::known_operation_name(input.opcode).map(str::to_owned),
        response_code: response.code,
        response: ptp::response_name(response.code).map(str::to_owned),
        response_params: response.params,
        data: deliver(response.data, input.path.as_deref(), input.max_bytes)?,
    })
}

/// Masks the identifiers a read can return, in place and keeping the data's length: the
/// serial number in GetDeviceInfo, the owner's names in the SynchronizationPartner and
/// DeviceFriendlyName properties, the unit ID in GarminDevice.xml. Data in which one can't be
/// found is withheld.
fn mask_identifiers(
    dev: &mut dyn Mtp,
    opcode: u16,
    params: &[u32],
    data: &mut [u8],
) -> Result<(), Error> {
    let param = |i: usize| params.get(i).copied().unwrap_or(0);
    match opcode {
        op::GET_DEVICE_INFO => {
            let at = ptp::DeviceInfo::decode_with_serial_offset(data)
                .map_err(|_| unmaskable("serial number"))?
                .1;
            mask_ptp_string(data, at).ok_or_else(|| unmaskable("serial number"))?;
        }
        op::GET_DEVICE_PROP_VALUE
            if matches!(param(0), SYNCHRONIZATION_PARTNER | DEVICE_FRIENDLY_NAME) =>
        {
            mask_ptp_string(data, 0).ok_or_else(|| unmaskable("owner's name"))?;
        }
        op::GET_DEVICE_PROP_DESC
            if matches!(param(0), SYNCHRONIZATION_PARTNER | DEVICE_FRIENDLY_NAME) =>
        {
            // Property code (u16), datatype (u16), get/set (u8), then the factory default
            // and current values.
            let string = data.get(2..4) == Some(STR.to_le_bytes().as_slice());
            let masked = string
                .then(|| mask_ptp_string(data, 5).and_then(|next| mask_ptp_string(data, next)))
                .flatten();
            masked.ok_or_else(|| unmaskable("owner's name"))?;
        }
        op::GET_OBJECT | op::GET_PARTIAL_OBJECT if holds_unit_id(dev, param(0))? => {
            let spans = unit_id_spans(&dev.get_object(param(0))?);
            let offset = if opcode == op::GET_PARTIAL_OBJECT {
                param(1) as usize
            } else {
                0
            };
            mask_spans(data, offset, &spans);
        }
        _ => {}
    }
    Ok(())
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
