//! PTP/MTP wire format: little-endian containers and datasets (PTP 1.1, ISO 15740).

use std::fmt;

/// Operation codes used by this client.
pub mod op {
    pub const GET_DEVICE_INFO: u16 = 0x1001;
    pub const OPEN_SESSION: u16 = 0x1002;
    pub const CLOSE_SESSION: u16 = 0x1003;
    pub const GET_STORAGE_IDS: u16 = 0x1004;
    pub const GET_OBJECT_HANDLES: u16 = 0x1007;
    pub const GET_OBJECT_INFO: u16 = 0x1008;
    pub const GET_OBJECT: u16 = 0x1009;
    pub const DELETE_OBJECT: u16 = 0x100B;
    pub const SEND_OBJECT_INFO: u16 = 0x100C;
    pub const SEND_OBJECT: u16 = 0x100D;
}

/// Object format codes.
pub mod format {
    pub const UNDEFINED: u16 = 0x3000;
    pub const ASSOCIATION: u16 = 0x3001;
}

/// Response codes this client distinguishes.
pub mod response {
    pub const OK: u16 = 0x2001;
    pub const SESSION_ALREADY_OPEN: u16 = 0x201E;
}

/// `GetObjectHandles` parent value selecting every object regardless of hierarchy.
pub const ALL_OBJECTS: u32 = 0x0000_0000;
/// `GetObjectHandles` parent value selecting root-level objects.
pub const ROOT: u32 = 0xFFFF_FFFF;

pub const HEADER_LEN: usize = 12;
const MAX_PARAMS: usize = 5;

pub const COMMAND: u16 = 1;
pub const DATA: u16 = 2;
pub const RESPONSE: u16 = 3;

pub fn operation_name(code: u16) -> &'static str {
    match code {
        op::GET_DEVICE_INFO => "GetDeviceInfo",
        op::OPEN_SESSION => "OpenSession",
        op::CLOSE_SESSION => "CloseSession",
        op::GET_STORAGE_IDS => "GetStorageIDs",
        op::GET_OBJECT_HANDLES => "GetObjectHandles",
        op::GET_OBJECT_INFO => "GetObjectInfo",
        op::GET_OBJECT => "GetObject",
        op::DELETE_OBJECT => "DeleteObject",
        op::SEND_OBJECT_INFO => "SendObjectInfo",
        op::SEND_OBJECT => "SendObject",
        _ => "operation",
    }
}

pub fn response_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0x2001 => "OK",
        0x2002 => "GeneralError",
        0x2003 => "SessionNotOpen",
        0x2004 => "InvalidTransactionID",
        0x2005 => "OperationNotSupported",
        0x2006 => "ParameterNotSupported",
        0x2007 => "IncompleteTransfer",
        0x2008 => "InvalidStorageID",
        0x2009 => "InvalidObjectHandle",
        0x200A => "DevicePropNotSupported",
        0x200B => "InvalidObjectFormatCode",
        0x200C => "StoreFull",
        0x200D => "ObjectWriteProtected",
        0x200E => "StoreReadOnly",
        0x200F => "AccessDenied",
        0x2010 => "NoThumbnailPresent",
        0x2012 => "PartialDeletion",
        0x2013 => "StoreNotAvailable",
        0x2014 => "SpecificationByFormatUnsupported",
        0x2015 => "NoValidObjectInfo",
        0x2019 => "DeviceBusy",
        0x201A => "InvalidParentObject",
        0x201D => "InvalidParameter",
        0x201E => "SessionAlreadyOpen",
        0x201F => "TransactionCancelled",
        0xA809 => "ObjectTooLarge",
        _ => return None,
    })
}

/// Malformed bytes from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError(pub String);

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn protocol(message: impl Into<String>) -> ProtocolError {
    ProtocolError(message.into())
}

/// A container header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub length: u32,
    pub kind: u16,
    pub code: u16,
    pub transaction: u32,
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Result<Header, ProtocolError> {
        let mut r = Reader::new(bytes);
        let header = Header {
            length: r.u32()?,
            kind: r.u16()?,
            code: r.u16()?,
            transaction: r.u32()?,
        };
        if (header.length as usize) < HEADER_LEN {
            return Err(protocol(format!(
                "container length {} is below the header size",
                header.length
            )));
        }
        Ok(header)
    }

    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0; HEADER_LEN];
        out[0..4].copy_from_slice(&self.length.to_le_bytes());
        out[4..6].copy_from_slice(&self.kind.to_le_bytes());
        out[6..8].copy_from_slice(&self.code.to_le_bytes());
        out[8..12].copy_from_slice(&self.transaction.to_le_bytes());
        out
    }
}

/// A command container with up to five parameters.
pub fn encode_command(code: u16, transaction: u32, params: &[u32]) -> Vec<u8> {
    assert!(
        params.len() <= MAX_PARAMS,
        "PTP commands carry at most five parameters"
    );
    let length = HEADER_LEN + 4 * params.len();
    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(
        &Header {
            length: length as u32,
            kind: COMMAND,
            code,
            transaction,
        }
        .encode(),
    );
    for p in params {
        out.extend_from_slice(&p.to_le_bytes());
    }
    out
}

/// A data container: header followed by the payload.
pub fn encode_data(code: u16, transaction: u32, payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let length = u32::try_from(HEADER_LEN + payload.len())
        .map_err(|_| protocol("data phase exceeds 4 GiB"))?;
    let mut out = Vec::with_capacity(length as usize);
    out.extend_from_slice(
        &Header {
            length,
            kind: DATA,
            code,
            transaction,
        }
        .encode(),
    );
    out.extend_from_slice(payload);
    Ok(out)
}

/// A response container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub code: u16,
    pub transaction: u32,
    pub params: Vec<u32>,
}

impl Response {
    pub fn parse(bytes: &[u8]) -> Result<Response, ProtocolError> {
        let header = Header::parse(bytes)?;
        if header.kind != RESPONSE {
            return Err(protocol(format!(
                "expected a response container, got type {}",
                header.kind
            )));
        }
        let len = header.length as usize;
        if len != bytes.len()
            || !(len - HEADER_LEN).is_multiple_of(4)
            || len > HEADER_LEN + 4 * MAX_PARAMS
        {
            return Err(protocol(format!(
                "malformed response container of {} bytes",
                bytes.len()
            )));
        }
        let params = bytes[HEADER_LEN..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Ok(Response {
            code: header.code,
            transaction: header.transaction,
            params,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let length = HEADER_LEN + 4 * self.params.len();
        let mut out = Vec::with_capacity(length);
        out.extend_from_slice(
            &Header {
                length: length as u32,
                kind: RESPONSE,
                code: self.code,
                transaction: self.transaction,
            }
            .encode(),
        );
        for p in &self.params {
            out.extend_from_slice(&p.to_le_bytes());
        }
        out
    }
}

/// Cursor over a little-endian dataset.
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| {
                protocol(format!(
                    "dataset truncated at byte {} (needed {n} more)",
                    self.pos
                ))
            })?;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, ProtocolError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, ProtocolError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A PTP string: a count of UCS-2 units including the terminating NUL, then the units.
    /// Unpaired surrogates decode as U+FFFD so one odd filename cannot break a listing scan.
    pub fn string(&mut self) -> Result<String, ProtocolError> {
        let count = self.u8()? as usize;
        let units = self.take(2 * count)?;
        let mut units: Vec<u16> = units
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        while units.last() == Some(&0) {
            units.pop();
        }
        Ok(char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect())
    }

    pub fn u16_array(&mut self) -> Result<Vec<u16>, ProtocolError> {
        let n = self.u32()? as usize;
        if n > self.bytes.len() / 2 {
            return Err(protocol(format!(
                "array of {n} elements exceeds the dataset"
            )));
        }
        (0..n).map(|_| self.u16()).collect()
    }

    pub fn u32_array(&mut self) -> Result<Vec<u32>, ProtocolError> {
        let n = self.u32()? as usize;
        if n > self.bytes.len() / 4 {
            return Err(protocol(format!(
                "array of {n} elements exceeds the dataset"
            )));
        }
        (0..n).map(|_| self.u32()).collect()
    }
}

/// Little-endian dataset writer.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn string(&mut self, s: &str) -> Result<&mut Self, ProtocolError> {
        if s.is_empty() {
            self.0.push(0);
            return Ok(self);
        }
        let units: Vec<u16> = s.encode_utf16().chain([0]).collect();
        let count = u8::try_from(units.len()).map_err(|_| {
            protocol(format!(
                "string of {} UTF-16 units is too long for PTP",
                units.len()
            ))
        })?;
        self.0.push(count);
        for u in units {
            self.0.extend_from_slice(&u.to_le_bytes());
        }
        Ok(self)
    }

    pub fn u32_array(&mut self, values: &[u32]) -> &mut Self {
        self.u32(values.len() as u32);
        for &v in values {
            self.u32(v);
        }
        self
    }
}

/// The `GetDeviceInfo` dataset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
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

impl DeviceInfo {
    pub fn decode(bytes: &[u8]) -> Result<DeviceInfo, ProtocolError> {
        let mut r = Reader::new(bytes);
        Ok(DeviceInfo {
            standard_version: r.u16()?,
            vendor_extension_id: r.u32()?,
            vendor_extension_version: r.u16()?,
            vendor_extension_desc: r.string()?,
            functional_mode: r.u16()?,
            operations_supported: r.u16_array()?,
            events_supported: r.u16_array()?,
            device_properties_supported: r.u16_array()?,
            capture_formats: r.u16_array()?,
            playback_formats: r.u16_array()?,
            manufacturer: r.string()?,
            model: r.string()?,
            device_version: r.string()?,
            serial_number: r.string()?,
        })
    }
}

/// The `ObjectInfo` dataset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectInfo {
    pub storage_id: u32,
    pub format: u16,
    pub protection_status: u16,
    pub compressed_size: u32,
    pub thumb_format: u16,
    pub thumb_compressed_size: u32,
    pub thumb_pix_width: u32,
    pub thumb_pix_height: u32,
    pub image_pix_width: u32,
    pub image_pix_height: u32,
    pub image_bit_depth: u32,
    pub parent: u32,
    pub association_type: u16,
    pub association_desc: u32,
    pub sequence_number: u32,
    pub filename: String,
    pub capture_date: String,
    pub modification_date: String,
    pub keywords: String,
}

impl ObjectInfo {
    pub fn decode(bytes: &[u8]) -> Result<ObjectInfo, ProtocolError> {
        let mut r = Reader::new(bytes);
        Ok(ObjectInfo {
            storage_id: r.u32()?,
            format: r.u16()?,
            protection_status: r.u16()?,
            compressed_size: r.u32()?,
            thumb_format: r.u16()?,
            thumb_compressed_size: r.u32()?,
            thumb_pix_width: r.u32()?,
            thumb_pix_height: r.u32()?,
            image_pix_width: r.u32()?,
            image_pix_height: r.u32()?,
            image_bit_depth: r.u32()?,
            parent: r.u32()?,
            association_type: r.u16()?,
            association_desc: r.u32()?,
            sequence_number: r.u32()?,
            filename: r.string()?,
            capture_date: r.string()?,
            modification_date: r.string()?,
            keywords: r.string()?,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut w = Writer::default();
        w.u32(self.storage_id)
            .u16(self.format)
            .u16(self.protection_status)
            .u32(self.compressed_size)
            .u16(self.thumb_format)
            .u32(self.thumb_compressed_size)
            .u32(self.thumb_pix_width)
            .u32(self.thumb_pix_height)
            .u32(self.image_pix_width)
            .u32(self.image_pix_height)
            .u32(self.image_bit_depth)
            .u32(self.parent)
            .u16(self.association_type)
            .u32(self.association_desc)
            .u32(self.sequence_number);
        w.string(&self.filename)?
            .string(&self.capture_date)?
            .string(&self.modification_date)?
            .string(&self.keywords)?;
        Ok(w.0)
    }

    pub fn is_folder(&self) -> bool {
        self.format == format::ASSOCIATION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn command_container_layout() {
        assert_eq!(
            encode_command(op::GET_OBJECT_HANDLES, 7, &[0x0002_0001, 0, ROOT]),
            hex("18000000 0100 0710 07000000 01000200 00000000 ffffffff")
        );
        assert_eq!(
            encode_command(op::GET_STORAGE_IDS, 1, &[]),
            hex("0c000000 0100 0410 01000000")
        );
    }

    #[test]
    fn data_container_layout() {
        assert_eq!(
            encode_data(op::SEND_OBJECT, 9, b"ab").unwrap(),
            hex("0e000000 0200 0d10 09000000 6162")
        );
    }

    #[test]
    fn response_parse_and_reject() {
        let r = Response::parse(&hex(
            "18000000 0300 0120 05000000 01000200 09000001 2a000001",
        ))
        .unwrap();
        assert_eq!(
            r,
            Response {
                code: 0x2001,
                transaction: 5,
                params: vec![0x0002_0001, 0x0100_0009, 0x0100_002a]
            }
        );
        assert_eq!(
            r.encode(),
            hex("18000000 0300 0120 05000000 01000200 09000001 2a000001")
        );
        // A data container where a response belongs.
        assert!(Response::parse(&hex("0c000000 0200 0120 05000000")).is_err());
        // Length field disagrees with the bytes received.
        assert!(Response::parse(&hex("10000000 0300 0120 05000000")).is_err());
        // Length below the header size.
        assert!(Header::parse(&hex("08000000 0300 0120 05000000")).is_err());
    }

    #[test]
    fn strings_are_counted_ucs2_with_nul() {
        let mut w = Writer::default();
        w.string("Ab")
            .unwrap()
            .string("")
            .unwrap()
            .string("ქ")
            .unwrap();
        assert_eq!(w.0, hex("03 4100 6200 0000  00  02 e510 0000"));
        let mut r = Reader::new(&w.0);
        assert_eq!(r.string().unwrap(), "Ab");
        assert_eq!(r.string().unwrap(), "");
        assert_eq!(r.string().unwrap(), "ქ");
        assert!(Reader::new(&hex("03 4100 6200")).string().is_err());
        assert!(Writer::default().string(&"x".repeat(255)).is_err());
    }

    #[test]
    fn object_info_bytes() {
        let info = ObjectInfo {
            storage_id: 0x0002_0001,
            format: format::UNDEFINED,
            compressed_size: 33_048,
            parent: 0x0100_0009,
            filename: "a.tmp".into(),
            ..ObjectInfo::default()
        };
        let bytes = hex(
            "01000200 0030 0000 18810000 0000 00000000 00000000 00000000 00000000 00000000 00000000
             09000001 0000 00000000 00000000
             06 6100 2e00 7400 6d00 7000 0000  00 00 00",
        );
        assert_eq!(info.encode().unwrap(), bytes);
        assert_eq!(ObjectInfo::decode(&bytes).unwrap(), info);
        assert!(ObjectInfo::decode(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn device_info_bytes() {
        let bytes = hex("6400 06000000 6400 02 4d00 0000 0000
             03000000 0110 0910 0c10  00000000  00000000  00000000  01000000 0030
             02 4700 0000  03 5600 3200 0000  02 3100 0000  00");
        let info = DeviceInfo::decode(&bytes).unwrap();
        assert_eq!(info.standard_version, 100);
        assert_eq!(info.vendor_extension_id, 6);
        assert_eq!(info.vendor_extension_desc, "M");
        assert_eq!(info.operations_supported, vec![0x1001, 0x1009, 0x100C]);
        assert_eq!(info.playback_formats, vec![0x3000]);
        assert_eq!(
            (info.manufacturer.as_str(), info.model.as_str()),
            ("G", "V2")
        );
        assert_eq!(info.device_version, "1");
        assert_eq!(info.serial_number, "");
        // An array count larger than the dataset is rejected rather than allocated.
        assert!(DeviceInfo::decode(&hex("6400 06000000 6400 00 0000 ffffff7f")).is_err());
    }
}
