//! Stages Georgian-extended Garmin fonts on a watch over USB MTP.
//!
//! [`ops::call`] runs an operation from JSON input; [`connect`] opens the attached watch.

pub mod error;
pub mod extdata;
pub mod fonts;
pub mod garmin_device;
pub mod mtp;
pub mod ops;
pub mod ptp;
pub mod trace;
#[cfg(not(windows))]
pub mod usb;

pub use error::{Error, ErrorCode};

/// Opens an MTP session with the single attached Garmin watch.
#[cfg(not(windows))]
pub fn connect() -> Result<Box<dyn mtp::Mtp>, Error> {
    Ok(Box::new(usb::connect()?))
}

/// Opens an MTP session with the single attached Garmin watch.
#[cfg(windows)]
pub fn connect() -> Result<Box<dyn mtp::Mtp>, Error> {
    Err(Error::new(
        ErrorCode::UnsupportedPlatform,
        "Windows is not supported yet; it needs a Windows Portable Devices (WPD) backend.",
    ))
}
