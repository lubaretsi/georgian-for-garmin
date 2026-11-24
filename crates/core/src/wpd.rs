//! Windows backend: the watch through Windows Portable Devices (WPD) and Windows' built-in MTP
//! driver, with no driver replacement.
//!
//! WPD names an MTP object `o` + its handle in hex without leading zeros (`o1000009`) and a
//! storage `s` + its storage ID (`s20001`); WPD answers for an ID it never listed, which is
//! how the handle probe in [`crate::extdata`] reaches an unlisted `ExtData`. Files are
//! created, read and deleted with the standard WPD calls, so the driver's own listing stays
//! current. MTP has read-only queries WPD has no call for, `GetDeviceInfo`, the storage-wide
//! `GetObjectHandles` and an object's ProtectionStatus; those go through WPD's MTP extension
//! commands.
//!
//! This module holds the parts that are plain data handling, tested on every host; the COM
//! calls are in `device`, built on Windows only.

#[cfg(windows)]
mod device;

#[cfg(windows)]
pub use device::{WpdMtp, connect, garmin_devices};

use crate::GARMIN_VENDOR_ID;
use crate::error::{Error, ErrorCode};
use crate::mtp::MtpError;
use crate::ptp::{ObjectInfo, response};

/// WPD's object ID for MTP object `handle`.
pub fn object_id(handle: u32) -> String {
    format!("o{handle:X}")
}

/// WPD's object ID for MTP storage `storage`.
pub fn storage_object_id(storage: u32) -> String {
    format!("s{storage:X}")
}

/// The MTP handle in a WPD object ID (`o` + 1 to 8 hex digits), if it is one.
pub fn parse_object_id(id: &str) -> Option<u32> {
    parse_hex_id(id, "o")
}

/// The MTP storage ID in a WPD storage object ID (`s` + 1 to 8 hex digits), if it is one.
pub fn parse_storage_id(id: &str) -> Option<u32> {
    parse_hex_id(id, "s")
}

fn parse_hex_id(id: &str, prefix: &str) -> Option<u32> {
    let hex = id.strip_prefix(prefix)?;
    if hex.is_empty() || hex.len() > 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// The MTP parent handle of an object whose WPD parent is `id`: 0 at a storage's root, as in
/// MTP's ObjectInfo.
pub fn parent_handle(id: &str) -> Option<u32> {
    parse_object_id(id).or_else(|| parse_storage_id(id).map(|_| 0))
}

/// Whether an object is read-only, from MTP's ObjectInfo dataset `data` (as `GetObjectInfo`
/// returns it through WPD's MTP extension command): its ProtectionStatus is anything but
/// 0x0000 (No Protection). `None` when `data` isn't a readable ObjectInfo, so callers report
/// the protection as unknown rather than guess.
///
/// WPD has no documented property for ObjectProtectionStatus: its object properties
/// (<https://learn.microsoft.com/en-us/windows/win32/wpd_sdk/object-properties>) offer
/// `WPD_OBJECT_CAN_DELETE` and `WPD_OBJECT_ISSYSTEM` without saying how the MTP driver sets
/// them, so the backend asks the watch for the ObjectInfo itself.
pub fn read_only_from_object_info(data: &[u8]) -> Option<bool> {
    ObjectInfo::decode(data)
        .ok()
        .map(|info| info.protection_status != 0)
}

/// WPD format GUIDs carry the MTP format code in their first 16 bits:
/// `{XXXX0000-AE6C-4804-98BA-C57B46965FE7}`.
const FORMAT_GUID_BASE: u128 = 0x0000_0000_ae6c_4804_98ba_c57b_4696_5fe7;
const FORMAT_GUID_MASK: u128 = (1 << 112) - 1;

/// The WPD format GUID, as a `u128`, for MTP format `code`.
pub fn format_guid(code: u16) -> u128 {
    (u128::from(code) << 112) | FORMAT_GUID_BASE
}

/// The MTP format code in a WPD format GUID, if it is one.
pub fn format_code(guid: u128) -> Option<u16> {
    (guid & FORMAT_GUID_MASK == FORMAT_GUID_BASE).then_some((guid >> 112) as u16)
}

/// Whether a WPD device ID (a PnP device interface path such as
/// `\\?\usb#vid_091e&pid_4e77#…`) belongs to a Garmin USB device.
pub fn is_garmin_device_id(id: &str) -> bool {
    let vid = format!("vid_{GARMIN_VENDOR_ID:04x}");
    let id = id.as_bytes();
    id.windows(vid.len()).enumerate().any(|(i, w)| {
        w.eq_ignore_ascii_case(vid.as_bytes())
            && id
                .get(i + vid.len())
                .is_none_or(|next| !next.is_ascii_hexdigit())
    })
}

/// The USB product ID in a WPD device ID (`…#vid_091e&pid_4e77#…`), if it names one. The rest of
/// the ID, which holds the device's serial number, is never kept.
pub fn product_id_from_device_id(id: &str) -> Option<u16> {
    let lower = id.to_ascii_lowercase();
    let at = lower.find("&pid_")? + "&pid_".len();
    let digits = lower.get(at..at + 4)?;
    let after = lower.as_bytes().get(at + 4);
    if after.is_some_and(u8::is_ascii_hexdigit) {
        return None;
    }
    u16::from_str_radix(digits, 16).ok()
}

/// A Garmin device Windows Portable Devices lists, without its device ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedDevice {
    pub product_id: Option<u16>,
    /// WPD's description of the device (its driver's name for it, such as its model).
    pub description: Option<String>,
}

/// What a failed WPD call's HRESULT means for the installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The object does not exist.
    NotFound,
    /// Another program holds the device.
    Busy,
    /// The device was unplugged or switched off.
    Gone,
    /// Windows refused access, e.g. by a policy on portable devices.
    Denied,
    Other,
}

const fn win32(code: u32) -> u32 {
    0x8007_0000 | code
}

pub fn classify(hresult: u32) -> Failure {
    const FILE_NOT_FOUND: u32 = win32(2);
    const PATH_NOT_FOUND: u32 = win32(3);
    const ACCESS_DENIED: u32 = win32(5);
    const NOT_READY: u32 = win32(21);
    const SHARING_VIOLATION: u32 = win32(32);
    const LOCK_VIOLATION: u32 = win32(33);
    const BUSY: u32 = win32(170);
    const PIPE_BUSY: u32 = win32(231);
    const NO_SUCH_DEVICE: u32 = win32(433);
    const NOT_FOUND: u32 = win32(1168);
    const DEVICE_NOT_CONNECTED: u32 = win32(1167);
    const DEVICE_REMOVED: u32 = win32(1617);
    const WPD_DEVICE_ALREADY_OPENED: u32 = 0x802A_0001;
    match hresult {
        FILE_NOT_FOUND | PATH_NOT_FOUND | NOT_FOUND => Failure::NotFound,
        SHARING_VIOLATION | LOCK_VIOLATION | BUSY | PIPE_BUSY | WPD_DEVICE_ALREADY_OPENED => {
            Failure::Busy
        }
        NOT_READY | NO_SUCH_DEVICE | DEVICE_NOT_CONNECTED | DEVICE_REMOVED => Failure::Gone,
        ACCESS_DENIED => Failure::Denied,
        _ => Failure::Other,
    }
}

/// The error code for a WPD failure during a session.
pub fn error_code(hresult: u32) -> ErrorCode {
    match classify(hresult) {
        Failure::Busy => ErrorCode::DeviceBusy,
        Failure::Gone => ErrorCode::NoDevice,
        Failure::NotFound | Failure::Denied | Failure::Other => ErrorCode::DeviceError,
    }
}

/// A failed WPD call standing in for MTP `operation`. A missing object reads as MTP's
/// InvalidObjectHandle, as the USB backend reports it.
pub fn mtp_error(operation: u16, action: &'static str, hresult: u32, message: String) -> MtpError {
    match classify(hresult) {
        Failure::NotFound => MtpError::Response {
            operation,
            code: response::INVALID_OBJECT_HANDLE,
        },
        _ => MtpError::Wpd {
            action,
            hresult,
            message,
        },
    }
}

/// The error for failing to list or open the watch.
pub fn open_error(context: &str, hresult: u32, message: &str) -> Error {
    let detail = if message.is_empty() {
        format!("HRESULT {hresult:#010x}")
    } else {
        format!("{message} (HRESULT {hresult:#010x})")
    };
    match classify(hresult) {
        Failure::Busy => {
            // Windows doesn't say who holds a device; only whether Garmin Express runs.
            #[cfg(windows)]
            let holder = crate::busy::windows();
            #[cfg(not(windows))]
            let holder = crate::busy::Holder::unknown();
            Error::busy(crate::busy::Busy {
                holder,
                ..crate::busy::Busy::unknown()
            })
        }
        Failure::Gone | Failure::NotFound => {
            Error::new(ErrorCode::NoDevice, "The watch disconnected.")
        }
        Failure::Denied => Error::new(
            ErrorCode::DeviceError,
            format!("{context}: {detail}. A policy may block access to portable devices."),
        ),
        Failure::Other => Error::new(ErrorCode::DeviceError, format!("{context}: {detail}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ptp::format;

    #[test]
    fn protection_comes_from_the_object_info_dataset() {
        for (status, read_only) in [(0x0000, false), (0x0001, true), (0x8002, true)] {
            let info = ObjectInfo {
                protection_status: status,
                filename: "006-D9971-09.bin".into(),
                ..ObjectInfo::default()
            };
            let data = info.encode().unwrap();
            assert_eq!(read_only_from_object_info(&data), Some(read_only));
        }
        assert_eq!(read_only_from_object_info(&[1, 2, 3]), None);
    }

    #[test]
    fn the_product_id_comes_from_the_device_id() {
        let id = r"\\?\usb#vid_091e&pid_4e77#0000c1234567#{6ac27878-a6fa-4155-ba85-f98f491d4f33}";
        assert_eq!(product_id_from_device_id(id), Some(0x4E77));
        assert_eq!(
            product_id_from_device_id(r"\\?\USB#VID_091E&PID_4F1B#x"),
            Some(0x4F1B)
        );
        assert_eq!(
            product_id_from_device_id(r"\\?\usb#vid_091e&pid_4e7#x"),
            None
        );
        assert_eq!(
            product_id_from_device_id(r"\\?\usb#vid_091e&pid_4e77a#x"),
            None
        );
        assert_eq!(product_id_from_device_id(r"\\?\swd#wpdbusenum#x"), None);
    }

    #[test]
    fn object_ids_are_hex_handles_without_leading_zeros() {
        assert_eq!(object_id(0x0100_0009), "o1000009");
        assert_eq!(object_id(0x0100_03B9), "o10003B9");
        assert_eq!(object_id(0), "o0");
        assert_eq!(object_id(u32::MAX), "oFFFFFFFF");
        assert_eq!(storage_object_id(0x0002_0001), "s20001");
        for handle in [0, 1, 0x0100_0000, 0x0100_03B9, u32::MAX] {
            assert_eq!(parse_object_id(&object_id(handle)), Some(handle));
        }
        assert_eq!(parse_storage_id("s20001"), Some(0x0002_0001));
    }

    #[test]
    fn object_id_parsing_accepts_only_prefixed_hex() {
        assert_eq!(parse_object_id("o10003b9"), Some(0x0100_03B9));
        assert_eq!(parse_object_id("o00000001"), Some(1));
        for id in [
            "",
            "o",
            "o123456789",
            "o-1",
            "o+1",
            "o1g",
            "O1",
            "s20001",
            "DEVICE",
            " o1",
            "o1 ",
        ] {
            assert_eq!(parse_object_id(id), None, "{id:?}");
        }
        assert_eq!(parse_storage_id("o1000000"), None);
    }

    #[test]
    fn root_objects_have_parent_zero() {
        assert_eq!(parent_handle("o1000000"), Some(0x0100_0000));
        assert_eq!(parent_handle("s20001"), Some(0));
        assert_eq!(parent_handle("DEVICE"), None);
    }

    #[test]
    fn format_guids_carry_the_mtp_format_code() {
        // WPD_OBJECT_FORMAT_UNSPECIFIED and WPD_OBJECT_FORMAT_PROPERTIES_ONLY (association).
        assert_eq!(
            format_guid(format::UNDEFINED),
            0x3000_0000_ae6c_4804_98ba_c57b_4696_5fe7
        );
        assert_eq!(
            format_code(0x3001_0000_ae6c_4804_98ba_c57b_4696_5fe7),
            Some(format::ASSOCIATION)
        );
        assert_eq!(format_code(format_guid(0xB802)), Some(0xB802));
        // WPD_CONTENT_TYPE_FOLDER is not a format GUID; nor is one with low data1 bits set.
        assert_eq!(format_code(0x27e2_e392_a111_48e0_ab0c_e177_05a0_5f85), None);
        assert_eq!(format_code(0x3001_0001_ae6c_4804_98ba_c57b_4696_5fe7), None);
    }

    #[test]
    fn garmin_devices_are_found_by_vendor_id() {
        for id in [
            r"\\?\usb#vid_091e&pid_4e77#0000c8a5e2f8#{6ac27878-a6fa-4155-ba85-f98f491d4f33}",
            r"\\?\USB#VID_091E&PID_4E77#0000C8A5E2F8#{6AC27878-A6FA-4155-BA85-F98F491D4F33}",
            r"\\?\usb#vid_091e",
        ] {
            assert!(is_garmin_device_id(id), "{id}");
        }
        for id in [
            r"\\?\usb#vid_045e&pid_091e#serial#{6ac27878-a6fa-4155-ba85-f98f491d4f33}",
            r"\\?\usb#vid_091e1&pid_0001#x",
            r"\\?\swd#wpdbusenum#_??_usbstor#disk&ven_garmin&prod_venu_2#{6ac27878}",
            "",
        ] {
            assert!(!is_garmin_device_id(id), "{id}");
        }
    }

    #[test]
    fn hresults_map_onto_error_codes() {
        for (hresult, failure, code) in [
            (0x8007_0490, Failure::NotFound, ErrorCode::DeviceError),
            (0x8007_0002, Failure::NotFound, ErrorCode::DeviceError),
            (0x8007_0020, Failure::Busy, ErrorCode::DeviceBusy),
            (0x8007_00AA, Failure::Busy, ErrorCode::DeviceBusy),
            (0x802A_0001, Failure::Busy, ErrorCode::DeviceBusy),
            (0x8007_048F, Failure::Gone, ErrorCode::NoDevice),
            (0x8007_01B1, Failure::Gone, ErrorCode::NoDevice),
            (0x8007_0651, Failure::Gone, ErrorCode::NoDevice),
            (0x8007_0015, Failure::Gone, ErrorCode::NoDevice),
            (0x8007_0005, Failure::Denied, ErrorCode::DeviceError),
            (0x8007_001F, Failure::Other, ErrorCode::DeviceError),
            (0x802A_0006, Failure::Other, ErrorCode::DeviceError),
            (0x8000_4005, Failure::Other, ErrorCode::DeviceError),
        ] {
            assert_eq!(classify(hresult), failure, "{hresult:#x}");
            assert_eq!(error_code(hresult), code, "{hresult:#x}");
            let mtp = MtpError::Wpd {
                action: "WPD call",
                hresult,
                message: String::new(),
            };
            assert_eq!(Error::from(mtp).code, code, "{hresult:#x}");
        }
    }

    #[test]
    fn a_missing_object_reads_as_invalid_object_handle() {
        let err = mtp_error(
            crate::ptp::op::GET_OBJECT_INFO,
            "WPD GetValues",
            0x8007_0490,
            "Element not found.".into(),
        );
        assert_eq!(
            err,
            MtpError::Response {
                operation: crate::ptp::op::GET_OBJECT_INFO,
                code: response::INVALID_OBJECT_HANDLE
            }
        );
        let err = mtp_error(
            crate::ptp::op::GET_OBJECT,
            "WPD resource read",
            0x8007_001F,
            "A device attached to the system is not functioning.".into(),
        );
        assert_eq!(
            err.to_string(),
            "WPD resource read failed: A device attached to the system is not functioning. \
             (HRESULT 0x8007001f)"
        );
        assert_eq!(
            Error::from(err).message,
            "WPD resource read failed: A device attached to the system is not functioning. \
             (HRESULT 0x8007001f)"
        );
    }

    #[test]
    fn open_failures_name_the_cause() {
        assert_eq!(
            open_error("cannot open the watch", 0x8007_0020, "").code,
            ErrorCode::DeviceBusy
        );
        assert_eq!(
            open_error("cannot open the watch", 0x8007_048F, "").code,
            ErrorCode::NoDevice
        );
        let denied = open_error("cannot open the watch", 0x8007_0005, "Access is denied.");
        assert_eq!(denied.code, ErrorCode::DeviceError);
        assert_eq!(
            denied.message,
            "cannot open the watch: Access is denied. (HRESULT 0x80070005). A policy may block \
             access to portable devices."
        );
        assert_eq!(
            open_error("cannot list portable devices", 0x8000_4005, "").message,
            "cannot list portable devices: HRESULT 0x80004005"
        );
    }
}
