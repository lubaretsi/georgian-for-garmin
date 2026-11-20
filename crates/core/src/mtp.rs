//! The MTP operations the installer needs, and a PTP session that runs them over bulk pipes.

use std::fmt;

use crate::ptp::{
    self, DATA, DeviceInfo, HEADER_LEN, Header, ObjectInfo, ProtocolError, RESPONSE, Response, op,
    response,
};

/// A failed MTP operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MtpError {
    /// The device answered with a response code other than OK.
    Response { operation: u16, code: u16 },
    /// The USB link failed (timeout, stall, disconnect).
    Transport(String),
    /// The device sent bytes that do not parse.
    Protocol(String),
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
            MtpError::Transport(m) => write!(f, "USB transfer failed: {m}"),
            MtpError::Protocol(m) => write!(f, "unexpected MTP data: {m}"),
        }
    }
}

impl std::error::Error for MtpError {}

impl From<ProtocolError> for MtpError {
    fn from(e: ProtocolError) -> Self {
        MtpError::Protocol(e.0)
    }
}

/// Where `SendObjectInfo` placed the new object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewObject {
    pub storage: u32,
    pub parent: u32,
    pub handle: u32,
}

/// The MTP operations used by the installer, within an open session.
///
/// A backend owns session setup and teardown. The USB backend implements this over
/// [`Session`]; a Windows WPD backend can implement it with WPD's MTP pass-through commands.
pub trait Mtp {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError>;
    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError>;
    /// Objects of any format on `storage`; `parent` is a handle, [`ptp::ALL_OBJECTS`] or
    /// [`ptp::ROOT`].
    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError>;
    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError>;
    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError>;
    fn send_object_info(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
    ) -> Result<NewObject, MtpError>;
    /// Sends the data for the object announced by the preceding `send_object_info`.
    fn send_object(&mut self, data: &[u8]) -> Result<(), MtpError>;
    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError>;
}

/// One bulk IN and one bulk OUT endpoint.
pub trait BulkPipe {
    fn max_packet_size(&self) -> usize;
    /// One OUT transfer. An empty slice sends a zero-length packet.
    fn write(&mut self, data: &[u8]) -> Result<(), MtpError>;
    /// One IN transfer of up to `len` bytes (a multiple of the packet size); it ends early on a
    /// short packet.
    fn read(&mut self, len: usize) -> Result<Vec<u8>, MtpError>;
}

/// Bytes requested per IN transfer, rounded down to a multiple of the packet size.
const READ_CHUNK: usize = 64 * 1024;

/// A PTP session over bulk pipes.
pub struct Session<P: BulkPipe> {
    pipe: P,
    next_transaction: u32,
    /// Bytes read past the end of a data phase: the start of the next container.
    pending: Option<Vec<u8>>,
    open: bool,
}

impl<P: BulkPipe> Session<P> {
    /// Opens session 1, closing a stale session first if the device reports one.
    pub fn open(pipe: P) -> Result<Self, MtpError> {
        let mut session = Session {
            pipe,
            next_transaction: 0,
            pending: None,
            open: false,
        };
        match session.open_session() {
            Err(MtpError::Response {
                code: response::SESSION_ALREADY_OPEN,
                ..
            }) => {
                session.open = true;
                session.close()?;
                session.open_session()?;
            }
            other => other?,
        }
        Ok(session)
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

    fn chunk(&self) -> usize {
        let mps = self.pipe.max_packet_size().max(1);
        (READ_CHUNK / mps).max(1) * mps
    }

    fn read_transfer(&mut self) -> Result<(Vec<u8>, bool), MtpError> {
        if let Some(bytes) = self.pending.take() {
            return Ok((bytes, false));
        }
        let len = self.chunk();
        let bytes = self.pipe.read(len)?;
        // A full transfer did not end on a short packet; a zero-length packet may follow.
        let full = bytes.len() == len;
        Ok((bytes, full))
    }

    /// Runs one transaction: command, optional data out, optional data in, response.
    /// Returns the response parameters and any data the device sent.
    fn transaction(
        &mut self,
        code: u16,
        params: &[u32],
        data_out: Option<&[u8]>,
    ) -> Result<(Vec<u32>, Vec<u8>), MtpError> {
        let tid = self.next_transaction;
        self.next_transaction = self.next_transaction.wrapping_add(1).max(1);
        self.pipe.write(&ptp::encode_command(code, tid, params))?;
        if let Some(payload) = data_out {
            let container = ptp::encode_data(code, tid, payload)?;
            self.pipe.write(&container)?;
            if container
                .len()
                .is_multiple_of(self.pipe.max_packet_size().max(1))
            {
                self.pipe.write(&[])?;
            }
        }

        let mut data = Vec::new();
        let (mut first, mut full) = self.read_transfer()?;
        while first.is_empty() {
            // A stray zero-length packet; the container follows.
            (first, full) = self.read_transfer()?;
        }
        let mut header = Header::parse(&first)?;
        if header.kind == DATA {
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
                let (more, more_full) = self.read_transfer()?;
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
                let (next, _) = self.read_transfer()?;
                if !next.is_empty() {
                    self.pending = Some(next);
                }
            }
            data = buf.split_off(HEADER_LEN);
            let (next, _) = self.read_transfer()?;
            first = next;
            header = Header::parse(&first)?;
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

impl<P: BulkPipe> Drop for Session<P> {
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

impl<P: BulkPipe> Mtp for Session<P> {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        let (_, data) = self.transaction(op::GET_DEVICE_INFO, &[], None)?;
        Ok(DeviceInfo::decode(&data)?)
    }

    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError> {
        let (_, data) = self.transaction(op::GET_STORAGE_IDS, &[], None)?;
        Ok(ptp::Reader::new(&data).u32_array()?)
    }

    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let (_, data) = self.transaction(op::GET_OBJECT_HANDLES, &[storage, 0, parent], None)?;
        Ok(ptp::Reader::new(&data).u32_array()?)
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError> {
        let (_, data) = self.transaction(op::GET_OBJECT_INFO, &[handle], None)?;
        Ok(ObjectInfo::decode(&data)?)
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        Ok(self.transaction(op::GET_OBJECT, &[handle], None)?.1)
    }

    fn send_object_info(
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

    fn send_object(&mut self, data: &[u8]) -> Result<(), MtpError> {
        self.transaction(op::SEND_OBJECT, &[], Some(data)).map(drop)
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

    /// A pipe that checks each OUT transfer against a script and replays IN transfers.
    /// IN transfers are given as the packets the device sends; one read collects packets until
    /// a short one or the requested length, as USB does.
    struct ScriptedPipe {
        mps: usize,
        expect_out: VecDeque<Vec<u8>>,
        in_packets: VecDeque<Vec<u8>>,
    }

    impl ScriptedPipe {
        fn new(mps: usize) -> Self {
            ScriptedPipe {
                mps,
                expect_out: VecDeque::new(),
                in_packets: VecDeque::new(),
            }
        }

        fn expect(&mut self, bytes: Vec<u8>) {
            self.expect_out.push_back(bytes);
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
    }

    impl BulkPipe for &mut ScriptedPipe {
        fn max_packet_size(&self) -> usize {
            self.mps
        }

        fn write(&mut self, data: &[u8]) -> Result<(), MtpError> {
            let expected = self
                .expect_out
                .pop_front()
                .expect("unexpected OUT transfer");
            assert_eq!(data, expected.as_slice());
            Ok(())
        }

        fn read(&mut self, len: usize) -> Result<Vec<u8>, MtpError> {
            assert_eq!(len % self.mps, 0);
            let mut out = Vec::new();
            loop {
                let Some(packet) = self.in_packets.pop_front() else {
                    return Err(MtpError::Transport("timed out".into()));
                };
                let short = packet.len() < self.mps;
                out.extend_from_slice(&packet);
                if short || out.len() == len {
                    return Ok(out);
                }
            }
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

    fn opened(pipe: &mut ScriptedPipe) -> Session<&mut ScriptedPipe> {
        pipe.expect(ptp::encode_command(op::OPEN_SESSION, 0, &[1]));
        pipe.device_sends(&ok(0, &[]), true);
        Session::open(pipe).unwrap()
    }

    fn close(session: &mut Session<&mut ScriptedPipe>, tid: u32) {
        session
            .pipe
            .expect(ptp::encode_command(op::CLOSE_SESSION, tid, &[]));
        session.pipe.device_sends(&ok(tid, &[]), true);
        session.close().unwrap();
        assert!(session.pipe.expect_out.is_empty() && session.pipe.in_packets.is_empty());
    }

    #[test]
    fn multi_packet_data_ending_on_packet_boundary_with_zlp() {
        let mut pipe = ScriptedPipe::new(16);
        let mut s = opened(&mut pipe);
        let object: Vec<u8> = (0..52u8).collect(); // 12 + 52 = 64 bytes: four full packets
        s.pipe
            .expect(ptp::encode_command(op::GET_OBJECT, 1, &[0x0100_0010]));
        s.pipe
            .device_sends(&ptp::encode_data(op::GET_OBJECT, 1, &object).unwrap(), true);
        s.pipe.device_sends(&ok(1, &[]), true);
        assert_eq!(s.get_object(0x0100_0010).unwrap(), object);
        close(&mut s, 2);
    }

    #[test]
    fn data_on_packet_boundary_without_zlp_keeps_the_response() {
        let mut pipe = ScriptedPipe::new(16);
        let mut s = opened(&mut pipe);
        let object = vec![0xAA; 20]; // 32 bytes with the header
        s.pipe.expect(ptp::encode_command(op::GET_OBJECT, 1, &[5]));
        s.pipe.device_sends(
            &ptp::encode_data(op::GET_OBJECT, 1, &object).unwrap(),
            false,
        );
        s.pipe.device_sends(&ok(1, &[]), true);
        assert_eq!(s.get_object(5).unwrap(), object);
        close(&mut s, 2);
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
        let container = ptp::encode_data(op::GET_STORAGE_IDS, 1, &payload).unwrap();
        s.pipe
            .expect(ptp::encode_command(op::GET_STORAGE_IDS, 1, &[]));
        s.pipe
            .in_packets
            .push_back(container[..HEADER_LEN].to_vec());
        s.pipe
            .in_packets
            .push_back(container[HEADER_LEN..].to_vec());
        s.pipe.device_sends(&ok(1, &[]), true);
        assert_eq!(s.storage_ids().unwrap(), ids);
        close(&mut s, 2);
    }

    #[test]
    fn error_response_instead_of_data() {
        let mut pipe = ScriptedPipe::new(512);
        let mut s = opened(&mut pipe);
        s.pipe
            .expect(ptp::encode_command(op::GET_OBJECT_INFO, 1, &[0x0100_0001]));
        s.pipe.device_sends(
            &Response {
                code: 0x2009,
                transaction: 1,
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
        close(&mut s, 2);
    }

    #[test]
    fn send_object_info_and_object_with_zlp() {
        let mut pipe = ScriptedPipe::new(16);
        let mut s = opened(&mut pipe);
        let info = ObjectInfo {
            format: ptp::format::UNDEFINED,
            compressed_size: 4,
            filename: "f".into(),
            ..Default::default()
        };
        s.pipe.expect(ptp::encode_command(
            op::SEND_OBJECT_INFO,
            1,
            &[0x0002_0001, 0x0100_0009],
        ));
        s.pipe
            .expect(ptp::encode_data(op::SEND_OBJECT_INFO, 1, &info.encode().unwrap()).unwrap());
        s.pipe
            .device_sends(&ok(1, &[0x0002_0001, 0x0100_0009, 0x0100_0100]), true);
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
        s.pipe.expect(ptp::encode_command(op::SEND_OBJECT, 2, &[]));
        s.pipe
            .expect(ptp::encode_data(op::SEND_OBJECT, 2, &data).unwrap());
        s.pipe.expect(Vec::new());
        s.pipe.device_sends(&ok(2, &[]), true);
        s.send_object(&data).unwrap();
        close(&mut s, 3);
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
        let mut s = Session::open(&mut pipe).unwrap();
        close(&mut s, 1);
    }

    #[test]
    fn mismatched_transaction_and_truncated_data_are_errors() {
        let mut pipe = ScriptedPipe::new(512);
        let mut s = opened(&mut pipe);
        s.pipe
            .expect(ptp::encode_command(op::DELETE_OBJECT, 1, &[9, 0]));
        s.pipe.device_sends(&ok(7, &[]), true);
        assert!(matches!(s.delete_object(9), Err(MtpError::Protocol(_))));

        s.pipe.expect(ptp::encode_command(op::GET_OBJECT, 2, &[9]));
        let container = ptp::encode_data(op::GET_OBJECT, 2, &[1; 100]).unwrap();
        s.pipe.device_sends(&container[..50], true);
        s.pipe.in_packets.push_back(Vec::new());
        assert!(matches!(s.get_object(9), Err(MtpError::Protocol(_))));
        s.open = false;
    }
}
