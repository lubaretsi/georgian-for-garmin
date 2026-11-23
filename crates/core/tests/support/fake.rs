//! A Venu 2 in memory that can be unplugged and plugged back in, shared across threads: for
//! the session, `rpc` and `mcp` tests. `ExtData` is hidden from listings, the watch lists
//! Garmin's GetObjectIds, and it holds two read-only imported fonts.
#![allow(dead_code)]

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use georgian_for_garmin_core::fonts::{Font, FontSet, Kind, Pack};
use georgian_for_garmin_core::mtp::{CreateError, Mtp, MtpError, NewObject, Phase, TransportError};
use georgian_for_garmin_core::names;
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use georgian_for_garmin_core::{Error, ErrorCode};

pub const STORAGE: u32 = 0x0002_0001;
pub const GARMIN: u32 = 0x0100_0002;
pub const XML: u32 = 0x0100_0003;
pub const EXTDATA: u32 = 0x0100_0009;
/// The imported 27 px Armenian font (stock) and an imported CJK font this build doesn't know.
pub const IMPORTED_ARMENIAN: u32 = 0x0100_0040;
pub const IMPORTED_CJK: u32 = 0x0100_0041;

pub const ARMENIAN: [&str; 4] = [
    "006-D9035-01",
    "006-D9035-02",
    "006-D9035-03",
    "006-D9035-04",
];
/// A part this build doesn't carry, declared under its short name, as Garmin names some
/// downloads.
pub const CJK: &str = "006-D8448-70";
/// The watch's identifiers, which outputs mask unless asked: its MTP serial number, its unit ID
/// and its `<Ifix>` and `<AppId>` values in GarminDevice.xml (`<AppId>` twice), and the owner's
/// name in its DeviceFriendlyName property.
pub const SERIAL: &str = "FAKESERIAL0042";
pub const UNIT_ID: &str = "3999999999";
pub const IFIX: &str = "1888888888";
pub const APP_ID: &str = "2777777777";
pub const OWNER_NAME: &str = "Ann's Venu";

fn bytes_for(part: &str, base: usize, step: u32) -> Vec<u8> {
    let seed: usize = part.bytes().map(usize::from).sum();
    (0..(base + seed % 97) as u32)
        .map(|i| ((i * step + seed as u32) % 251) as u8)
        .collect()
}

pub fn georgian(part: &str) -> Vec<u8> {
    bytes_for(part, 3_300, 7)
}

pub fn stock(part: &str) -> Vec<u8> {
    bytes_for(part, 2_070, 13)
}

/// The CJK font the watch imported.
pub fn cjk_stock() -> Vec<u8> {
    bytes_for(CJK, 5_000, 3)
}

/// One bitmap pack of the four Armenian parts.
pub fn fonts() -> FontSet {
    FontSet {
        packs: vec![Pack {
            id: "armenian".into(),
            name: "armenian pack".into(),
            kind: Kind::Bitmap,
            host: "armenian".into(),
            weight: 400,
            install: true,
            fonts: ARMENIAN
                .iter()
                .zip([21, 24, 27, 31])
                .map(|(&part, size)| Font {
                    part: part.into(),
                    kind: Kind::Bitmap,
                    size_px: Some(size),
                    version: "2.10".into(),
                    stock: Cow::Owned(stock(part)),
                    georgian: Cow::Owned(georgian(part)),
                    mark: None,
                    earlier: vec![],
                })
                .collect(),
            tested: vec![],
        }],
        watches: vec![],
    }
}

/// The part's file name in the form the watch declares: `006-D9035-03.tmp`, or `D903503.tmp`
/// with `short`.
fn declared_name(part: &str, short: bool, ext: &str) -> String {
    if short {
        format!("{}.{ext}", names::short_form(part).unwrap())
    } else {
        format!("{part}.{ext}")
    }
}

fn device_xml(short: bool) -> Vec<u8> {
    let file = |part: &str, name: &str| {
        format!(
            "<UpdateFile><PartNumber>{part}</PartNumber><Version><Major>2</Major><Minor>10</Minor></Version><Path>Garmin/ExtData</Path><FileName>{name}</FileName></UpdateFile>"
        )
    };
    let mut files: String = ARMENIAN
        .iter()
        .map(|p| file(p, &declared_name(p, short, "tmp")))
        .collect();
    files.push_str(&file(CJK, "D844870.tmp"));
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">\
         <Model><PartNumber>006-B3703-00</PartNumber><SoftwareVersion>1905</SoftwareVersion><Description>Venu 2</Description></Model>\
         <Id>{UNIT_ID}</Id><Ifix>{IFIX}</Ifix><AppId>{APP_ID}</AppId><AppId>{APP_ID}</AppId>\
         <MassStorageMode>{files}</MassStorageMode></Device>"
    )
    .into_bytes()
}

pub struct State {
    objects: BTreeMap<u32, (ObjectInfo, Vec<u8>)>,
    /// Answered by GetObjectInfo and GetObjectIds, left out of GetObjectHandles.
    hidden: BTreeSet<u32>,
    next_handle: u32,
    plugged: bool,
    /// Grows at every plug-in; a connection from an earlier one is dead.
    generation: u32,
    pub connects: usize,
    /// How long each object creation takes.
    pub write_delay: Duration,
    /// `DeleteObject` fails, as on a watch that refuses it.
    pub fail_delete: bool,
    /// Garmin's `GetObjectIds` fails for this parent.
    pub fail_object_ids_of: Option<u32>,
    /// `GetObjectInfo` fails for this handle.
    pub fail_object_info_of: Option<u32>,
    /// The nth object creation (1-based) stores other bytes than it was sent.
    pub corrupt_create_at: Option<usize>,
    creates: usize,
    /// Operations sent, by name, in order.
    pub log: Vec<String>,
}

impl State {
    fn add(&mut self, handle: u32, parent: u32, name: &str, fmt: u16, data: Vec<u8>, ro: bool) {
        let info = ObjectInfo {
            storage_id: STORAGE,
            format: fmt,
            protection_status: if ro { 1 } else { 0 },
            compressed_size: data.len() as u32,
            parent,
            filename: name.into(),
            ..ObjectInfo::default()
        };
        self.objects.insert(handle, (info, data));
    }
}

/// The watch itself: what a connection talks to.
#[derive(Clone)]
pub struct Watch(pub Arc<Mutex<State>>);

impl Watch {
    /// A Venu 2, which declares its fonts under their part numbers (`006-D9035-03.tmp`).
    pub fn venu2() -> Watch {
        Watch::declaring(false)
    }

    /// The same watch declaring the Armenian parts under their short names (`D903503.tmp`),
    /// as the Lily 2 Active declares its fonts; its import is `D903503.fnt`.
    pub fn declaring_short_names() -> Watch {
        Watch::declaring(true)
    }

    fn declaring(short: bool) -> Watch {
        let mut s = State {
            objects: BTreeMap::new(),
            hidden: BTreeSet::new(),
            next_handle: 0x0100_0100,
            plugged: true,
            generation: 0,
            connects: 0,
            write_delay: Duration::ZERO,
            fail_delete: false,
            fail_object_ids_of: None,
            fail_object_info_of: None,
            corrupt_create_at: None,
            creates: 0,
            log: Vec::new(),
        };
        s.add(GARMIN, 0, "GARMIN", format::ASSOCIATION, Vec::new(), false);
        s.add(
            XML,
            GARMIN,
            "GarminDevice.xml",
            format::UNDEFINED,
            device_xml(short),
            false,
        );
        s.add(
            EXTDATA,
            GARMIN,
            "ExtData",
            format::ASSOCIATION,
            Vec::new(),
            false,
        );
        s.hidden.insert(EXTDATA);
        let imported = [
            (
                IMPORTED_ARMENIAN,
                declared_name("006-D9035-03", short, "fnt"),
                stock("006-D9035-03"),
            ),
            (IMPORTED_CJK, "D844870.fnt".to_owned(), cjk_stock()),
        ];
        for (handle, name, data) in imported {
            s.add(handle, EXTDATA, &name, format::UNDEFINED, data, true);
            s.hidden.insert(handle);
        }
        Watch(Arc::new(Mutex::new(s)))
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    /// Opens a session, as `connect` does: `no_device` while unplugged.
    pub fn connect(&self) -> Result<Box<dyn Mtp>, Error> {
        let mut s = self.state();
        if !s.plugged {
            return Err(Error::new(
                ErrorCode::NoDevice,
                "No Garmin watch is connected.",
            ));
        }
        s.connects += 1;
        Ok(Box::new(Connection {
            watch: self.clone(),
            generation: s.generation,
            info: None,
        }))
    }

    pub fn unplug(&self) {
        self.state().plugged = false;
    }

    pub fn plug(&self) {
        let mut s = self.state();
        s.plugged = true;
        s.generation += 1;
    }

    /// A file in the normal `ExtData` listing.
    pub fn add_file(&self, name: &str, data: Vec<u8>) -> u32 {
        let mut s = self.state();
        let handle = s.next_handle;
        s.next_handle += 1;
        s.add(handle, EXTDATA, name, format::UNDEFINED, data, false);
        handle
    }

    /// A read-only file only Garmin's GetObjectIds lists, as the Venu 2 keeps its imports.
    pub fn add_hidden(&self, name: &str, data: Vec<u8>) -> u32 {
        let mut s = self.state();
        let handle = s.next_handle;
        s.next_handle += 1;
        s.add(handle, EXTDATA, name, format::UNDEFINED, data, true);
        s.hidden.insert(handle);
        handle
    }

    /// `ExtData`'s files in the normal listing: name and contents.
    pub fn listed(&self) -> Vec<(String, Vec<u8>)> {
        let s = self.state();
        s.objects
            .iter()
            .filter(|(h, (i, _))| i.parent == EXTDATA && !s.hidden.contains(h))
            .map(|(_, (i, d))| (i.filename.clone(), d.clone()))
            .collect()
    }
}

/// One session with the watch. Like both real backends, it keeps the first `device_info`
/// answer, so that call doesn't show whether the watch is still there.
struct Connection {
    watch: Watch,
    generation: u32,
    info: Option<DeviceInfo>,
}

impl Connection {
    /// The state, while this connection is alive. Each call is one transaction: with the `dev`
    /// feature it goes to the live trace (as answered OK, unless the watch is gone).
    fn live(&self, operation: u16) -> Result<MutexGuard<'_, State>, MtpError> {
        let mut s = self.watch.state();
        let result = if !s.plugged || s.generation != self.generation {
            Err(MtpError::Transport {
                operation,
                phase: Phase::Command,
                error: TransportError::Disconnected,
            })
        } else {
            s.log
                .push(georgian_for_garmin_core::ptp::operation_name(operation).to_owned());
            Ok(())
        };
        #[cfg(feature = "dev")]
        {
            use georgian_for_garmin_core::mtp::{RawResponse, traced};
            let answer = result.clone().map(|()| RawResponse {
                code: 0x2001,
                params: vec![],
                data: vec![],
            });
            let t = traced(operation, &[], 0, &answer, Duration::ZERO);
            georgian_for_garmin_core::trace::transaction(t);
        }
        result.map(|()| s)
    }
}

fn invalid_handle(operation: u16) -> MtpError {
    MtpError::Response {
        operation,
        code: 0x2009,
    }
}

impl Mtp for Connection {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        if let Some(info) = &self.info {
            return Ok(info.clone());
        }
        drop(self.live(op::GET_DEVICE_INFO)?);
        let info = DeviceInfo {
            model: "Venu 2".into(),
            serial_number: SERIAL.into(),
            operations_supported: vec![
                op::GET_DEVICE_INFO,
                op::OPEN_SESSION,
                op::CLOSE_SESSION,
                op::GET_STORAGE_IDS,
                op::GET_OBJECT_HANDLES,
                op::GET_OBJECT_INFO,
                op::GET_OBJECT,
                op::DELETE_OBJECT,
                op::SEND_OBJECT_INFO,
                op::SEND_OBJECT,
                op::GARMIN_GET_OBJECT_IDS,
            ],
            ..DeviceInfo::default()
        };
        self.info = Some(info.clone());
        Ok(info)
    }

    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError> {
        drop(self.live(op::GET_STORAGE_IDS)?);
        Ok(vec![STORAGE])
    }

    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let s = self.live(op::GET_OBJECT_HANDLES)?;
        Ok(s.objects
            .iter()
            .filter(|(h, (i, _))| {
                !s.hidden.contains(h)
                    && i.storage_id == storage
                    && match parent {
                        ALL_OBJECTS => true,
                        ROOT => i.parent == 0,
                        p => i.parent == p,
                    }
            })
            .map(|(&h, _)| h)
            .collect())
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError> {
        let s = self.live(op::GET_OBJECT_INFO)?;
        if s.fail_object_info_of == Some(handle) {
            return Err(MtpError::Response {
                operation: op::GET_OBJECT_INFO,
                code: 0x2002,
            });
        }
        s.objects
            .get(&handle)
            .map(|(i, _)| i.clone())
            .ok_or(invalid_handle(op::GET_OBJECT_INFO))
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        let s = self.live(op::GET_OBJECT)?;
        s.objects
            .get(&handle)
            .map(|(_, d)| d.clone())
            .ok_or(invalid_handle(op::GET_OBJECT))
    }

    fn create_object(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
        data: &[u8],
    ) -> Result<NewObject, CreateError> {
        let delay = self
            .live(op::SEND_OBJECT_INFO)
            .map_err(CreateError::Unconfirmed)?
            .write_delay;
        std::thread::sleep(delay);
        let mut s = self
            .live(op::SEND_OBJECT)
            .map_err(CreateError::Unconfirmed)?;
        let handle = s.next_handle;
        s.next_handle += 1;
        s.creates += 1;
        let mut stored = data.to_vec();
        if s.corrupt_create_at == Some(s.creates)
            && let Some(last) = stored.last_mut()
        {
            *last ^= 0xff;
        }
        let name = info.filename.clone();
        s.add(handle, parent, &name, format::UNDEFINED, stored, false);
        Ok(NewObject {
            storage,
            parent,
            handle,
        })
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        let mut s = self.live(op::DELETE_OBJECT)?;
        if s.fail_delete {
            return Err(MtpError::Response {
                operation: op::DELETE_OBJECT,
                code: 0x2002,
            });
        }
        s.objects
            .remove(&handle)
            .map(drop)
            .ok_or(invalid_handle(op::DELETE_OBJECT))
    }

    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let s = self.live(op::GARMIN_GET_OBJECT_IDS)?;
        if s.fail_object_ids_of == Some(parent) {
            return Err(MtpError::Response {
                operation: op::GARMIN_GET_OBJECT_IDS,
                code: 0x2002,
            });
        }
        Ok(s.hidden
            .iter()
            .copied()
            .filter(|h| {
                s.objects
                    .get(h)
                    .is_some_and(|(i, _)| i.storage_id == storage && i.parent == parent)
            })
            .collect())
    }

    #[cfg(feature = "dev")]
    fn storage_info(
        &mut self,
        storage: u32,
    ) -> Result<georgian_for_garmin_core::ptp::StorageInfo, MtpError> {
        drop(self.live(op::GET_STORAGE_INFO)?);
        if storage != STORAGE {
            return Err(MtpError::Response {
                operation: op::GET_STORAGE_INFO,
                code: 0x2008,
            });
        }
        Ok(georgian_for_garmin_core::ptp::StorageInfo {
            storage_type: 3,
            filesystem_type: 2,
            access_capability: 0,
            max_capacity: 8 << 30,
            free_space_in_bytes: 4 << 30,
            free_space_in_objects: u32::MAX,
            storage_description: "Internal Storage".into(),
            volume_identifier: String::new(),
        })
    }

    /// The reads the tests send, answered from the state; any other code is answered
    /// OperationNotSupported.
    #[cfg(feature = "dev")]
    fn ptp_transaction(
        &mut self,
        code: u16,
        params: &[u32],
    ) -> Result<georgian_for_garmin_core::mtp::RawResponse, MtpError> {
        let p = |i: usize| params.get(i).copied().unwrap_or(0);
        let array = |values: Vec<u32>| {
            let mut w = georgian_for_garmin_core::ptp::Writer::default();
            w.u32_array(&values);
            w.0
        };
        let answer = match code {
            op::GET_STORAGE_IDS => self.storage_ids().map(|ids| (vec![], array(ids))),
            op::GET_OBJECT_HANDLES => self.object_handles(p(0), p(2)).map(|h| (vec![], array(h))),
            op::GET_NUM_OBJECTS => self
                .object_handles(p(0), p(2))
                .map(|h| (vec![h.len() as u32], vec![])),
            op::GET_OBJECT_INFO => self
                .object_info(p(0))
                .and_then(|i| Ok((vec![], i.encode()?))),
            op::GET_OBJECT => self.get_object(p(0)).map(|d| (vec![], d)),
            op::GET_PARTIAL_OBJECT => self.get_object(p(0)).map(|d| {
                (
                    vec![],
                    georgian_for_garmin_core::mtp::byte_range(d, p(1), p(2)),
                )
            }),
            op::GET_DEVICE_INFO => self.device_info().and_then(|i| {
                let mut w = georgian_for_garmin_core::ptp::Writer::default();
                w.u16(i.standard_version)
                    .u32(i.vendor_extension_id)
                    .u16(i.vendor_extension_version);
                w.string(&i.vendor_extension_desc)?;
                w.u16(i.functional_mode);
                for list in [
                    &i.operations_supported,
                    &i.events_supported,
                    &i.device_properties_supported,
                    &i.capture_formats,
                    &i.playback_formats,
                ] {
                    w.u32(list.len() as u32);
                    for &code in list {
                        w.u16(code);
                    }
                }
                for s in [
                    &i.manufacturer,
                    &i.model,
                    &i.device_version,
                    &i.serial_number,
                ] {
                    w.string(s)?;
                }
                Ok((vec![], w.0))
            }),
            // DeviceFriendlyName: its value, and its description (code, string type, get-set,
            // factory default, current value, no form).
            0x1015 if p(0) == 0xD402 => self.live(code).map(|_| {
                let mut w = georgian_for_garmin_core::ptp::Writer::default();
                w.string(OWNER_NAME).unwrap();
                (vec![], w.0)
            }),
            0x1014 if p(0) == 0xD402 => self.live(code).map(|_| {
                let mut w = georgian_for_garmin_core::ptp::Writer::default();
                w.u16(0xD402).u16(0xFFFF);
                w.0.push(1);
                w.string("Venu 2").unwrap().string(OWNER_NAME).unwrap();
                w.0.push(0);
                (vec![], w.0)
            }),
            op::GARMIN_GET_OBJECT_IDS => self
                .garmin_object_ids(p(0), p(1))
                .map(|h| (vec![], array(h))),
            _ => {
                drop(self.live(code)?);
                Err(MtpError::Response {
                    operation: code,
                    code: 0x2005,
                })
            }
        };
        match answer {
            Ok((params, data)) => Ok(georgian_for_garmin_core::mtp::RawResponse {
                code: 0x2001,
                params,
                data,
            }),
            Err(MtpError::Response { code, .. }) => {
                Ok(georgian_for_garmin_core::mtp::RawResponse {
                    code,
                    params: vec![],
                    data: vec![],
                })
            }
            Err(e) => Err(e),
        }
    }
}
