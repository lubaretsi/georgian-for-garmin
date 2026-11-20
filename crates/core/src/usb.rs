//! USB backend: a PTP session over the watch's MTP bulk endpoints, through `nusb`.

use std::time::{Duration, Instant};

use nusb::descriptors::{ConfigurationDescriptor, TransferType};
use nusb::transfer::{
    Buffer, Bulk, BulkOrInterrupt, Completion, ControlIn, ControlOut, ControlType, Direction,
    EndpointDirection, In, Interrupt, Out, Recipient, TransferError,
};
use nusb::{Endpoint, ErrorKind, Interface, MaybeFuture};

use crate::GARMIN_VENDOR_ID;
use crate::error::{Error, ErrorCode};
use crate::mtp::{
    CreateError, DeviceStatus, Link, Mtp, MtpError, NewObject, Session, TransportError,
};
use crate::ptp::{DeviceInfo, ObjectInfo};
use crate::trace;

const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
/// How long to wait for a timed-out transfer to come back after cancelling it.
const CANCEL_TIMEOUT: Duration = Duration::from_secs(2);
/// Draining queued IN data: stop after this long without data, or after this many reads.
const DRAIN_TIMEOUT: Duration = Duration::from_millis(300);
const DRAIN_READS: usize = 16;

/// Still-image class requests (PIMA 15740 USB transport).
const CANCEL_REQUEST: u8 = 0x64;
const DEVICE_RESET_REQUEST: u8 = 0x66;
const GET_DEVICE_STATUS: u8 = 0x67;
/// PTP event code carried by Cancel Request.
const CANCEL_TRANSACTION: u16 = 0x4001;

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
    interrupt_in: u8,
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
        Some(MtpEndpoints {
            interface: interface.interface_number(),
            bulk_in: find(TransferType::Bulk, Direction::In)?,
            bulk_out: find(TransferType::Bulk, Direction::Out)?,
            interrupt_in: find(TransferType::Interrupt, Direction::In)?,
        })
    })
}

/// What usually holds the watch's USB connection on this system, and how to release it.
#[cfg(target_os = "linux")]
const BUSY_HINT: &str = "Another program is using the watch's USB connection, usually the desktop's \
     file manager. On GNOME, eject the watch in Files (or `gio mount -u` its mtp:// address); on KDE, \
     run `kquitapp6 kiod6`. Then try again.";
#[cfg(not(target_os = "linux"))]
const BUSY_HINT: &str = "Another program is using the watch's USB connection, usually Garmin Express. \
     Quit Garmin Express (including its background helper) and try again.";

fn device_error(context: &str, e: nusb::Error) -> Error {
    match e.kind() {
        ErrorKind::Busy => Error::new(ErrorCode::DeviceBusy, BUSY_HINT),
        ErrorKind::Disconnected => Error::new(ErrorCode::NoDevice, "The watch disconnected."),
        ErrorKind::PermissionDenied => Error::new(
            ErrorCode::DeviceError,
            if cfg!(target_os = "linux") {
                format!(
                    "{context}: permission denied ({e}). Install the udev rule from \
                     packaging/linux/70-georgian-for-garmin.rules, then reconnect the watch."
                )
            } else {
                format!(
                    "{context}: permission denied; this user may not access the USB device ({e})"
                )
            },
        ),
        _ => Error::new(ErrorCode::DeviceError, format!("{context}: {e}")),
    }
}

struct NusbLink {
    interface: Interface,
    bulk_in: Endpoint<Bulk, In>,
    bulk_out: Endpoint<Bulk, Out>,
    interrupt_in: Endpoint<Interrupt, In>,
}

fn transport(e: TransferError) -> TransportError {
    match e {
        TransferError::Cancelled => TransportError::TimedOut,
        TransferError::Stall => TransportError::Stall,
        TransferError::Disconnected => TransportError::Disconnected,
        other => TransportError::Other(other.to_string()),
    }
}

/// One transfer with a bounded wait. Unlike `transfer_blocking`, a transfer the OS fails to
/// cancel is reported instead of waited on forever.
fn transfer<T: BulkOrInterrupt, D: EndpointDirection>(
    endpoint: &mut Endpoint<T, D>,
    buffer: Buffer,
    timeout: Duration,
) -> Result<Completion, TransportError> {
    while endpoint.pending() > 0 {
        // Left over from an earlier timeout whose cancellation had not finished.
        if endpoint.wait_next_complete(CANCEL_TIMEOUT).is_none() {
            return Err(TransportError::Other(
                "an earlier cancelled transfer is still pending".into(),
            ));
        }
    }
    endpoint.submit(buffer);
    if let Some(done) = endpoint.wait_next_complete(timeout) {
        return Ok(done);
    }
    endpoint.cancel_all();
    match endpoint.wait_next_complete(CANCEL_TIMEOUT) {
        Some(done) => {
            trace!(
                "  transfer cancelled after {:?} with {} bytes moved",
                timeout, done.actual_len
            );
            Err(TransportError::TimedOut)
        }
        None => Err(TransportError::Other(
            "timed out, and the transfer could not be cancelled".into(),
        )),
    }
}

/// Reads what `endpoint` has queued, until it has nothing within `DRAIN_TIMEOUT`.
fn drain_endpoint<T: BulkOrInterrupt>(endpoint: &mut Endpoint<T, In>, name: &str) -> usize {
    let len = endpoint.max_packet_size() * 128;
    let mut total = 0;
    for _ in 0..DRAIN_READS {
        match transfer(endpoint, Buffer::new(len), DRAIN_TIMEOUT) {
            Ok(done) if done.status.is_ok() => {
                let bytes = done.buffer.into_vec();
                trace!(
                    "  drained {} bytes from {name}: {:02x?}",
                    bytes.len(),
                    &bytes[..bytes.len().min(32)]
                );
                total += bytes.len();
            }
            Ok(done) => {
                trace!("  draining {name}: {:?}", done.status);
                break;
            }
            Err(_) => break,
        }
    }
    total
}

impl NusbLink {
    fn control_out(&mut self, request: u8, data: &[u8]) -> Result<(), TransportError> {
        self.interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request,
                    value: 0,
                    index: self.interface.interface_number().into(),
                    data,
                },
                CONTROL_TIMEOUT,
            )
            .wait()
            .map_err(transport)
    }
}

impl Link for NusbLink {
    fn max_packet_size(&self) -> usize {
        self.bulk_out.max_packet_size()
    }

    fn write(&mut self, data: &[u8], timeout: Duration) -> Result<(), TransportError> {
        let buf = if data.is_empty() {
            Buffer::new(0)
        } else {
            Buffer::from(data.to_vec())
        };
        let start = Instant::now();
        let result = transfer(&mut self.bulk_out, buf, timeout);
        trace!(
            "  OUT {} bytes: {} in {:.1} ms",
            data.len(),
            match &result {
                Ok(done) => format!("{} sent, {:?}", done.actual_len, done.status),
                Err(e) => e.to_string(),
            },
            start.elapsed().as_secs_f64() * 1e3
        );
        let done = result?;
        done.status.map_err(transport)?;
        if done.actual_len != data.len() {
            return Err(TransportError::Other(format!(
                "sent {} of {} bytes",
                done.actual_len,
                data.len()
            )));
        }
        Ok(())
    }

    fn read(&mut self, len: usize, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        let start = Instant::now();
        let result = transfer(&mut self.bulk_in, Buffer::new(len), timeout);
        trace!(
            "  IN up to {len} bytes: {} in {:.1} ms",
            match &result {
                Ok(done) => format!("{} received, {:?}", done.actual_len, done.status),
                Err(e) => e.to_string(),
            },
            start.elapsed().as_secs_f64() * 1e3
        );
        let done = result?;
        done.status.map_err(transport)?;
        Ok(done.buffer.into_vec())
    }

    fn device_status(&mut self) -> Result<DeviceStatus, TransportError> {
        let bytes = self
            .interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: GET_DEVICE_STATUS,
                    value: 0,
                    index: self.interface.interface_number().into(),
                    length: 20,
                },
                CONTROL_TIMEOUT,
            )
            .wait()
            .map_err(transport)?;
        trace!("  Get Device Status: {bytes:02x?}");
        DeviceStatus::parse(&bytes).map_err(|e| TransportError::Other(e.0))
    }

    fn cancel_request(&mut self, transaction: u32) -> Result<(), TransportError> {
        let mut data = [0; 6];
        data[..2].copy_from_slice(&CANCEL_TRANSACTION.to_le_bytes());
        data[2..].copy_from_slice(&transaction.to_le_bytes());
        self.control_out(CANCEL_REQUEST, &data)
    }

    fn device_reset(&mut self) -> Result<(), TransportError> {
        self.control_out(DEVICE_RESET_REQUEST, &[])
    }

    fn drain(&mut self) -> usize {
        drain_endpoint(&mut self.bulk_in, "bulk IN")
            + drain_endpoint(&mut self.interrupt_in, "interrupt IN")
    }

    fn clear_halts(&mut self) -> Result<(), TransportError> {
        let failed = |e: nusb::Error| TransportError::Other(e.to_string());
        let a = self.bulk_in.clear_halt().wait().map_err(failed);
        let b = self.bulk_out.clear_halt().wait().map_err(failed);
        a.and(b)
    }
}

/// An open MTP session with a connected Garmin watch.
pub struct UsbMtp(Session<NusbLink>);

fn find_watch() -> Result<nusb::DeviceInfo, Error> {
    let mut candidates: Vec<_> = nusb::list_devices()
        .wait()
        .map_err(|e| device_error("cannot list USB devices", e))?
        .filter(|d| {
            d.vendor_id() == GARMIN_VENDOR_ID
                && d.interfaces()
                    .any(|i| is_mtp_class(i.class(), i.subclass(), i.protocol()))
        })
        .collect();
    match candidates.len() {
        0 => Err(Error::new(
            ErrorCode::NoDevice,
            "No Garmin watch in USB file-transfer (MTP) mode was found. Connect it with a data cable.",
        )),
        1 => Ok(candidates.pop().unwrap()),
        n => Err(Error::new(
            ErrorCode::MultipleDevices,
            format!("{n} Garmin devices are connected; connect only the watch to update."),
        )),
    }
}

fn open_link(info: &nusb::DeviceInfo) -> Result<(nusb::Device, NusbLink), Error> {
    let device = info
        .open()
        .wait()
        .map_err(|e| device_error("cannot open the watch", e))?;
    let endpoints = {
        let config = device.active_configuration().map_err(|e| {
            Error::new(
                ErrorCode::DeviceError,
                format!("cannot read the watch's USB configuration: {e}"),
            )
        })?;
        find_mtp_endpoints(&config).ok_or_else(|| {
            Error::new(
                ErrorCode::UnsupportedDevice,
                "The Garmin device exposes no MTP bulk and interrupt endpoints.",
            )
        })?
    };
    let interface = device
        .claim_interface(endpoints.interface)
        .wait()
        .map_err(|e| device_error("cannot claim the watch's MTP interface", e))?;
    let link = NusbLink {
        bulk_in: interface
            .endpoint::<Bulk, In>(endpoints.bulk_in)
            .map_err(|e| device_error("cannot open the bulk IN endpoint", e))?,
        bulk_out: interface
            .endpoint::<Bulk, Out>(endpoints.bulk_out)
            .map_err(|e| device_error("cannot open the bulk OUT endpoint", e))?,
        interrupt_in: interface
            .endpoint::<Interrupt, In>(endpoints.interrupt_in)
            .map_err(|e| device_error("cannot open the interrupt endpoint", e))?,
        interface,
    };
    trace!(
        "claimed interface {} (bulk IN {:#04x}, OUT {:#04x}, max packet {})",
        endpoints.interface,
        endpoints.bulk_in,
        endpoints.bulk_out,
        link.bulk_out.max_packet_size()
    );
    Ok((device, link))
}

fn unresponsive(e: &MtpError) -> Error {
    Error::new(
        ErrorCode::DeviceError,
        format!(
            "The watch does not answer MTP requests ({e}). Unplug it, restart it and try again."
        ),
    )
}

/// How long to wait for the watch to come back after a USB reset.
const REENUMERATE_TIMEOUT: Duration = Duration::from_secs(30);

/// Finds the single Garmin device with an MTP interface, claims it and opens a session.
///
/// If the watch's MTP responder takes no commands even after the class-level recovery in
/// [`Session::open`], the device is reset once (re-enumerated, as libmtp does and as a replug
/// would), and the session opened again.
pub fn connect() -> Result<UsbMtp, Error> {
    let info = find_watch()?;
    let (device, link) = open_link(&info)?;
    let error = match Session::open(link) {
        Ok(session) => return Ok(UsbMtp(session)),
        Err(e @ MtpError::Transport { .. }) => e,
        Err(e) => return Err(unresponsive(&e)),
    };
    trace!("{error}: resetting the USB device so it re-enumerates");
    device
        .reset()
        .wait()
        .map_err(|e| device_error("cannot reset the watch's USB connection", e))?;
    drop(device);
    let old = info.id();
    let deadline = Instant::now() + REENUMERATE_TIMEOUT;
    let info = loop {
        std::thread::sleep(Duration::from_millis(500));
        match find_watch() {
            Ok(info) if info.id() != old => break info,
            _ if Instant::now() < deadline => continue,
            _ => {
                return Err(Error::new(
                    ErrorCode::NoDevice,
                    format!(
                        "The watch did not come back in MTP mode within {} s of a USB reset \
                         ({error}). Unplug it, restart it and try again.",
                        REENUMERATE_TIMEOUT.as_secs()
                    ),
                ));
            }
        }
    };
    trace!("watch re-enumerated");
    let (_device, link) = open_link(&info)?;
    Session::open(link)
        .map(UsbMtp)
        .map_err(|e| unresponsive(&e))
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

    fn create_object(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
        data: &[u8],
    ) -> Result<NewObject, CreateError> {
        self.0.create_object(storage, parent, info, data)
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        self.0.delete_object(handle)
    }
}
