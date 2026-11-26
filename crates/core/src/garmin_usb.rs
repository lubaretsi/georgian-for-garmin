//! Garmin's own USB protocol, as a watch in Garmin mode (USB `091e:0003`) speaks it, and the one
//! exchange this program makes with it: Garmin Express's request to come back as MTP.
//!
//! A packet is a 12-byte header {u8 layer: 0 USB, 20 application; 3 reserved bytes; u16 id;
//! 2 reserved bytes; u32 data size}, then the data, little-endian. Packets go out on bulk OUT.
//! The watch answers on interrupt IN (64-byte reads, joined until a shorter one) and announces
//! packets waiting on bulk IN with Data Available, which are read until a zero-length read.
//!
//! Privacy: a watch in Garmin mode also sends packets nobody asked for, among them its position
//! and the time. A packet keeps its data only when [`kept`] says so (product data, the protocol
//! list, the locked reply and the answers to the switch); every other one keeps its layer, id and
//! size, so its data can't reach a trace, an output or the diagnostic info.

use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

use crate::error::{Error, ErrorCode};
use crate::garmin_mode::Product;
use crate::mtp::TransportError;

pub(crate) const USB_LAYER: u8 = 0;
pub(crate) const APP_LAYER: u8 = 20;
const HEADER: usize = 12;
/// Larger than any packet this exchange expects; a header claiming more is not a header.
const MAX_DATA: u32 = 0x1_0000;

// USB-layer ids.
pub(crate) const DATA_AVAILABLE: u16 = 2;
pub(crate) const START_SESSION: u16 = 5;
pub(crate) const SESSION_STARTED: u16 = 6;
// Application-layer ids.
pub(crate) const PRODUCT_REQUEST: u16 = 249;
pub(crate) const PROTOCOL_ARRAY: u16 = 253;
pub(crate) const PRODUCT_DATA: u16 = 255;
/// Garmin Express's request to come back as MTP ("to mass storage"), with one byte 0.
pub(crate) const TO_MTP: u16 = 1071;
/// What the watch sent right after [`TO_MTP`] when it was seen switching (unnamed).
pub(crate) const TO_MTP_ANSWER: u16 = 6402;
pub(crate) const LOCKED_STATUS: u16 = 8843;

/// The protocol a watch lists when it takes [`TO_MTP`].
pub(crate) const SWITCH_PROTOCOL: &str = "A1050";

/// Start Session: tries, and how long each waits for Session Started.
const SESSION_TRIES: usize = 3;
const SESSION_TIMEOUT: Duration = Duration::from_secs(1);
/// How long the locked query and the product request wait for their answer.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);
const BULK_TIMEOUT: Duration = Duration::from_secs(3);
const WRITE_TIMEOUT: Duration = Duration::from_secs(3);
/// How long the watch's answers to the switch are read (for the trace) before letting it go.
const AFTER_SWITCH: Duration = Duration::from_secs(1);

/// A claimed device in Garmin mode: its bulk OUT, interrupt IN and bulk IN endpoints, and the
/// clock their timeouts run on.
pub(crate) trait Link {
    /// Bulk OUT's maximum packet size: a write of a multiple of it is followed by a zero-length
    /// packet.
    fn max_packet_size(&self) -> usize;
    /// Interrupt IN's read size; a read this long continues in the next one.
    fn interrupt_size(&self) -> usize;
    fn write(&mut self, data: &[u8], timeout: Duration) -> Result<(), TransportError>;
    /// One interrupt IN read of up to [`Link::interrupt_size`] bytes.
    fn read_interrupt(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError>;
    /// One bulk IN read.
    fn read_bulk(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError>;
    fn now(&self) -> Instant;
}

/// A packet's bytes.
pub(crate) fn packet(layer: u8, id: u16, data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER + data.len());
    bytes.extend_from_slice(&[layer, 0, 0, 0]);
    bytes.extend_from_slice(&id.to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(data);
    bytes
}

/// Whether a packet keeps its data: only those this exchange reads.
pub(crate) fn kept(layer: u8, id: u16) -> bool {
    layer == APP_LAYER
        && matches!(
            id,
            PRODUCT_DATA | PROTOCOL_ARRAY | LOCKED_STATUS | TO_MTP_ANSWER
        )
}

/// A received packet. Its data is empty unless [`kept`]; `size` is what the header declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Packet {
    pub layer: u8,
    pub id: u16,
    pub size: u32,
    data: Vec<u8>,
}

impl Packet {
    fn is(&self, layer: u8, id: u16) -> bool {
        self.layer == layer && self.id == id
    }
}

impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let layer = if self.layer == USB_LAYER {
            "usb"
        } else {
            "app"
        };
        write!(f, "{layer} {} ({} bytes", self.id, self.size)?;
        if kept(self.layer, self.id) {
            write!(f, "): {:02x?}", self.data)
        } else if self.size > 0 {
            f.write_str(", not kept)")
        } else {
            f.write_str(")")
        }
    }
}

/// Moves the complete packets at the start of `buffer` to `out`, each with only its kept data. A
/// header that can't be one (another layer, reserved bytes set, an implausible size) drops the
/// rest of the buffer: the stream can't be followed past it.
fn take_packets(buffer: &mut Vec<u8>, out: &mut VecDeque<Packet>) {
    let mut at = 0;
    while buffer.len() - at >= HEADER {
        let h = &buffer[at..at + HEADER];
        let layer = h[0];
        let id = u16::from_le_bytes([h[4], h[5]]);
        let size = u32::from_le_bytes([h[8], h[9], h[10], h[11]]);
        if !matches!(layer, USB_LAYER | APP_LAYER)
            || h[1] | h[2] | h[3] | h[6] | h[7] != 0
            || size > MAX_DATA
        {
            crate::trace!(
                "  Garmin mode: {} bytes that aren't a packet, dropped",
                buffer.len() - at
            );
            buffer.clear();
            return;
        }
        let end = at + HEADER + size as usize;
        if buffer.len() < end {
            break;
        }
        let data = if kept(layer, id) {
            buffer[at + HEADER..end].to_vec()
        } else {
            Vec::new()
        };
        out.push_back(Packet {
            layer,
            id,
            size,
            data,
        });
        at = end;
    }
    buffer.drain(..at);
}

/// Pid_Product_Data: u16 product ID, s16 software version × 100, then NUL-terminated strings,
/// of which the first, the description, is kept.
fn product(data: &[u8]) -> Option<Product> {
    let [a, b, c, d, rest @ ..] = data else {
        return None;
    };
    let product_id = u16::from_le_bytes([*a, *b]);
    let version = i16::from_le_bytes([*c, *d]);
    let description = rest
        .split(|&b| b == 0)
        .next()
        .unwrap_or_default()
        .iter()
        .map(|&b| char::from(b))
        .collect::<String>()
        .trim()
        .to_owned();
    Some(Product {
        product_id,
        software_version: format!("{}.{:02}", version / 100, (version % 100).abs()),
        description,
    })
}

/// Pid_Protocol_Array: 3-byte entries {tag, u16 number}, e.g. `A` 1050 as "A1050".
fn protocols(data: &[u8]) -> Vec<String> {
    data.chunks_exact(3)
        .filter(|e| e[0].is_ascii_uppercase())
        .map(|e| {
            format!(
                "{}{:03}",
                char::from(e[0]),
                u16::from_le_bytes([e[1], e[2]])
            )
        })
        .collect()
}

fn transport_error(e: TransportError) -> Error {
    match e {
        TransportError::Disconnected => Error::new(ErrorCode::NoDevice, "The watch disconnected."),
        e => Error::new(
            ErrorCode::DeviceError,
            format!("USB transfer with the watch in Garmin mode failed: {e}"),
        ),
    }
}

/// What the watch said about itself before the switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identified {
    pub product: Option<Product>,
    pub protocols: Vec<String>,
    /// When the switch went out.
    pub sent: Instant,
}

/// One exchange with a watch in Garmin mode.
struct Conversation<L> {
    link: L,
    /// Interrupt IN reads joined until a short one.
    interrupt: Vec<u8>,
    /// Bulk IN bytes not yet a whole packet.
    bulk: Vec<u8>,
    queue: VecDeque<Packet>,
    product: Option<Product>,
    /// The trace lines, kept for the tests' privacy checks.
    #[cfg(test)]
    notes: Vec<String>,
}

impl<L: Link> Conversation<L> {
    fn new(link: L) -> Self {
        Conversation {
            link,
            interrupt: Vec::new(),
            bulk: Vec::new(),
            queue: VecDeque::new(),
            product: None,
            #[cfg(test)]
            notes: Vec::new(),
        }
    }

    fn note(&mut self, line: String) {
        crate::trace!("Garmin mode: {line}");
        #[cfg(test)]
        self.notes.push(line);
    }

    fn send(&mut self, layer: u8, id: u16, data: &[u8]) -> Result<(), Error> {
        let bytes = packet(layer, id, data);
        self.note(format!("send {bytes:02x?}"));
        self.link
            .write(&bytes, WRITE_TIMEOUT)
            .map_err(transport_error)?;
        if bytes
            .len()
            .is_multiple_of(self.link.max_packet_size().max(1))
        {
            self.link
                .write(&[], WRITE_TIMEOUT)
                .map_err(transport_error)?;
        }
        Ok(())
    }

    /// After Data Available: bulk IN until a zero-length read (or none within its timeout).
    fn read_bulk(&mut self) -> Result<(), Error> {
        loop {
            match self.link.read_bulk(BULK_TIMEOUT) {
                Ok(chunk) if chunk.is_empty() => return Ok(()),
                Ok(chunk) => {
                    self.bulk.extend_from_slice(&chunk);
                    take_packets(&mut self.bulk, &mut self.queue);
                }
                Err(TransportError::TimedOut) => return Ok(()),
                Err(e) => return Err(transport_error(e)),
            }
        }
    }

    /// The next packet before `deadline`, or none.
    fn next(&mut self, deadline: Instant) -> Result<Option<Packet>, Error> {
        loop {
            if let Some(packet) = self.queue.pop_front() {
                self.note(format!("received {packet}"));
                return Ok(Some(packet));
            }
            let now = self.link.now();
            if now >= deadline {
                return Ok(None);
            }
            match self.link.read_interrupt(deadline - now) {
                Ok(chunk) => {
                    let full = chunk.len() >= self.link.interrupt_size();
                    self.interrupt.extend_from_slice(&chunk);
                    if full {
                        continue;
                    }
                    let mut packets = VecDeque::new();
                    take_packets(&mut self.interrupt, &mut packets);
                    // A short read ends what the interrupt endpoint had to say.
                    self.interrupt.clear();
                    for packet in packets {
                        if packet.is(USB_LAYER, DATA_AVAILABLE) {
                            self.read_bulk()?;
                        } else {
                            self.queue.push_back(packet);
                        }
                    }
                }
                Err(TransportError::TimedOut) => {}
                Err(e) => return Err(transport_error(e)),
            }
        }
    }

    /// The first packet `layer`/`id` within `timeout`; product data met on the way is kept.
    fn wait_for(&mut self, layer: u8, id: u16, timeout: Duration) -> Result<Option<Packet>, Error> {
        let deadline = self.link.now() + timeout;
        while let Some(packet) = self.next(deadline)? {
            if packet.is(APP_LAYER, PRODUCT_DATA) {
                self.product = product(&packet.data);
            }
            if packet.is(layer, id) {
                return Ok(Some(packet));
            }
        }
        Ok(None)
    }

    fn start_session(&mut self) -> Result<(), Error> {
        for attempt in 1..=SESSION_TRIES {
            self.send(USB_LAYER, START_SESSION, &[])?;
            // Its data, the unit ID, isn't kept.
            if self
                .wait_for(USB_LAYER, SESSION_STARTED, SESSION_TIMEOUT)?
                .is_some()
            {
                return Ok(());
            }
            self.note(format!("no Session Started (try {attempt})"));
        }
        Err(Error::new(
            ErrorCode::DeviceError,
            format!(
                "The watch in Garmin mode doesn't answer (no session after {SESSION_TRIES} tries). \
                 Unplug it and connect it again, or choose MTP on the watch when it asks."
            ),
        ))
    }

    /// No answer within the timeout counts as not locked, as for Garmin Express.
    fn locked(&mut self) -> Result<bool, Error> {
        self.send(APP_LAYER, LOCKED_STATUS, &[])?;
        Ok(
            match self.wait_for(APP_LAYER, LOCKED_STATUS, ANSWER_TIMEOUT)? {
                Some(reply) => reply.data.first().is_some_and(|&b| b != 0),
                None => {
                    self.note("no answer to the locked query: not locked".into());
                    false
                }
            },
        )
    }

    fn protocols(&mut self) -> Result<Vec<String>, Error> {
        self.send(APP_LAYER, PRODUCT_REQUEST, &[])?;
        match self.wait_for(APP_LAYER, PROTOCOL_ARRAY, ANSWER_TIMEOUT)? {
            Some(reply) => Ok(protocols(&reply.data)),
            None => Err(Error::new(
                ErrorCode::DeviceError,
                format!(
                    "The watch in Garmin mode didn't send its product data within {} s. Unplug \
                     it and connect it again, or choose MTP on the watch when it asks.",
                    ANSWER_TIMEOUT.as_secs()
                ),
            )),
        }
    }
}

/// Garmin Express's sequence: Start Session, the locked query, the product request, then the
/// switch (1071 with one byte 0) only if the watch lists A1050. `cancelled` is asked just before
/// the switch; nothing before it changes the watch. Never retried here.
pub(crate) fn request_mtp<L: Link>(
    link: L,
    cancelled: &dyn Fn() -> bool,
) -> Result<Identified, Error> {
    request_mtp_noted(link, cancelled).0
}

fn request_mtp_noted<L: Link>(
    link: L,
    cancelled: &dyn Fn() -> bool,
) -> (Result<Identified, Error>, Conversation<L>) {
    let mut talk = Conversation::new(link);
    let result = run(&mut talk, cancelled);
    (result, talk)
}

fn run<L: Link>(
    talk: &mut Conversation<L>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Identified, Error> {
    talk.start_session()?;
    if talk.locked()? {
        return Err(Error::new(
            ErrorCode::DeviceLocked,
            "The watch reports that it is locked, so it wasn't switched to file transfer (MTP). \
             Unlock it, then choose MTP on the watch when it asks, or unplug it and connect it \
             again.",
        ));
    }
    let protocols = talk.protocols()?;
    if !protocols.iter().any(|p| p == SWITCH_PROTOCOL) {
        return Err(Error::new(
            ErrorCode::UnsupportedDevice,
            format!(
                "The watch can't be switched to file transfer (MTP) over USB: it doesn't list \
                 {SWITCH_PROTOCOL} (it lists {}). On the watch, choose MTP when it asks, or set \
                 Settings › System › USB Mode › MTP.",
                if protocols.is_empty() {
                    "none".to_owned()
                } else {
                    protocols.join(", ")
                }
            ),
        ));
    }
    if cancelled() {
        return Err(Error::new(
            ErrorCode::Cancelled,
            "Cancelled before the watch was asked to switch.",
        ));
    }
    talk.send(APP_LAYER, TO_MTP, &[0])?;
    let sent = talk.link.now();
    // The watch answers and leaves USB; only the trace reads what it says meanwhile.
    let deadline = sent + AFTER_SWITCH;
    while let Ok(Some(_)) = talk.next(deadline) {}
    Ok(Identified {
        product: talk.product.take(),
        protocols,
        sent,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// The clock a fake link and a fake bus share.
    pub(crate) type Clock = Rc<Cell<Duration>>;

    /// What the fake watch does when a packet arrives.
    #[derive(Clone, Default)]
    pub(crate) struct Script {
        /// Answers Start Session on the first try of these (1-based); empty: never.
        pub session_on_try: Vec<usize>,
        /// The locked query's reply data; None: no answer.
        pub locked_reply: Option<Vec<u8>>,
        /// Answers the product request with product data and this protocol array; None: no answer.
        pub protocols: Option<Vec<(u8, u16)>>,
        /// Packets the watch sends unasked after Session Started, on bulk IN.
        pub unasked: Vec<Vec<u8>>,
        /// Fails every transfer once the switch was sent (the watch left USB).
        pub gone_after_switch: bool,
    }

    /// Venu 2's answers as seen switching.
    pub(crate) fn venu2() -> Script {
        Script {
            session_on_try: vec![1],
            locked_reply: None,
            protocols: Some(vec![
                (b'P', 0),
                (b'L', 1),
                (b'A', 10),
                (b'A', 903),
                (b'A', 1050),
            ]),
            unasked: Vec::new(),
            gone_after_switch: true,
        }
    }

    /// A position packet (Pid_Pvt_Data 51) and a time one (14) with recognisable data.
    pub(crate) const SECRET: [u8; 8] = [0x5e, 0xc2, 0xe7, 0x10, 0xca, 0x7e, 0xd0, 0x0d];
    pub(crate) fn private_packets() -> Vec<Vec<u8>> {
        vec![
            packet(APP_LAYER, 51, &SECRET),
            packet(APP_LAYER, 153, &SECRET),
            packet(APP_LAYER, 14, &SECRET),
            packet(APP_LAYER, 38, &SECRET),
        ]
    }

    pub(crate) struct FakeLink {
        pub clock: Clock,
        pub script: Script,
        /// Every write, in order.
        pub written: Rc<std::cell::RefCell<Vec<Vec<u8>>>>,
        interrupt: VecDeque<Vec<u8>>,
        bulk: VecDeque<Vec<u8>>,
        sessions: usize,
        switched: bool,
    }

    impl FakeLink {
        pub(crate) fn new(clock: Clock, script: Script) -> Self {
            FakeLink {
                clock,
                script,
                written: Default::default(),
                interrupt: VecDeque::new(),
                bulk: VecDeque::new(),
                sessions: 0,
                switched: false,
            }
        }

        /// Queues `bytes` on bulk IN with a Data Available, as the watch sends application
        /// packets.
        fn on_bulk(&mut self, bytes: Vec<u8>) {
            self.interrupt
                .push_back(packet(USB_LAYER, DATA_AVAILABLE, &[]));
            self.bulk.push_back(bytes);
            self.bulk.push_back(Vec::new());
        }

        fn gone(&self) -> bool {
            self.switched && self.script.gone_after_switch
        }
    }

    impl Link for FakeLink {
        fn max_packet_size(&self) -> usize {
            512
        }

        fn interrupt_size(&self) -> usize {
            64
        }

        fn write(&mut self, data: &[u8], _: Duration) -> Result<(), TransportError> {
            if self.gone() {
                return Err(TransportError::Disconnected);
            }
            self.written.borrow_mut().push(data.to_vec());
            if data.len() < HEADER {
                return Ok(());
            }
            let id = u16::from_le_bytes([data[4], data[5]]);
            match (data[0], id) {
                (USB_LAYER, START_SESSION) => {
                    self.sessions += 1;
                    if self.script.session_on_try.contains(&self.sessions) {
                        // Session Started with a unit ID, which must not be kept.
                        self.interrupt
                            .push_back(packet(USB_LAYER, SESSION_STARTED, &SECRET[..4]));
                        for p in self.script.unasked.clone() {
                            self.on_bulk(p);
                        }
                    }
                }
                (APP_LAYER, LOCKED_STATUS) => {
                    if let Some(reply) = self.script.locked_reply.clone() {
                        self.on_bulk(packet(APP_LAYER, LOCKED_STATUS, &reply));
                    }
                }
                (APP_LAYER, PRODUCT_REQUEST) => {
                    if let Some(list) = self.script.protocols.clone() {
                        self.on_bulk(packet(APP_LAYER, 248, b"GPS V6.60\0"));
                        let mut data = vec![0x77, 0x0e, 0x71, 0x07];
                        data.extend_from_slice(b"VENU 2 V19.05\0");
                        self.on_bulk(packet(APP_LAYER, PRODUCT_DATA, &data));
                        let entries: Vec<u8> = list
                            .iter()
                            .flat_map(|&(tag, n)| [tag, n as u8, (n >> 8) as u8])
                            .collect();
                        self.on_bulk(packet(APP_LAYER, PROTOCOL_ARRAY, &entries));
                    }
                }
                (APP_LAYER, TO_MTP) => {
                    self.switched = true;
                    if !self.script.gone_after_switch {
                        for b in [0, 1, 0] {
                            self.on_bulk(packet(APP_LAYER, TO_MTP_ANSWER, &[b]));
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        }

        fn read_interrupt(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
            if self.gone() {
                return Err(TransportError::Disconnected);
            }
            match self.interrupt.pop_front() {
                Some(bytes) => {
                    self.clock.set(self.clock.get() + Duration::from_millis(1));
                    Ok(bytes)
                }
                None => {
                    self.clock.set(self.clock.get() + timeout);
                    Err(TransportError::TimedOut)
                }
            }
        }

        fn read_bulk(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
            match self.bulk.pop_front() {
                Some(bytes) => Ok(bytes),
                None => {
                    self.clock.set(self.clock.get() + timeout);
                    Err(TransportError::TimedOut)
                }
            }
        }

        fn now(&self) -> Instant {
            start() + self.clock.get()
        }
    }

    /// The instant fake clocks count from.
    pub(crate) fn start() -> Instant {
        static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
        *START
    }

    fn talk(
        script: Script,
    ) -> (
        Result<Identified, Error>,
        Vec<Vec<u8>>,
        Vec<String>,
        Duration,
    ) {
        let clock = Clock::default();
        let link = FakeLink::new(clock.clone(), script);
        let written = link.written.clone();
        let (result, talk) = request_mtp_noted(link, &|| false);
        let written = written.borrow().clone();
        (result, written, talk.notes, clock.get())
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn packets_are_garmin_s_framing() {
        assert_eq!(
            hex(&packet(USB_LAYER, START_SESSION, &[])),
            "000000000500000000000000"
        );
        assert_eq!(
            hex(&packet(APP_LAYER, LOCKED_STATUS, &[])),
            "140000008b22000000000000"
        );
        assert_eq!(
            hex(&packet(APP_LAYER, PRODUCT_REQUEST, &[])),
            "14000000f900000000000000"
        );
        assert_eq!(
            hex(&packet(APP_LAYER, TO_MTP, &[0])),
            "140000002f0400000100000000"
        );
    }

    #[test]
    fn a_watch_that_lists_a1050_gets_exactly_express_s_bytes() {
        let (result, written, _, elapsed) = talk(venu2());
        let identified = result.unwrap();
        assert_eq!(
            written.iter().map(|w| hex(w)).collect::<Vec<_>>(),
            [
                "000000000500000000000000",
                "140000008b22000000000000",
                "14000000f900000000000000",
                "140000002f0400000100000000",
            ]
        );
        assert_eq!(
            identified.protocols,
            ["P000", "L001", "A010", "A903", "A1050"]
        );
        assert_eq!(
            identified.product,
            Some(Product {
                product_id: 3703,
                software_version: "19.05".into(),
                description: "VENU 2 V19.05".into(),
            })
        );
        // The locked query waited out its 5 s; the switch went out after it.
        assert!(identified.sent >= start() + ANSWER_TIMEOUT, "{elapsed:?}");
    }

    #[test]
    fn a_watch_without_a1050_is_not_switched() {
        let mut script = venu2();
        script.protocols = Some(vec![(b'P', 0), (b'L', 1), (b'A', 10)]);
        let (result, written, _, _) = talk(script);
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::UnsupportedDevice, "{error:?}");
        assert!(error.message.contains("P000, L001, A010"), "{error:?}");
        assert_eq!(written.len(), 3, "no 1071");
    }

    #[test]
    fn a_locked_watch_is_not_switched() {
        let mut script = venu2();
        script.locked_reply = Some(vec![1]);
        let (result, written, _, _) = talk(script);
        assert_eq!(result.unwrap_err().code, ErrorCode::DeviceLocked);
        assert_eq!(written.len(), 2, "no product request, no 1071");

        // A reply that says unlocked goes on at once.
        let mut script = venu2();
        script.locked_reply = Some(vec![0]);
        let (result, written, _, elapsed) = talk(script);
        result.unwrap();
        assert_eq!(written.len(), 4);
        assert!(elapsed < ANSWER_TIMEOUT, "{elapsed:?}");
    }

    #[test]
    fn start_session_is_tried_three_times() {
        let mut script = venu2();
        script.session_on_try = vec![3];
        let (result, written, _, _) = talk(script);
        result.unwrap();
        assert_eq!(written.len(), 6, "three Start Sessions, then the rest");

        let mut script = venu2();
        script.session_on_try = vec![];
        let (result, written, _, elapsed) = talk(script);
        assert_eq!(result.unwrap_err().code, ErrorCode::DeviceError);
        assert_eq!(written.len(), 3, "only Start Session");
        assert_eq!(elapsed, SESSION_TIMEOUT * 3);
    }

    #[test]
    fn no_product_data_is_a_device_error_without_the_switch() {
        let mut script = venu2();
        script.protocols = None;
        let (result, written, _, elapsed) = talk(script);
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::DeviceError, "{error:?}");
        assert_eq!(written.len(), 3, "no 1071");
        // Session Started came at once; each query waited its 5 s.
        assert_eq!(elapsed, ANSWER_TIMEOUT * 2 + Duration::from_millis(1));
    }

    #[test]
    fn cancelled_before_the_switch_sends_nothing_more() {
        let link = FakeLink::new(Clock::default(), venu2());
        let written = link.written.clone();
        let error = request_mtp(link, &|| true).unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert_eq!(written.borrow().len(), 3, "no 1071");
    }

    #[test]
    fn the_answers_to_the_switch_are_read_while_the_watch_stays() {
        let mut script = venu2();
        script.gone_after_switch = false;
        let (result, _, notes, _) = talk(script);
        result.unwrap();
        let answers: Vec<_> = notes.iter().filter(|n| n.contains("app 6402")).collect();
        assert_eq!(answers.len(), 3, "{notes:?}");
        assert!(answers[1].ends_with("[01]"), "{answers:?}");
    }

    #[test]
    fn position_time_and_identifiers_never_reach_the_trace() {
        let mut script = venu2();
        script.unasked = private_packets();
        script.gone_after_switch = false;
        let (result, _, notes, _) = talk(script);
        result.unwrap();
        let text = notes.join("\n");
        for id in [51, 153, 14, 38] {
            assert!(
                text.contains(&format!("app {id} (8 bytes, not kept)")),
                "{text}"
            );
        }
        assert!(text.contains("usb 6 (4 bytes, not kept)"), "{text}");
        for secret in [hex(&SECRET), format!("{:02x?}", &SECRET[..2])] {
            assert!(!text.contains(&secret), "{text}");
        }
        assert!(!text.contains("5e, c2"), "{text}");
    }

    #[test]
    fn interrupt_reads_join_until_a_short_one_and_bad_headers_are_dropped() {
        let mut buffer = packet(APP_LAYER, PROTOCOL_ARRAY, &[b'A', 0x1a, 0x04]);
        buffer.extend(packet(APP_LAYER, 51, &[1; 60]));
        buffer.extend_from_slice(&[0x14, 0, 0]);
        let mut out = VecDeque::new();
        take_packets(&mut buffer, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(protocols(&out[0].data), ["A1050"]);
        assert!(out[1].data.is_empty() && out[1].size == 60);
        assert_eq!(buffer, [0x14, 0, 0], "an incomplete header waits");

        let mut buffer = vec![0x99; 20];
        take_packets(&mut buffer, &mut out);
        assert!(buffer.is_empty());
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn product_data_and_protocols_parse() {
        let mut data = vec![0x77, 0x0e, 0x71, 0x07];
        data.extend_from_slice(b"VENU 2 V19.05\0second\0");
        let p = product(&data).unwrap();
        assert_eq!(
            (
                p.product_id,
                p.software_version.as_str(),
                p.description.as_str()
            ),
            (3703, "19.05", "VENU 2 V19.05")
        );
        assert_eq!(product(&[1, 2, 3]), None);
        assert_eq!(
            protocols(&[b'P', 0, 0, b'L', 1, 0, b'A', 0x87, 0x03, 0, 1, 2]),
            ["P000", "L001", "A903"]
        );
    }
}
