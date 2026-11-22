//! Stages Georgian-extended Garmin fonts on a watch over USB MTP: directly over USB on macOS
//! and Linux, through Windows' built-in MTP driver on Windows.
//!
//! [`ops::call`] runs an operation from JSON input; [`connect`] opens the attached watch.
//! [`report`] builds the shareable watch report, [`drive`] reads watches that mount as a drive,
//! [`support`] derives a watch's status in the support list.

pub mod drive;
pub mod error;
pub mod extdata;
pub mod fonts;
pub mod garmin_device;
pub mod mtp;
pub mod ops;
pub mod ptp;
pub mod report;
pub mod support;
pub mod trace;
#[cfg(not(windows))]
pub mod usb;
pub mod wpd;

pub use error::{Error, ErrorCode};

/// USB vendor ID of Garmin devices.
pub const GARMIN_VENDOR_ID: u16 = 0x091E;

/// Opens an MTP session with the single attached Garmin watch.
#[cfg(not(windows))]
pub fn connect() -> Result<Box<dyn mtp::Mtp>, Error> {
    Ok(Box::new(usb::connect()?))
}

/// Opens an MTP session with the single attached Garmin watch.
#[cfg(windows)]
pub fn connect() -> Result<Box<dyn mtp::Mtp>, Error> {
    Ok(Box::new(wpd::connect()?))
}
