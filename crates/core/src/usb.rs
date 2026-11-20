//! USB backend: a PTP session over the watch's MTP bulk endpoints, through `nusb`.

use std::time::Duration;

use nusb::descriptors::{ConfigurationDescriptor, TransferType};
use nusb::transfer::{Buffer, Bulk, Direction, In, Out, TransferError};
use nusb::{Endpoint, ErrorKind, Interface, MaybeFuture};

use crate::error::{Error, ErrorCode};
use crate::mtp::{BulkPipe, Mtp, MtpError, NewObject, Session};
use crate::ptp::{DeviceInfo, ObjectInfo};

pub const GARMIN_VENDOR_ID: u16 = 0x091E;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(15);

/// Interface classes that carry MTP: still-image class (6/1/1) as most devices declare it, and
/// the vendor-specific (0xFF/0xFF/0x00) interface Garmin watches use.
fn is_mtp_class(class: u8, subclass: u8, protocol: u8) -> bool {
    matches!(
        (class, subclass, protocol),
        (0x06, 0x01, 0x01) | (0xFF, 0xFF, 0x00)
    )
}

struct MtpEndpoints {
    interface: u8,
    bulk_in: u8,
    bulk_out: u8,
}

/// The interface with MTP's endpoint set: bulk IN, bulk OUT and interrupt IN.
fn find_mtp_endpoints(config: &ConfigurationDescriptor<'_>) -> Option<MtpEndpoints> {
    config.interfaces().find_map(|interface| {
        let alt = interface.first_alt_setting();
        if !is_mtp_class(alt.class(), alt.subclass(), alt.protocol()) {
            return None;
        }
        let find = |kind: TransferType, dir: Direction| {
            alt.endpoints()
                .find(|e| e.transfer_type() == kind && e.direction() == dir)
                .map(|e| e.address())
        };
        find(TransferType::Interrupt, Direction::In)?;
        Some(MtpEndpoints {
            interface: interface.interface_number(),
            bulk_in: find(TransferType::Bulk, Direction::In)?,
            bulk_out: find(TransferType::Bulk, Direction::Out)?,
        })
    })
}

fn device_error(context: &str, e: nusb::Error) -> Error {
    match e.kind() {
        ErrorKind::Busy => Error::new(
            ErrorCode::DeviceBusy,
            "Another program is using the watch's USB connection, usually Garmin Express. \
             Quit Garmin Express (including its background helper) and try again.",
        ),
        ErrorKind::Disconnected => Error::new(ErrorCode::NoDevice, "The watch disconnected."),
        ErrorKind::PermissionDenied => Error::new(
            ErrorCode::DeviceError,
            format!("{context}: permission denied; this user may not access the USB device ({e})"),
        ),
        _ => Error::new(ErrorCode::DeviceError, format!("{context}: {e}")),
    }
}

struct NusbPipe {
    bulk_in: Endpoint<Bulk, In>,
    bulk_out: Endpoint<Bulk, Out>,
    _interface: Interface,
}

fn transfer_error(e: TransferError) -> MtpError {
    match e {
        TransferError::Cancelled => MtpError::Transport("timed out".into()),
        other => MtpError::Transport(other.to_string()),
    }
}

impl BulkPipe for NusbPipe {
    fn max_packet_size(&self) -> usize {
        self.bulk_in.max_packet_size()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), MtpError> {
        let buf = if data.is_empty() {
            Buffer::new(0)
        } else {
            Buffer::from(data.to_vec())
        };
        let done = self.bulk_out.transfer_blocking(buf, TRANSFER_TIMEOUT);
        done.status.map_err(transfer_error)?;
        if done.actual_len != data.len() {
            return Err(MtpError::Transport(format!(
                "sent {} of {} bytes",
                done.actual_len,
                data.len()
            )));
        }
        Ok(())
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>, MtpError> {
        let done = self
            .bulk_in
            .transfer_blocking(Buffer::new(len), TRANSFER_TIMEOUT);
        done.status.map_err(transfer_error)?;
        Ok(done.buffer.into_vec())
    }
}

/// An open MTP session with a connected Garmin watch.
pub struct UsbMtp(Session<NusbPipe>);

/// Finds the single Garmin device with an MTP interface, claims it and opens a session.
pub fn connect() -> Result<UsbMtp, Error> {
    let candidates: Vec<_> = nusb::list_devices()
        .wait()
        .map_err(|e| device_error("cannot list USB devices", e))?
        .filter(|d| {
            d.vendor_id() == GARMIN_VENDOR_ID
                && d.interfaces()
                    .any(|i| is_mtp_class(i.class(), i.subclass(), i.protocol()))
        })
        .collect();
    let info = match candidates.as_slice() {
        [] => {
            return Err(Error::new(
                ErrorCode::NoDevice,
                "No Garmin watch in USB file-transfer (MTP) mode was found. Connect it with a data cable.",
            ));
        }
        [one] => one,
        _ => {
            return Err(Error::new(
                ErrorCode::MultipleDevices,
                format!(
                    "{} Garmin devices are connected; connect only the watch to update.",
                    candidates.len()
                ),
            ));
        }
    };
    let device = info
        .open()
        .wait()
        .map_err(|e| device_error("cannot open the watch", e))?;
    let config = device.active_configuration().map_err(|e| {
        Error::new(
            ErrorCode::DeviceError,
            format!("cannot read the watch's USB configuration: {e}"),
        )
    })?;
    let endpoints = find_mtp_endpoints(&config).ok_or_else(|| {
        Error::new(
            ErrorCode::UnsupportedDevice,
            "The Garmin device exposes no MTP bulk and interrupt endpoints.",
        )
    })?;
    let interface = device
        .claim_interface(endpoints.interface)
        .wait()
        .map_err(|e| device_error("cannot claim the watch's MTP interface", e))?;
    let pipe = NusbPipe {
        bulk_in: interface
            .endpoint::<Bulk, In>(endpoints.bulk_in)
            .map_err(|e| device_error("cannot open the bulk IN endpoint", e))?,
        bulk_out: interface
            .endpoint::<Bulk, Out>(endpoints.bulk_out)
            .map_err(|e| device_error("cannot open the bulk OUT endpoint", e))?,
        _interface: interface,
    };
    Ok(UsbMtp(Session::open(pipe)?))
}

impl Mtp for UsbMtp {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        self.0.device_info()
    }

    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError> {
        self.0.storage_ids()
    }

    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        self.0.object_handles(storage, parent)
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError> {
        self.0.object_info(handle)
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        self.0.get_object(handle)
    }

    fn send_object_info(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
    ) -> Result<NewObject, MtpError> {
        self.0.send_object_info(storage, parent, info)
    }

    fn send_object(&mut self, data: &[u8]) -> Result<(), MtpError> {
        self.0.send_object(data)
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        self.0.delete_object(handle)
    }
}
