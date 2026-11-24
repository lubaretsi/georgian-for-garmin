//! USB backend: a PTP session over the watch's MTP bulk endpoints, through `nusb`.

use std::time::{Duration, Instant};

use nusb::descriptors::{ConfigurationDescriptor, TransferType};
use nusb::transfer::{
    Buffer, Bulk, BulkOrInterrupt, Completion, ControlIn, ControlOut, ControlType, Direction,
    EndpointDirection, In, Interrupt, Out, Recipient, TransferError,
};
use nusb::{Endpoint, ErrorKind, Interface, MaybeFuture};

use crate::GARMIN_VENDOR_ID;
use crate::busy::{self, Busy, UsbInterface};
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
pub(crate) fn is_mtp_class(class: u8, subclass: u8, protocol: u8) -> bool {
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

/// The shipped udev rules file; the permission error's one-line fix writes its rule line.
const UDEV_RULES_FILE: &str = include_str!("../../../packaging/linux/70-georgian-for-garmin.rules");

/// The rule lines of [`UDEV_RULES_FILE`]: neither blank nor comments.
fn udev_rules() -> impl Iterator<Item = &'static str> {
    UDEV_RULES_FILE
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn device_error(context: &str, e: nusb::Error) -> Error {
    match e.kind() {
        ErrorKind::Busy => Error::busy(Busy::unknown()),
        ErrorKind::Disconnected => Error::new(ErrorCode::NoDevice, "The watch disconnected."),
        ErrorKind::PermissionDenied => Error::new(
            ErrorCode::PermissionDenied,
            if cfg!(target_os = "linux") {
                format!(
                    "{context}: permission denied ({e}). Let your user open Garmin devices, then \
                     reconnect the watch:\n  echo '{}' | sudo tee \
                     /etc/udev/rules.d/70-georgian-for-garmin.rules && sudo udevadm control --reload\n\
                     That is the rule shipped as 70-georgian-for-garmin.rules. Over SSH, also run \
                     `sudo usermod -aG plugdev \"$USER\"` and log in again.",
                    udev_rules().next().unwrap_or_default()
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

/// The Garmin devices with an MTP interface.
fn garmin_mtp_devices() -> Result<Vec<nusb::DeviceInfo>, Error> {
    Ok(nusb::list_devices()
        .wait()
        .map_err(|e| device_error("cannot list USB devices", e))?
        .filter(|d| {
            d.vendor_id() == GARMIN_VENDOR_ID
                && d.interfaces()
                    .any(|i| is_mtp_class(i.class(), i.subclass(), i.protocol()))
        })
        .collect())
}

/// The watch, when `candidates` (the Garmin MTP devices) has exactly one.
fn single<T>(mut candidates: Vec<T>) -> Result<T, Error> {
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
/// How often to look for it meanwhile.
const REENUMERATE_POLL: Duration = Duration::from_millis(500);
/// A watch re-enumerating after a USB reset leaves the device list only briefly. When no Garmin
/// MTP device is listed for this long, the watch was unplugged, and [`connect`] stops waiting
/// before [`REENUMERATE_TIMEOUT`].
const UNPLUGGED_GRACE: Duration = Duration::from_secs(5);

/// A `device_busy` failure for `info` with who holds the watch, as far as the system tells
/// ([`crate::busy`]); other failures unchanged.
fn name_holder(e: Error, info: &nusb::DeviceInfo) -> Error {
    if e.code != ErrorCode::DeviceBusy {
        return e;
    }
    let interface = info
        .interfaces()
        .find(|i| is_mtp_class(i.class(), i.subclass(), i.protocol()))
        .map(|i| UsbInterface {
            class: i.class(),
            subclass: i.subclass(),
            protocol: i.protocol(),
        });
    // nusb's product string on macOS can be a hub's when the watch has none (the Venu 2's), so
    // there it comes from the watch's own registry entry.
    #[cfg(target_os = "macos")]
    let (holder, product_name) = busy::macos(info.registry_entry_id());
    #[cfg(target_os = "linux")]
    let (holder, product_name) = (
        busy::linux(info.busnum(), info.device_address()),
        busy::product_name(info.product_string()),
    );
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let (holder, product_name) = (busy::Holder::unknown(), None);
    Error::busy(Busy {
        holder,
        product_id: Some(info.product_id()),
        product_name,
        interface,
    })
}

/// An opened device, and the MTP session started on it; the device is kept for a reset when the
/// session fails.
struct Opened<D, S> {
    device: D,
    session: Result<S, MtpError>,
}

/// What [`connect`] does with USB and the clock: [`Nusb`], or a fake in the tests.
trait Bus {
    type Id: Copy + PartialEq;
    type Info;
    type Device;
    type Session;
    /// The Garmin devices with an MTP interface.
    fn watches(&mut self) -> Result<Vec<Self::Info>, Error>;
    fn id(info: &Self::Info) -> Self::Id;
    /// Opens the device and starts an MTP session.
    fn open(&mut self, info: &Self::Info) -> Result<Opened<Self::Device, Self::Session>, Error>;
    /// A USB reset: the device re-enumerates under a new id.
    fn reset(&mut self, device: Self::Device) -> Result<(), Error>;
    fn now(&self) -> Instant;
    fn sleep(&mut self, duration: Duration);
}

/// This computer's USB devices.
struct Nusb;

impl Bus for Nusb {
    type Id = nusb::DeviceId;
    type Info = nusb::DeviceInfo;
    type Device = nusb::Device;
    type Session = UsbMtp;

    fn watches(&mut self) -> Result<Vec<nusb::DeviceInfo>, Error> {
        garmin_mtp_devices()
    }

    fn id(info: &nusb::DeviceInfo) -> nusb::DeviceId {
        info.id()
    }

    fn open(&mut self, info: &nusb::DeviceInfo) -> Result<Opened<nusb::Device, UsbMtp>, Error> {
        let (device, link) = open_link(info).map_err(|e| name_holder(e, info))?;
        Ok(Opened {
            device,
            session: Session::open(link).map(UsbMtp),
        })
    }

    fn reset(&mut self, device: nusb::Device) -> Result<(), Error> {
        device
            .reset()
            .wait()
            .map_err(|e| device_error("cannot reset the watch's USB connection", e))
    }

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

fn disconnected() -> Error {
    Error::new(ErrorCode::NoDevice, "The watch disconnected.")
}

/// Whether no Garmin MTP device with `id` is listed. A failed listing tells nothing: not gone.
fn gone<B: Bus>(bus: &mut B, id: B::Id) -> bool {
    bus.watches()
        .is_ok_and(|watches| !watches.iter().any(|w| B::id(w) == id))
}

/// Finds the single Garmin device with an MTP interface, claims it and opens a session.
///
/// If the watch's MTP responder takes no commands even after the class-level recovery in
/// [`Session::open`], the device is reset once (re-enumerated, as libmtp does and as a replug
/// would), and the session opened again. A watch unplugged meanwhile fails the same way; it is
/// `no_device` at once, or once [`UNPLUGGED_GRACE`] passes with no Garmin MTP device listed.
pub fn connect() -> Result<UsbMtp, Error> {
    connect_with(&mut Nusb)
}

fn connect_with<B: Bus>(bus: &mut B) -> Result<B::Session, Error> {
    let info = single(bus.watches()?)?;
    let old = B::id(&info);
    let Opened { device, session } = bus.open(&info)?;
    let error = match session {
        Ok(session) => return Ok(session),
        Err(e @ MtpError::Transport { .. }) => e,
        Err(e) => return Err(unresponsive(&e)),
    };
    if gone(bus, old) {
        trace!("{error}: the watch is no longer listed");
        return Err(disconnected());
    }
    trace!("{error}: resetting the USB device so it re-enumerates");
    if let Err(e) = bus.reset(device) {
        return Err(if gone(bus, old) { disconnected() } else { e });
    }
    let reset = bus.now();
    let mut last_listed = reset;
    let info = loop {
        bus.sleep(REENUMERATE_POLL);
        let now = bus.now();
        match bus.watches() {
            Ok(mut watches) => {
                if watches.len() == 1 && B::id(&watches[0]) != old {
                    break watches.swap_remove(0);
                }
                if !watches.is_empty() {
                    last_listed = now;
                } else if now - last_listed >= UNPLUGGED_GRACE {
                    trace!("no Garmin MTP device listed for {:?}", now - last_listed);
                    return Err(Error::new(
                        ErrorCode::NoDevice,
                        "The watch was disconnected while its USB connection was being reset.",
                    ));
                }
            }
            // Tells nothing either way: wait on, up to the limit.
            Err(_) => last_listed = now,
        }
        if now - reset >= REENUMERATE_TIMEOUT {
            return Err(Error::new(
                ErrorCode::NoDevice,
                format!(
                    "The watch did not come back in MTP mode within {} s of a USB reset \
                     ({error}). Unplug it, restart it and try again.",
                    REENUMERATE_TIMEOUT.as_secs()
                ),
            ));
        }
    };
    trace!("watch re-enumerated");
    let new = B::id(&info);
    match bus.open(&info)?.session {
        Ok(session) => Ok(session),
        Err(MtpError::Transport { .. }) if gone(bus, new) => Err(disconnected()),
        Err(e) => Err(unresponsive(&e)),
    }
}

// Every method is forwarded, the trait's defaults included: a default left here would answer
// in place of the session's own (as GetPartialObject and raw transactions once did).
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

    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        self.0.garmin_object_ids(storage, parent)
    }

    fn read_only(&mut self, handle: u32, info: &crate::ptp::ObjectInfo) -> Option<bool> {
        self.0.read_only(handle, info)
    }

    #[cfg(feature = "dev")]
    fn storage_info(&mut self, storage: u32) -> Result<crate::ptp::StorageInfo, MtpError> {
        self.0.storage_info(storage)
    }

    #[cfg(feature = "dev")]
    fn get_partial_object(
        &mut self,
        handle: u32,
        offset: u32,
        len: u32,
    ) -> Result<Vec<u8>, MtpError> {
        self.0.get_partial_object(handle, offset, len)
    }

    #[cfg(feature = "dev")]
    fn ptp_transaction(
        &mut self,
        code: u16,
        params: &[u32],
    ) -> Result<crate::mtp::RawResponse, MtpError> {
        self.0.ptp_transaction(code, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mtp::Phase;

    /// How long the fake takes to open a session, and to reset a device.
    const OPENING: Duration = Duration::from_secs(1);
    const RESETTING: Duration = Duration::from_millis(100);

    /// Devices are ids; the session is the id of the device it opened.
    struct FakeBus {
        start: Instant,
        elapsed: Duration,
        /// The Garmin MTP devices listed at each time since the start.
        listed: fn(Duration) -> Vec<u64>,
        /// The devices whose MTP session opens; others fail with a timed-out transfer.
        answering: &'static [u64],
        reset_fails: bool,
        resets: usize,
    }

    impl Bus for FakeBus {
        type Id = u64;
        type Info = u64;
        type Device = u64;
        type Session = u64;

        fn watches(&mut self) -> Result<Vec<u64>, Error> {
            Ok((self.listed)(self.elapsed))
        }

        fn id(info: &u64) -> u64 {
            *info
        }

        fn open(&mut self, info: &u64) -> Result<Opened<u64, u64>, Error> {
            self.elapsed += OPENING;
            let session = if self.answering.contains(info) {
                Ok(*info)
            } else {
                Err(MtpError::Transport {
                    operation: crate::ptp::op::OPEN_SESSION,
                    phase: Phase::Reply,
                    error: TransportError::TimedOut,
                })
            };
            Ok(Opened {
                device: *info,
                session,
            })
        }

        fn reset(&mut self, _: u64) -> Result<(), Error> {
            self.elapsed += RESETTING;
            self.resets += 1;
            if self.reset_fails {
                Err(Error::new(
                    ErrorCode::DeviceError,
                    "cannot reset the watch's USB connection: failed to reset device",
                ))
            } else {
                Ok(())
            }
        }

        fn now(&self) -> Instant {
            self.start + self.elapsed
        }

        fn sleep(&mut self, duration: Duration) {
            self.elapsed += duration;
        }
    }

    fn connect_fake(
        listed: fn(Duration) -> Vec<u64>,
        answering: &'static [u64],
        reset_fails: bool,
    ) -> (Result<u64, Error>, FakeBus) {
        let mut bus = FakeBus {
            start: Instant::now(),
            elapsed: Duration::ZERO,
            listed,
            answering,
            reset_fails,
            resets: 0,
        };
        (connect_with(&mut bus), bus)
    }

    fn secs(s: f64) -> Duration {
        Duration::from_secs_f64(s)
    }

    #[test]
    fn a_watch_that_answers_opens_without_a_reset() {
        let (result, bus) = connect_fake(|_| vec![1], &[1], false);
        assert_eq!(result.unwrap(), 1);
        assert_eq!(bus.resets, 0);
    }

    #[test]
    fn a_watch_unplugged_while_opening_is_no_device_without_a_reset() {
        let (result, bus) =
            connect_fake(|t| if t < secs(0.5) { vec![1] } else { vec![] }, &[], false);
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::NoDevice, "{error:?}");
        assert_eq!(bus.resets, 0);
        assert_eq!(bus.elapsed, OPENING);
    }

    #[test]
    fn a_watch_unplugged_after_the_reset_is_no_device_after_the_grace_period() {
        let (result, bus) =
            connect_fake(|t| if t < secs(2.0) { vec![1] } else { vec![] }, &[], false);
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::NoDevice, "{error:?}");
        assert!(error.message.contains("disconnected"), "{error:?}");
        assert_eq!(bus.resets, 1);
        assert!(
            bus.elapsed >= secs(2.0) + UNPLUGGED_GRACE - REENUMERATE_POLL,
            "{:?}",
            bus.elapsed
        );
        assert!(
            bus.elapsed <= secs(2.0) + UNPLUGGED_GRACE + REENUMERATE_POLL,
            "{:?}",
            bus.elapsed
        );
    }

    #[test]
    fn a_watch_that_re_enumerates_after_the_reset_opens() {
        // Listed under its old id just after the reset, gone for 2 s, back under a new id.
        let (result, bus) = connect_fake(
            |t| match t {
                t if t < secs(2.0) => vec![1],
                t if t < secs(4.0) => vec![],
                _ => vec![2],
            },
            &[2],
            false,
        );
        assert_eq!(result.unwrap(), 2);
        assert_eq!(bus.resets, 1);
    }

    #[test]
    fn a_watch_that_stays_listed_but_never_re_enumerates_waits_out_the_limit() {
        let (result, bus) = connect_fake(|_| vec![1], &[], false);
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::NoDevice, "{error:?}");
        assert!(error.message.contains("did not come back"), "{error:?}");
        assert_eq!(bus.resets, 1);
        assert!(
            bus.elapsed >= OPENING + RESETTING + REENUMERATE_TIMEOUT,
            "{:?}",
            bus.elapsed
        );
    }

    #[test]
    fn a_reset_that_fails_because_the_watch_is_gone_is_no_device() {
        // Listed through the open; gone by the time the reset fails.
        let (result, bus) =
            connect_fake(|t| if t <= OPENING { vec![1] } else { vec![] }, &[], true);
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::NoDevice, "{error:?}");
        assert_eq!(bus.resets, 1);
        assert_eq!(bus.elapsed, OPENING + RESETTING);
    }

    #[test]
    fn a_reset_that_fails_with_the_watch_still_listed_is_a_device_error() {
        let (result, _) = connect_fake(|_| vec![1], &[], true);
        assert_eq!(result.unwrap_err().code, ErrorCode::DeviceError);
    }

    #[test]
    fn a_re_enumerated_watch_unplugged_while_opening_is_no_device() {
        let (result, bus) = connect_fake(
            |t| match t {
                t if t < secs(1.6) => vec![1],
                t if t < secs(2.0) => vec![2],
                _ => vec![],
            },
            &[],
            false,
        );
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::NoDevice, "{error:?}");
        assert_eq!(bus.resets, 1);
    }

    #[test]
    fn the_shipped_udev_file_has_exactly_one_rule() {
        let rules: Vec<&str> = super::udev_rules().collect();
        assert_eq!(rules.len(), 1, "{rules:?}");
        assert!(
            rules[0].contains("ATTR{idVendor}==\"091e\""),
            "{}",
            rules[0]
        );
        assert!(
            !rules[0].contains('\''),
            "the fix quotes the rule in single quotes"
        );
    }
}
