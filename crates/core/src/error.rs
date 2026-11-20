use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;

use crate::mtp::MtpError;

/// Stable, machine-readable error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The input does not match the operation's schema.
    InvalidInput,
    /// No Garmin watch in MTP mode is connected.
    NoDevice,
    /// More than one Garmin MTP device is connected.
    MultipleDevices,
    /// Another program (usually Garmin Express) holds the watch's USB interface.
    DeviceBusy,
    /// This operating system has no transport backend yet.
    UnsupportedPlatform,
    /// The watch does not declare the requested font part, or lacks a needed MTP operation.
    UnsupportedDevice,
    /// This build does not bundle the requested font size.
    FontUnavailable,
    /// The watch has no single `GARMIN` folder or no readable `GarminDevice.xml`.
    GarminFolderNotFound,
    /// The hidden `GARMIN/ExtData` staging folder could not be located.
    ExtdataNotFound,
    /// A different file is already staged under the target name; nothing was written.
    Conflict,
    /// The staged file read back differently and was deleted.
    VerificationFailed,
    /// An MTP operation or USB transfer failed.
    DeviceError,
    /// A bug: the operation produced output that violates its own schema.
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidInput => "invalid_input",
            ErrorCode::NoDevice => "no_device",
            ErrorCode::MultipleDevices => "multiple_devices",
            ErrorCode::DeviceBusy => "device_busy",
            ErrorCode::UnsupportedPlatform => "unsupported_platform",
            ErrorCode::UnsupportedDevice => "unsupported_device",
            ErrorCode::FontUnavailable => "font_unavailable",
            ErrorCode::GarminFolderNotFound => "garmin_folder_not_found",
            ErrorCode::ExtdataNotFound => "extdata_not_found",
            ErrorCode::Conflict => "conflict",
            ErrorCode::VerificationFailed => "verification_failed",
            ErrorCode::DeviceError => "device_error",
            ErrorCode::Internal => "internal",
        }
    }
}

/// An operation failure: a code and a message for people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Error {
            code,
            message: message.into(),
        }
    }

    /// Process exit code: 2 for invalid input, 1 otherwise.
    pub fn exit_code(&self) -> i32 {
        if self.code == ErrorCode::InvalidInput {
            2
        } else {
            1
        }
    }

    /// `{"error": {"code": …, "message": …}}`
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"error": {"code": self.code.as_str(), "message": self.message}})
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
        Error::new(ErrorCode::DeviceError, e.to_string())
    }
}
