//! The MTP operations the installer needs, and a PTP session that runs them over a USB link.
//! Windows reaches the same operations through Windows Portable Devices ([`crate::wpd`]).

use std::fmt;
use std::time::Duration;

use crate::ptp::{
    self, DATA, DeviceInfo, HEADER_LEN, Header, ObjectInfo, ProtocolError, RESPONSE, Response, op,
    response,
};
use crate::trace;

/// A failed USB transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    TimedOut,
    Stall,
    Disconnected,
    Other(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::TimedOut => f.write_str("timed out"),
            TransportError::Stall => f.write_str("was stalled by the device"),
            TransportError::Disconnected => f.write_str("failed because the device disconnected"),
            TransportError::Other(m) => write!(f, "failed ({m})"),
        }
    }
}

/// Where in a transaction a transfer failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Command,
    DataOut,
    /// Waiting for the device's first container: data or the response.
    Reply,
    DataIn,
    Response,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Phase::Command => "while sending the command",
            Phase::DataOut => "while sending data",
            Phase::Reply => "waiting for the reply",
            Phase::DataIn => "while receiving data",
            Phase::Response => "waiting for the response",
        })
    }
}

/// A failed MTP operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MtpError {
    /// The device answered with a response code other than OK.
    Response { operation: u16, code: u16 },
    /// A USB transfer of the operation failed.
    Transport {
        operation: u16,
        phase: Phase,
        error: TransportError,
    },
    /// The device sent bytes that do not parse.
    Protocol(String),
    /// A Windows Portable Devices call failed with `hresult`.
    Wpd {
        action: &'static str,
        hresult: u32,
        message: String,
    },
}

impl fmt::Display for MtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MtpError::Response { operation, code } => {
                write!(f, "{} failed: ", ptp::operation_name(*operation))?;
                match ptp::response_name(*code) {
                    Some(name) => write!(f, "{name} ({code:#06x})"),
                    None => write!(f, "response {code:#06x}"),
                }
            }
            MtpError::Transport {
                operation,
                phase,
                error,
            } => write!(f, "{} {error} {phase}", ptp::operation_name(*operation)),
            MtpError::Protocol(m) => write!(f, "unexpected MTP data: {m}"),
            MtpError::Wpd {
                action,
                hresult,
                message,
            } if message.is_empty() => write!(f, "{action} failed (HRESULT {hresult:#010x})"),
            MtpError::Wpd {
                action,
                hresult,
                message,
            } => write!(f, "{action} failed: {message} (HRESULT {hresult:#010x})"),
        }
    }
}

impl std::error::Error for MtpError {}

impl From<ProtocolError> for MtpError {
    fn from(e: ProtocolError) -> Self {
        MtpError::Protocol(e.0)
    }
}

/// Where the device placed a new object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewObject {
    pub storage: u32,
    pub parent: u32,
    pub handle: u32,
}

/// A failed [`Mtp::create_object`], saying whether an object may be left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateError {
    /// The device confirmed no new object, though it may have created one.
    Unconfirmed(MtpError),
    /// The device put the new object somewhere other than the requested storage and parent.
    /// The USB backend sends no data to it.
    Misplaced(NewObject),
    /// The object `handle` was created, but its data failed.
    Data { handle: u32, error: MtpError },
}

/// The MTP operations used by the installer, within an open session.
///
/// A backend owns session setup and teardown. The USB backend implements this over
/// [`Session`]; on Windows, [`crate::wpd`] implements it with Windows Portable Devices.
pub trait Mtp {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError>;
    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError>;
    /// Objects of any format on `storage`; `parent` is a handle, [`ptp::ALL_OBJECTS`] or
    /// [`ptp::ROOT`].
    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError>;
    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError>;
    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError>;
    /// Creates the file `info` describes in `parent` on `storage`, with `data` as its contents.
    fn create_object(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
        data: &[u8],
    ) -> Result<NewObject, CreateError>;
    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError>;
    /// Garmin's vendor `GetObjectIds` ([`op::GARMIN_GET_OBJECT_IDS`]): the children of `parent`
    /// on `storage`, including ones the watch omits from `GetObjectHandles`. Callers check
    /// that the device lists the operation; a backend that cannot send it answers as a device
    /// that doesn't support it.
    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let _ = (storage, parent);
        Err(MtpError::Response {
            operation: op::GARMIN_GET_OBJECT_IDS,
            code: response::OPERATION_NOT_SUPPORTED,
        })
    }
}

/// The still-image class `Get Device Status` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceStatus {
    pub code: u16,
    /// Endpoint addresses the device reports as halted.
    pub halted_endpoints: Vec<u32>,
}

impl DeviceStatus {
    pub fn is_ready(&self) -> bool {
        self.code == response::OK && self.halted_endpoints.is_empty()
    }

    /// `wLength` (u16, bytes in the dataset), status code (u16), then one u32 per halted
    /// endpoint.
    pub fn parse(bytes: &[u8]) -> Result<DeviceStatus, ProtocolError> {
        let mut r = ptp::Reader::new(bytes);
        let length = (r.u16()? as usize).min(bytes.len());
        let code = r.u16()?;
        if length < 4 {
            return Err(ProtocolError(format!("device status of {length} bytes")));
        }
        let halted_endpoints = bytes[4..length]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Ok(DeviceStatus {
            code,
            halted_endpoints,
        })
    }
}

/// A PTP-over-USB link: the bulk endpoints plus the still-image class control requests
/// (PIMA 15740 USB transport) used to recover a device stuck mid-transaction.
pub trait Link {
    fn max_packet_size(&self) -> usize;
    /// One OUT transfer. An empty slice sends a zero-length packet.
    fn write(&mut self, data: &[u8], timeout: Duration) -> Result<(), TransportError>;
    /// One IN transfer of up to `len` bytes (a multiple of the packet size); it ends early on a
    /// short packet.
    fn read(&mut self, len: usize, timeout: Duration) -> Result<Vec<u8>, TransportError>;
    /// Class request 0x67.
    fn device_status(&mut self) -> Result<DeviceStatus, TransportError>;
    /// Class request 0x64 for `transaction`.
    fn cancel_request(&mut self, transaction: u32) -> Result<(), TransportError>;
    /// Class request 0x66: abort all transactions and close the session.
    fn device_reset(&mut self) -> Result<(), TransportError>;
    /// `CLEAR_FEATURE(ENDPOINT_HALT)` on both bulk endpoints; also resynchronizes data toggles
    /// after an aborted transfer.
    fn clear_halts(&mut self) -> Result<(), TransportError>;
    /// Reads and discards whatever the device has queued on its IN endpoints (a reply or
    /// events a previous host never read); returns the number of bytes.
    fn drain(&mut self) -> usize;
    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Bytes requested per IN transfer, rounded down to a multiple of the packet size.
const READ_CHUNK: usize = 64 * 1024;
/// Timeout for the first OpenSession, so a wedged device is noticed quickly.
const OPEN_TIMEOUT: Duration = Duration::from_secs(5);
/// Per-transfer timeout otherwise. libmtp uses 60 s for this watch (`DEVICE_FLAG_LONG_TIMEOUT`).
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60);
const STATUS_POLLS: usize = 20;
const STATUS_INTERVAL: Duration = Duration::from_millis(250);

/// A PTP session over a USB link.
pub struct Session<L: Link> {
    link: L,
    next_transaction: u32,
    /// Bytes read past the end of a data phase: the start of the next container.
    pending: Option<Vec<u8>>,
    open: bool,
    timeout: Duration,
    /// The device sends data-container headers as their own 12-byte transfer and expects the
    /// host to do the same (libmtp's `split_header_data`, autodetected the same way). The Venu 2
    /// is such a device: a header and payload in one transfer leave its MTP task hung.
    split_header: bool,
    /// `GetDeviceInfo`, read once when the session opens.
    info: Option<DeviceInfo>,
}

fn container_name(kind: u16) -> &'static str {
    match kind {
        ptp::COMMAND => "Command",
        DATA => "Data",
        RESPONSE => "Response",
        4 => "Event",
        _ => "Unknown",
    }
}

impl<L: Link> Session<L> {
    /// Opens session 1 and reads `GetDeviceInfo`. Queued replies from an earlier host are
    /// drained, and a device reporting itself busy is reset first; if OpenSession still gets no
    /// answer, the device is reset and OpenSession retried once.
    pub fn open(link: L) -> Result<Self, MtpError> {
        let mut session = Session {
            link,
            next_transaction: 0,
            pending: None,
            open: false,
            timeout: OPEN_TIMEOUT,
            split_header: false,
            info: None,
        };
        let stale = session.link.drain();
        if stale > 0 {
            trace!("discarded {stale} bytes the device had queued for an earlier host");
        }
        match session.link.device_status() {
            Ok(status) if status.is_ready() => trace!("device status OK"),
            Ok(status) => {
                trace!(
                    "device status {:#06x}, halted endpoints {:x?}: resetting",
                    status.code, status.halted_endpoints
                );
                session.reset_device();
            }
            Err(e) => trace!("device status request {e}"),
        }
        match session.open_session() {
            Ok(()) => {}
            Err(MtpError::Transport { .. }) => {
                trace!("OpenSession got no answer: resetting the device and retrying once");
                session.reset_device();
                session.open_or_reopen()?;
            }
            Err(MtpError::Response {
                code: response::SESSION_ALREADY_OPEN,
                ..
            }) => session.reopen()?,
            Err(e) => return Err(e),
        }
        session.timeout = TRANSFER_TIMEOUT;
        // A data-in phase before any data-out phase, so a split-header device is recognized
        // before the first write.
        let info = session.device_info()?;
        session.info = Some(info);
        Ok(session)
    }

    fn open_or_reopen(&mut self) -> Result<(), MtpError> {
        match self.open_session() {
            Err(MtpError::Response {
                code: response::SESSION_ALREADY_OPEN,
                ..
            }) => self.reopen(),
            other => other,
        }
    }

    fn reopen(&mut self) -> Result<(), MtpError> {
        self.open = true;
        self.close()?;
        self.open_session()
    }

    fn open_session(&mut self) -> Result<(), MtpError> {
        // PTP requires transaction ID 0 for OpenSession.
        self.next_transaction = 0;
        self.transaction(op::OPEN_SESSION, &[1], None)?;
        self.open = true;
        Ok(())
    }

    /// Ends the session. Dropping the session does this too, ignoring errors.
    pub fn close(&mut self) -> Result<(), MtpError> {
        if !self.open {
            return Ok(());
        }
        self.open = false;
        self.transaction(op::CLOSE_SESSION, &[], None).map(drop)
    }

    /// Polls Get Device Status until the device is ready, clearing halts it reports.
    fn wait_ready(&mut self) -> bool {
        for _ in 0..STATUS_POLLS {
            match self.link.device_status() {
                Ok(status) if status.code == response::OK => {
                    if !status.halted_endpoints.is_empty() {
                        trace!("halted endpoints {:x?}: clearing", status.halted_endpoints);
                        self.clear_halts();
                    }
                    trace!("device status OK");
                    return true;
                }
                Ok(status) => {
                    trace!(
                        "device status {:#06x}, halted endpoints {:x?}",
                        status.code, status.halted_endpoints
                    );
                    if !status.halted_endpoints.is_empty() {
                        self.clear_halts();
                    }
                    self.link.pause(STATUS_INTERVAL);
                }
                Err(e) => {
                    trace!("device status request {e}");
                    return false;
                }
            }
        }
        false
    }

    fn clear_halts(&mut self) {
        match self.link.clear_halts() {
            Ok(()) => trace!("cleared halts on both bulk endpoints"),
            Err(e) => trace!("clearing halts {e}"),
        }
    }

    /// After a failed transfer: cancel the transaction, wait for the device to be ready, and
    /// resynchronize the bulk endpoints, so later operations (such as cleanup) can run.
    fn recover(&mut self, transaction: u32) {
        self.pending = None;
        match self.link.cancel_request(transaction) {
            Ok(()) => trace!("sent Cancel Request for transaction {transaction}"),
            Err(e) => trace!("Cancel Request {e}"),
        }
        let stale = self.link.drain();
        if stale > 0 {
            trace!("discarded {stale} bytes of the failed transaction");
        }
        self.wait_ready();
        self.clear_halts();
    }

    /// Device Reset Request: drops any transaction and session the device holds.
    fn reset_device(&mut self) {
        self.pending = None;
        self.open = false;
        match self.link.device_reset() {
            Ok(()) => trace!("sent Device Reset Request"),
            Err(e) => trace!("Device Reset Request {e}"),
        }
        self.wait_ready();
        self.clear_halts();
    }

    fn chunk(&self) -> usize {
        let mps = self.link.max_packet_size().max(1);
        (READ_CHUNK / mps).max(1) * mps
    }

    fn write(&mut self, operation: u16, phase: Phase, data: &[u8]) -> Result<(), MtpError> {
        self.link
            .write(data, self.timeout)
            .map_err(|error| MtpError::Transport {
                operation,
                phase,
                error,
            })
    }

    fn read_transfer(&mut self, operation: u16, phase: Phase) -> Result<(Vec<u8>, bool), MtpError> {
        if let Some(bytes) = self.pending.take() {
            return Ok((bytes, false));
        }
        let len = self.chunk();
        let bytes = self
            .link
            .read(len, self.timeout)
            .map_err(|error| MtpError::Transport {
                operation,
                phase,
                error,
            })?;
        // A full transfer did not end on a short packet; a zero-length packet may follow.
        let full = bytes.len() == len;
        Ok((bytes, full))
    }

    fn incoming(bytes: &[u8]) -> Result<Header, MtpError> {
        let header = Header::parse(bytes)?;
        trace!(
            "<- {} {} ({:#06x}) tid={} length={}",
            container_name(header.kind),
            if header.kind == RESPONSE {
                ptp::response_name(header.code).unwrap_or("?")
            } else {
                ptp::operation_name(header.code)
            },
            header.code,
            header.transaction,
            header.length
        );
        Ok(header)
    }

    /// Runs one transaction: command, optional data out, optional data in, response.
    /// Returns the response parameters and any data the device sent. A failed transfer or
    /// unparsable reply leaves the device mid-transaction, so it is cancelled before returning.
    fn transaction(
        &mut self,
        code: u16,
        params: &[u32],
        data_out: Option<&[u8]>,
    ) -> Result<(Vec<u32>, Vec<u8>), MtpError> {
        let tid = self.next_transaction;
        self.next_transaction = self.next_transaction.wrapping_add(1).max(1);
        let result = self.run(code, tid, params, data_out);
        if let Err(e @ (MtpError::Transport { .. } | MtpError::Protocol(_))) = &result {
            trace!("{e}: recovering");
            self.recover(tid);
        }
        result
    }

    fn run(
        &mut self,
        code: u16,
        tid: u32,
        params: &[u32],
        data_out: Option<&[u8]>,
    ) -> Result<(Vec<u32>, Vec<u8>), MtpError> {
        let name = ptp::operation_name(code);
        trace!("-> Command {name} ({code:#06x}) tid={tid} params={params:#x?}");
        self.write(
            code,
            Phase::Command,
            &ptp::encode_command(code, tid, params),
        )?;
        if let Some(payload) = data_out {
            let container = ptp::encode_data(code, tid, payload)?;
            trace!(
                "-> Data {name} tid={tid} length={}{}",
                container.len(),
                if self.split_header {
                    " (header in its own transfer)"
                } else {
                    ""
                }
            );
            let body: &[u8] = if self.split_header {
                self.write(code, Phase::DataOut, &container[..HEADER_LEN])?;
                &container[HEADER_LEN..]
            } else {
                &container
            };
            if !body.is_empty() {
                self.write(code, Phase::DataOut, body)?;
                if body
                    .len()
                    .is_multiple_of(self.link.max_packet_size().max(1))
                {
                    trace!("-> zero-length packet");
                    self.write(code, Phase::DataOut, &[])?;
                }
            }
        }

        let mut data = Vec::new();
        let (mut first, mut full) = self.read_transfer(code, Phase::Reply)?;
        while first.is_empty() {
            // A stray zero-length packet; the container follows.
            (first, full) = self.read_transfer(code, Phase::Reply)?;
        }
        let mut header = Self::incoming(&first)?;
        if header.kind == DATA {
            if !self.split_header
                && first.len() == HEADER_LEN
                && header.length as usize > HEADER_LEN
            {
                trace!("device sends data headers in their own transfer: splitting ours too");
                self.split_header = true;
            }
            if header.code != code || header.transaction != tid {
                return Err(ProtocolError(format!(
                    "data container for {:#06x}/{} during {:#06x}/{tid}",
                    header.code, header.transaction, code
                ))
                .into());
            }
            if header.length == u32::MAX {
                return Err(
                    ProtocolError("data phases over 4 GiB are not supported".into()).into(),
                );
            }
            let length = header.length as usize;
            let mut buf = first;
            while buf.len() < length {
                let (more, more_full) = self.read_transfer(code, Phase::DataIn)?;
                if more.is_empty() {
                    return Err(ProtocolError(format!(
                        "data phase ended after {} of {length} bytes",
                        buf.len()
                    ))
                    .into());
                }
                buf.extend_from_slice(&more);
                full = more_full;
            }
            if buf.len() > length {
                // The device sent no zero-length packet and the response arrived in the same
                // transfer.
                self.pending = Some(buf.split_off(length));
            } else if full {
                let (next, _) = self.read_transfer(code, Phase::Response)?;
                if !next.is_empty() {
                    self.pending = Some(next);
                }
            }
            data = buf.split_off(HEADER_LEN);
            let (next, _) = self.read_transfer(code, Phase::Response)?;
            first = next;
            header = Self::incoming(&first)?;
        }
        if header.kind != RESPONSE {
            return Err(ProtocolError(format!(
                "expected a response container, got type {}",
                header.kind
            ))
            .into());
        }
        let response = Response::parse(&first)?;
        if response.transaction != tid {
            return Err(ProtocolError(format!(
                "response for transaction {} during {tid}",
                response.transaction
            ))
            .into());
        }
        if response.code != response::OK {
            return Err(MtpError::Response {
                operation: code,
                code: response.code,
            });
        }
        Ok((response.params, data))
    }
}

impl<L: Link> Drop for Session<L> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn param(params: &[u32], index: usize, operation: u16) -> Result<u32, MtpError> {
    params.get(index).copied().ok_or_else(|| {
        MtpError::Protocol(format!(
            "{} response lacks parameter {}",
            ptp::operation_name(operation),
            index + 1
        ))
    })
}

impl<L: Link> Session<L> {
    /// Announces a new object; the device answers where it placed it.
    pub fn send_object_info(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
    ) -> Result<NewObject, MtpError> {
        let (params, _) = self.transaction(
            op::SEND_OBJECT_INFO,
            &[storage, parent],
            Some(&info.encode()?),
        )?;
        Ok(NewObject {
            storage: param(&params, 0, op::SEND_OBJECT_INFO)?,
            parent: param(&params, 1, op::SEND_OBJECT_INFO)?,
            handle: param(&params, 2, op::SEND_OBJECT_INFO)?,
        })
    }

    /// Sends the data for the object announced by the preceding `send_object_info`.
    pub fn send_object(&mut self, data: &[u8]) -> Result<(), MtpError> {
        self.transaction(op::SEND_OBJECT, &[], Some(data)).map(drop)
    }
}

impl<L: Link> Mtp for Session<L> {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        if let Some(info) = &self.info {
            return Ok(info.clone());
        }
        let (_, data) = self.transaction(op::GET_DEVICE_INFO, &[], None)?;
        let info = DeviceInfo::decode(&data)?;
        trace!(
            "device: {} {} {}, vendor extension {:?}, operations {:04x?}",
            info.manufacturer,
            info.model,
            info.device_version,
            info.vendor_extension_desc,
            info.operations_supported
        );
        Ok(info)
    }

    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError> {
        let (_, data) = self.transaction(op::GET_STORAGE_IDS, &[], None)?;
        Ok(ptp::Reader::new(&data).u32_array()?)
    }

    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let (_, data) = self.transaction(op::GET_OBJECT_HANDLES, &[storage, 0, parent], None)?;
        Ok(ptp::Reader::new(&data).u32_array()?)
    }

    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let (_, data) = self.transaction(op::GARMIN_GET_OBJECT_IDS, &[storage, parent], None)?;
        Ok(ptp::Reader::new(&data).u32_array()?)
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError> {
        let (_, data) = self.transaction(op::GET_OBJECT_INFO, &[handle], None)?;
        Ok(ObjectInfo::decode(&data)?)
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        Ok(self.transaction(op::GET_OBJECT, &[handle], None)?.1)
    }

    fn create_object(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
        data: &[u8],
    ) -> Result<NewObject, CreateError> {
        let new = self
            .send_object_info(storage, parent, info)
            .map_err(CreateError::Unconfirmed)?;
        if new.parent != parent || new.storage != storage {
            return Err(CreateError::Misplaced(new));
        }
        self.send_object(data).map_err(|error| CreateError::Data {
            handle: new.handle,
            error,
        })?;
        Ok(new)
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        self.transaction(op::DELETE_OBJECT, &[handle, 0], None)
            .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    /// A scripted device. OUT transfers are checked against `expect_out`; IN transfers replay
    /// the packets the device sends (one read collects packets until a short one or the
    /// requested length, as USB does; no packets left means a timeout). Class requests are
    /// logged in `events`, and `statuses` scripts Get Device Status answers (ready by default).
    struct ScriptedPipe {
        mps: usize,
        expect_out: VecDeque<(Vec<u8>, Option<TransportError>)>,
        in_packets: VecDeque<Vec<u8>>,
        statuses: VecDeque<Result<DeviceStatus, TransportError>>,
        events: Vec<String>,
        /// Upcoming reads that time out even if packets are queued.
        timed_out_reads: usize,
        /// IN data left from an earlier host, consumed only by `drain`.
        stale: VecDeque<Vec<u8>>,
    }

    fn ready() -> DeviceStatus {
        DeviceStatus {
            code: response::OK,
            halted_endpoints: vec![],
        }
    }

    impl ScriptedPipe {
        fn new(mps: usize) -> Self {
            ScriptedPipe {
                mps,
                expect_out: VecDeque::new(),
                in_packets: VecDeque::new(),
                statuses: VecDeque::new(),
                events: Vec::new(),
                timed_out_reads: 0,
                stale: VecDeque::new(),
            }
        }

        fn expect(&mut self, bytes: Vec<u8>) {
            self.expect_out.push_back((bytes, None));
        }

        /// The device accepts `bytes`, but the transfer then fails with `error`.
        fn expect_failing(&mut self, bytes: Vec<u8>, error: TransportError) {
            self.expect_out.push_back((bytes, Some(error)));
        }

        /// Queues a device message split into packets, with a zero-length packet when needed.
        fn device_sends(&mut self, bytes: &[u8], zlp: bool) {
            for chunk in bytes.chunks(self.mps) {
                self.in_packets.push_back(chunk.to_vec());
            }
            if zlp && bytes.len().is_multiple_of(self.mps) {
                self.in_packets.push_back(Vec::new());
            }
        }

        fn take_events(&mut self) -> Vec<String> {
            std::mem::take(&mut self.events)
        }
    }

    impl Link for &mut ScriptedPipe {
        fn max_packet_size(&self) -> usize {
            self.mps
        }

        fn write(&mut self, data: &[u8], _: Duration) -> Result<(), TransportError> {
            let (expected, error) = self
                .expect_out
                .pop_front()
                .expect("unexpected OUT transfer");
            assert_eq!(data, expected.as_slice());
            error.map_or(Ok(()), Err)
        }

        fn read(&mut self, len: usize, _: Duration) -> Result<Vec<u8>, TransportError> {
            assert_eq!(len % self.mps, 0);
            if self.timed_out_reads > 0 {
                self.timed_out_reads -= 1;
                return Err(TransportError::TimedOut);
            }
            let mut out = Vec::new();
            loop {
                let Some(packet) = self.in_packets.pop_front() else {
                    return Err(TransportError::TimedOut);
                };
                let short = packet.len() < self.mps;
                out.extend_from_slice(&packet);
                if short || out.len() == len {
                    return Ok(out);
                }
            }
        }

        fn device_status(&mut self) -> Result<DeviceStatus, TransportError> {
            self.events.push("status".into());
            self.statuses.pop_front().unwrap_or_else(|| Ok(ready()))
        }

        fn cancel_request(&mut self, transaction: u32) -> Result<(), TransportError> {
            self.events.push(format!("cancel {transaction}"));
            Ok(())
        }

        fn device_reset(&mut self) -> Result<(), TransportError> {
            self.events.push("reset".into());
            Ok(())
        }

        fn drain(&mut self) -> usize {
            self.events.push("drain".into());
            self.stale.drain(..).map(|p| p.len()).sum()
        }

        fn clear_halts(&mut self) -> Result<(), TransportError> {
            self.events.push("clear_halts".into());
            Ok(())
        }

        fn pause(&mut self, _: Duration) {
            self.events.push("pause".into());
        }
    }

    fn ok(tid: u32, params: &[u32]) -> Vec<u8> {
        Response {
            code: response::OK,
            transaction: tid,
            params: params.to_vec(),
        }
        .encode()
    }

    fn device_info_payload() -> Vec<u8> {
        let mut w = ptp::Writer::default();
        w.u16(100).u32(6).u16(100);
        w.string("microsoft.com: 1.0; ").unwrap();
        w.u16(0);
        for _ in 0..5 {
            w.u32(0); // empty operation, event, property and format lists
        }
        for s in ["Garmin", "Venu 2", "1905", ""] {
            w.string(s).unwrap();
        }
        w.0
    }

    /// The GetDeviceInfo that `Session::open` runs. A split-header device sends the data
    /// container's header in its own packet.
    fn script_device_info(pipe: &mut ScriptedPipe, tid: u32, split: bool) {
        pipe.expect(ptp::encode_command(op::GET_DEVICE_INFO, tid, &[]));
        let container = ptp::encode_data(op::GET_DEVICE_INFO, tid, &device_info_payload()).unwrap();
        if split {
            pipe.in_packets.push_back(container[..HEADER_LEN].to_vec());
            pipe.device_sends(&container[HEADER_LEN..], true);
        } else {
            pipe.device_sends(&container, true);
        }
        pipe.device_sends(&ok(tid, &[]), true);
    }

    fn opened_with(pipe: &mut ScriptedPipe, split: bool) -> Session<&mut ScriptedPipe> {
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.device_sends(&ok(0, &[]), true);
        script_device_info(pipe, 1, split);
        let session = Session::open(pipe).unwrap();
        assert_eq!(session.link.take_events(), ["drain", "status"]);
        assert_eq!(session.split_header, split);
        session
    }

    /// An open session whose next transaction ID is 2.
    fn opened(pipe: &mut ScriptedPipe) -> Session<&mut ScriptedPipe> {
        opened_with(pipe, false)
    }

    fn close(session: &mut Session<&mut ScriptedPipe>, tid: u32) {
        session
            .link
            .expect(ptp::encode_command(op::CLOSE_SESSION, tid, &[]));
        session.link.device_sends(&ok(tid, &[]), true);
        session.close().unwrap();
        assert!(session.link.expect_out.is_empty() && session.link.in_packets.is_empty());
    }

    #[test]
    fn multi_packet_data_ending_on_packet_boundary_with_zlp() {
        let mut pipe = ScriptedPipe::new(16);
        let mut s = opened(&mut pipe);
        let object: Vec<u8> = (0..52u8).collect(); // 12 + 52 = 64 bytes: four full packets
        s.link
            .expect(ptp::encode_command(op::GET_OBJECT, 2, &[0x0100_0010]));
        s.link
            .device_sends(&ptp::encode_data(op::GET_OBJECT, 2, &object).unwrap(), true);
        s.link.device_sends(&ok(2, &[]), true);
        assert_eq!(s.get_object(0x0100_0010).unwrap(), object);
        close(&mut s, 3);
    }

    #[test]
    fn data_on_packet_boundary_without_zlp_keeps_the_response() {
        let mut pipe = ScriptedPipe::new(16);
        let mut s = opened(&mut pipe);
        let object = vec![0xAA; 20]; // 32 bytes with the header
        s.link.expect(ptp::encode_command(op::GET_OBJECT, 2, &[5]));
        s.link.device_sends(
            &ptp::encode_data(op::GET_OBJECT, 2, &object).unwrap(),
            false,
        );
        s.link.device_sends(&ok(2, &[]), true);
        assert_eq!(s.get_object(5).unwrap(), object);
        close(&mut s, 3);
    }

    #[test]
    fn header_in_its_own_packet_then_payload() {
        let mut pipe = ScriptedPipe::new(512);
        let mut s = opened(&mut pipe);
        let ids = [0x0002_0001u32, 0x0001_0001];
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u32.to_le_bytes());
        for id in ids {
            payload.extend_from_slice(&id.to_le_bytes());
        }
        let container = ptp::encode_data(op::GET_STORAGE_IDS, 2, &payload).unwrap();
        s.link
            .expect(ptp::encode_command(op::GET_STORAGE_IDS, 2, &[]));
        s.link
            .in_packets
            .push_back(container[..HEADER_LEN].to_vec());
        s.link
            .in_packets
            .push_back(container[HEADER_LEN..].to_vec());
        s.link.device_sends(&ok(2, &[]), true);
        assert_eq!(s.storage_ids().unwrap(), ids);
        // Seen at any time, a split header switches our data-out phases too.
        assert!(s.split_header);
        close(&mut s, 3);
    }

    #[test]
    fn error_response_instead_of_data() {
        let mut pipe = ScriptedPipe::new(512);
        let mut s = opened(&mut pipe);
        s.link
            .expect(ptp::encode_command(op::GET_OBJECT_INFO, 2, &[0x0100_0001]));
        s.link.device_sends(
            &Response {
                code: 0x2009,
                transaction: 2,
                params: vec![],
            }
            .encode(),
            true,
        );
        let err = s.object_info(0x0100_0001).unwrap_err();
        assert_eq!(
            err,
            MtpError::Response {
                operation: op::GET_OBJECT_INFO,
                code: 0x2009
            }
        );
        assert_eq!(
            err.to_string(),
            "GetObjectInfo failed: InvalidObjectHandle (0x2009)"
        );
        // A response code is a complete transaction: no recovery.
        assert!(s.link.take_events().is_empty());
        close(&mut s, 3);
    }

    fn object_info() -> ObjectInfo {
        ObjectInfo {
            storage_id: 0x0002_0001,
            format: ptp::format::UNDEFINED,
            compressed_size: 20_720,
            parent: 0x0100_0009,
            filename: "006-D9035-03.tmp".into(),
            ..Default::default()
        }
    }

    #[test]
    fn combined_device_gets_header_and_payload_in_one_transfer() {
        let mut pipe = ScriptedPipe::new(16);
        let mut s = opened_with(&mut pipe, false);
        let info = object_info();
        s.link.expect(ptp::encode_command(
            op::SEND_OBJECT_INFO,
            2,
            &[0x0002_0001, 0x0100_0009],
        ));
        // 12 + 90 bytes in one transfer; not a multiple of 16, so no zero-length packet.
        s.link
            .expect(ptp::encode_data(op::SEND_OBJECT_INFO, 2, &info.encode().unwrap()).unwrap());
        s.link
            .device_sends(&ok(2, &[0x0002_0001, 0x0100_0009, 0x0100_0100]), true);
        let new = s.send_object_info(0x0002_0001, 0x0100_0009, &info).unwrap();
        assert_eq!(
            new,
            NewObject {
                storage: 0x0002_0001,
                parent: 0x0100_0009,
                handle: 0x0100_0100
            }
        );

        let data = [7u8; 20]; // 32 bytes with the header: needs a zero-length packet
        s.link.expect(ptp::encode_command(op::SEND_OBJECT, 3, &[]));
        s.link
            .expect(ptp::encode_data(op::SEND_OBJECT, 3, &data).unwrap());
        s.link.expect(Vec::new());
        s.link.device_sends(&ok(3, &[]), true);
        s.send_object(&data).unwrap();
        close(&mut s, 4);
    }

    #[test]
    fn split_header_device_gets_the_header_in_its_own_transfer() {
        let mut pipe = ScriptedPipe::new(512);
        let mut s = opened_with(&mut pipe, true);
        let info = object_info();
        let dataset = info.encode().unwrap();
        let container = ptp::encode_data(op::SEND_OBJECT_INFO, 2, &dataset).unwrap();
        s.link.expect(ptp::encode_command(
            op::SEND_OBJECT_INFO,
            2,
            &[0x0002_0001, 0x0100_0009],
        ));
        // Header alone (a short packet), then the 90-byte ObjectInfo, as libmtp sends it.
        s.link.expect(container[..HEADER_LEN].to_vec());
        s.link.expect(dataset.clone());
        assert_eq!(dataset.len(), 90);
        s.link
            .device_sends(&ok(2, &[0x0002_0001, 0x0100_0009, 0x0100_0100]), true);
        s.send_object_info(0x0002_0001, 0x0100_0009, &info).unwrap();

        // A payload that fills whole packets is terminated by a zero-length packet.
        let data = vec![9u8; 1024];
        let container = ptp::encode_data(op::SEND_OBJECT, 3, &data).unwrap();
        s.link.expect(ptp::encode_command(op::SEND_OBJECT, 3, &[]));
        s.link.expect(container[..HEADER_LEN].to_vec());
        s.link.expect(data.clone());
        s.link.expect(Vec::new());
        s.link.device_sends(&ok(3, &[]), true);
        s.send_object(&data).unwrap();

        // An empty payload is the header alone.
        s.link.expect(ptp::encode_command(op::SEND_OBJECT, 4, &[]));
        s.link
            .expect(ptp::encode_data(op::SEND_OBJECT, 4, &[]).unwrap());
        s.link.device_sends(&ok(4, &[]), true);
        s.send_object(&[]).unwrap();
        close(&mut s, 5);
    }

    #[test]
    fn stale_session_is_closed_and_reopened() {
        let mut pipe = ScriptedPipe::new(512);
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.device_sends(
            &Response {
                code: response::SESSION_ALREADY_OPEN,
                transaction: 0,
                params: vec![],
            }
            .encode(),
            true,
        );
        pipe.expect(ptp::encode_command(op::CLOSE_SESSION, 1, &[]));
        pipe.device_sends(&ok(1, &[]), true);
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.device_sends(&ok(0, &[]), true);
        script_device_info(&mut pipe, 1, false);
        let mut s = Session::open(&mut pipe).unwrap();
        close(&mut s, 2);
    }

    #[test]
    fn mismatched_transaction_and_truncated_data_are_errors() {
        let mut pipe = ScriptedPipe::new(512);
        let mut s = opened(&mut pipe);
        s.link
            .expect(ptp::encode_command(op::DELETE_OBJECT, 2, &[9, 0]));
        s.link.device_sends(&ok(7, &[]), true);
        assert!(matches!(s.delete_object(9), Err(MtpError::Protocol(_))));
        assert_eq!(
            s.link.take_events(),
            ["cancel 2", "drain", "status", "clear_halts"]
        );

        s.link.expect(ptp::encode_command(op::GET_OBJECT, 3, &[9]));
        let container = ptp::encode_data(op::GET_OBJECT, 3, &[1; 100]).unwrap();
        s.link.device_sends(&container[..50], true);
        s.link.in_packets.push_back(Vec::new());
        assert!(matches!(s.get_object(9), Err(MtpError::Protocol(_))));
        s.open = false;
    }

    #[test]
    fn busy_device_is_reset_before_opening() {
        let mut pipe = ScriptedPipe::new(512);
        pipe.statuses.extend([
            Ok(DeviceStatus {
                code: 0x2019,
                halted_endpoints: vec![0x81],
            }),
            Ok(DeviceStatus {
                code: 0x2019,
                halted_endpoints: vec![],
            }),
            Ok(ready()),
        ]);
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.device_sends(&ok(0, &[]), true);
        script_device_info(&mut pipe, 1, false);
        let mut s = Session::open(&mut pipe).unwrap();
        assert_eq!(
            s.link.take_events(),
            [
                "drain",
                "status",
                "reset",
                "status",
                "pause",
                "status",
                "clear_halts"
            ]
        );
        close(&mut s, 2);
    }

    #[test]
    fn unanswered_open_session_resets_and_retries_once() {
        let mut pipe = ScriptedPipe::new(512);
        pipe.timed_out_reads = 1;
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.device_sends(&ok(0, &[]), true);
        script_device_info(&mut pipe, 1, false);
        let mut s = Session::open(&mut pipe).unwrap();
        assert_eq!(
            s.link.take_events(),
            [
                "drain",
                "status",
                "cancel 0",
                "drain",
                "status",
                "clear_halts",
                "reset",
                "status",
                "clear_halts"
            ]
        );
        close(&mut s, 2);
    }

    #[test]
    fn reply_left_by_an_earlier_host_is_drained_before_opening() {
        let mut pipe = ScriptedPipe::new(512);
        pipe.stale.push_back(ok(41, &[]));
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.device_sends(&ok(0, &[]), true);
        script_device_info(&mut pipe, 1, false);
        let mut s = Session::open(&mut pipe).unwrap();
        assert!(s.link.stale.is_empty());
        close(&mut s, 2);
    }

    #[test]
    fn device_status_dataset() {
        assert_eq!(DeviceStatus::parse(&[4, 0, 0x01, 0x20]).unwrap(), ready());
        assert_eq!(
            DeviceStatus::parse(&[12, 0, 0x19, 0x20, 0x81, 0, 0, 0, 0x03, 0, 0, 0, 0xEE]).unwrap(),
            DeviceStatus {
                code: 0x2019,
                halted_endpoints: vec![0x81, 0x03]
            }
        );
        assert!(DeviceStatus::parse(&[2, 0, 0x01, 0x20]).is_err());
        assert!(DeviceStatus::parse(&[4, 0]).is_err());
    }

    #[test]
    fn wedged_device_fails_naming_the_operation() {
        let mut pipe = ScriptedPipe::new(512);
        pipe.timed_out_reads = 2;
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        let err = Session::open(&mut pipe).err().unwrap();
        assert_eq!(
            err.to_string(),
            "OpenSession timed out waiting for the reply"
        );
        assert!(pipe.expect_out.is_empty());
    }

    #[test]
    fn failed_send_is_cancelled_so_cleanup_can_run() {
        let mut pipe = ScriptedPipe::new(512);
        let mut s = opened(&mut pipe);
        let data = [3u8; 1000];
        s.link.expect(ptp::encode_command(op::SEND_OBJECT, 2, &[]));
        s.link.expect_failing(
            ptp::encode_data(op::SEND_OBJECT, 2, &data).unwrap(),
            TransportError::TimedOut,
        );
        let err = s.send_object(&data).unwrap_err();
        assert_eq!(err.to_string(), "SendObject timed out while sending data");
        assert_eq!(
            s.link.take_events(),
            ["cancel 2", "drain", "status", "clear_halts"]
        );

        // The response never comes: the error says the data went out.
        s.link.expect(ptp::encode_command(op::SEND_OBJECT, 3, &[]));
        s.link
            .expect(ptp::encode_data(op::SEND_OBJECT, 3, &data).unwrap());
        s.link.timed_out_reads = 1;
        let err = s.send_object(&data).unwrap_err();
        assert_eq!(
            err.to_string(),
            "SendObject timed out waiting for the reply"
        );
        assert_eq!(
            s.link.take_events(),
            ["cancel 3", "drain", "status", "clear_halts"]
        );

        // The session still works afterwards.
        s.link
            .expect(ptp::encode_command(op::DELETE_OBJECT, 4, &[0x0100_0100, 0]));
        s.link.device_sends(&ok(4, &[]), true);
        s.delete_object(0x0100_0100).unwrap();
        close(&mut s, 5);
    }
}
