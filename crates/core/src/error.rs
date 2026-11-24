use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::busy::Busy;
use crate::garmin_mode::GarminMode;
use crate::mtp::MtpError;

/// Stable, machine-readable error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The input does not match the operation's schema.
    InvalidInput,
    /// No Garmin watch in MTP mode is connected.
    NoDevice,
    /// No Garmin watch in MTP mode is connected, but one in Garmin mode is (USB `091e:0003`);
    /// the error's `garmin_mode` says whether `switch_to_mtp` can switch it here.
    GarminMode,
    /// More than one Garmin MTP device is connected.
    MultipleDevices,
    /// Another program holds the watch; the error's `busy` names it when the system tells.
    DeviceBusy,
    /// This user may not open the watch's USB device (on Linux: no udev rule).
    PermissionDenied,
    /// `switch_to_mtp`: the watch reports that it is locked, so it isn't switched.
    DeviceLocked,
    /// The watch lists none of this build's font packs or not a requested part, lists a
    /// TrueType part it doesn't let be read at another version or file name than this
    /// operation accepts, or lacks a needed MTP operation; for `switch_to_mtp`, the watch doesn't
    /// list the switch to MTP.
    UnsupportedDevice,
    /// This build does not bundle the requested font size.
    FontUnavailable,
    /// The watch has no single `GARMIN` folder or no readable `GarminDevice.xml`.
    GarminFolderNotFound,
    /// The hidden `GARMIN/ExtData` staging folder could not be located.
    ExtdataNotFound,
    /// A file under a target's name is neither Garmin's original nor one of this program's
    /// Georgian builds (by content), or several files share the name; nothing was written.
    Conflict,
    /// Install: a TrueType file the watch doesn't let be read is declared at a version other
    /// than Garmin's (below 1.00, as for this program's builds before 0.3.0), so install can't
    /// identify it; restoring the original font first fixes it. Nothing was written.
    RestoreFirst,
    /// A written file read back differently and was deleted (a replaced `.bin` was written
    /// back).
    VerificationFailed,
    /// An MTP operation or USB transfer failed.
    DeviceError,
    /// A bug: the operation produced output that violates its own schema.
    Internal,
    /// The caller cancelled the call (`notifications/cancelled` in `rpc` and `mcp`). Changes a
    /// write had already made were undone where possible; `rollback` says whether all were.
    Cancelled,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidInput => "invalid_input",
            ErrorCode::NoDevice => "no_device",
            ErrorCode::GarminMode => "garmin_mode",
            ErrorCode::MultipleDevices => "multiple_devices",
            ErrorCode::DeviceBusy => "device_busy",
            ErrorCode::PermissionDenied => "permission_denied",
            ErrorCode::DeviceLocked => "device_locked",
            ErrorCode::UnsupportedDevice => "unsupported_device",
            ErrorCode::FontUnavailable => "font_unavailable",
            ErrorCode::GarminFolderNotFound => "garmin_folder_not_found",
            ErrorCode::ExtdataNotFound => "extdata_not_found",
            ErrorCode::Conflict => "conflict",
            ErrorCode::RestoreFirst => "restore_first",
            ErrorCode::VerificationFailed => "verification_failed",
            ErrorCode::DeviceError => "device_error",
            ErrorCode::Internal => "internal",
            ErrorCode::Cancelled => "cancelled",
        }
    }
}

/// What a failed write did to undo this call's changes: every file it tried to delete or put
/// back, and whether all of that worked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rollback {
    pub status: RollbackStatus,
    pub files: Vec<RollbackFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RollbackStatus {
    /// Every change was undone.
    Complete,
    /// Some file couldn't be deleted or put back: the watch holds what `files` says.
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RollbackAction {
    /// A file this call created, deleted.
    Delete,
    /// A file this call replaced, written back from the bytes read before.
    Restore,
}

/// One file of a [`Rollback`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RollbackFile {
    /// "GARMIN/ExtData/<name>".
    pub path: String,
    pub action: RollbackAction,
    /// Whether it worked; false means the file may still be there (delete) or missing or
    /// different (restore).
    pub done: bool,
    /// What happened, in words.
    pub result: String,
}

/// An operation failure: a code and a message for people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    /// `device_busy`: who holds the watch, as far as the system tells.
    pub busy: Option<Box<Busy>>,
    /// `garmin_mode`: whether the watch can be switched to MTP here.
    pub garmin_mode: Option<Box<GarminMode>>,
    /// A failed write: how undoing this call's changes went.
    pub rollback: Option<Box<Rollback>>,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Error {
            code,
            message: message.into(),
            busy: None,
            garmin_mode: None,
            rollback: None,
        }
    }

    /// `device_busy` with what is known about the holder, and the message for it. A product name
    /// that looks like an identifier is dropped here, so no consumer sees it.
    pub fn busy(busy: Busy) -> Self {
        Error::busy_on(busy, crate::busy::System::current())
    }

    /// [`Error::busy`] with the message written for `system`.
    pub fn busy_on(busy: Busy, system: crate::busy::System) -> Self {
        let busy = busy.sanitized();
        let message = crate::busy::message_on(&busy, system);
        Error {
            busy: Some(Box::new(busy)),
            ..Error::new(ErrorCode::DeviceBusy, message)
        }
    }

    /// `garmin_mode` with whether the watch can be switched here, and the message for it.
    pub fn garmin_mode(mode: GarminMode) -> Self {
        Error {
            garmin_mode: Some(Box::new(mode.clone())),
            ..Error::new(ErrorCode::GarminMode, mode.message())
        }
    }

    /// This error with `files` added to its rollback, whose status follows from all of them.
    pub fn with_rollback(mut self, files: impl IntoIterator<Item = RollbackFile>) -> Self {
        let mut all = self.rollback.take().map(|r| r.files).unwrap_or_default();
        all.extend(files);
        if !all.is_empty() {
            let status = if all.iter().all(|f| f.done) {
                RollbackStatus::Complete
            } else {
                RollbackStatus::Incomplete
            };
            self.rollback = Some(Box::new(Rollback { status, files: all }));
        }
        self
    }

    /// Whether undoing this call's changes left something behind.
    pub fn rollback_incomplete(&self) -> bool {
        self.rollback
            .as_ref()
            .is_some_and(|r| r.status == RollbackStatus::Incomplete)
    }

    /// Process exit code: 2 for invalid input, 1 otherwise.
    pub fn exit_code(&self) -> i32 {
        if self.code == ErrorCode::InvalidInput {
            2
        } else {
            1
        }
    }

    /// `{"error": {"code": …, "message": …}}`, with `busy` for `device_busy`, `garmin_mode` for
    /// `garmin_mode` and `rollback` after a failed write.
    pub fn to_json(&self) -> serde_json::Value {
        let mut error = serde_json::json!({"code": self.code.as_str(), "message": self.message});
        if let Some(busy) = &self.busy {
            error["busy"] = serde_json::to_value(busy).expect("Busy serializes");
        }
        if let Some(mode) = &self.garmin_mode {
            error["garmin_mode"] = serde_json::to_value(mode).expect("GarminMode serializes");
        }
        if let Some(rollback) = &self.rollback {
            error["rollback"] = serde_json::to_value(rollback).expect("Rollback serializes");
        }
        serde_json::json!({ "error": error })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<MtpError> for Error {
    fn from(e: MtpError) -> Self {
        let code = match &e {
            MtpError::Wpd { hresult, .. } => crate::wpd::error_code(*hresult),
            MtpError::Unsupported(_) => ErrorCode::UnsupportedDevice,
            _ => ErrorCode::DeviceError,
        };
        Error::new(code, e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::busy::{Holder, UsbInterface};

    #[test]
    fn busy_errors_carry_the_holder_in_json() {
        assert_eq!(
            Error::new(ErrorCode::NoDevice, "none").to_json(),
            json!({"error": {"code": "no_device", "message": "none"}})
        );
        let error = Error::busy(Busy {
            holder: Holder::named("Garmin Express", Some(812), true),
            product_id: Some(0x4E77),
            product_name: None,
            interface: Some(UsbInterface {
                class: 255,
                subclass: 255,
                protocol: 0,
            }),
        });
        assert_eq!(error.code, ErrorCode::DeviceBusy);
        let value = error.to_json();
        assert_eq!(value["error"]["code"], "device_busy");
        assert_eq!(value["error"]["message"], error.message);
        assert_eq!(
            value["error"]["busy"],
            json!({
                "holder": {"kind": "garmin_express", "name": "Garmin Express", "pid": 812, "confirmed": true},
                "product_id": 20087,
                "product_name": null,
                "interface": {"class": 255, "subclass": 255, "protocol": 0}
            })
        );
    }
}
