//! An in-memory watch for trying the app without hardware (`--features demo`).
//!
//! It follows a short script so every screen shows up: the first two connections find no
//! watch, the next two find it held by Garmin Express, and from then on one of two watches is
//! attached:
//!
//! 1. a Venu 2 on software 19.05, listing the parts the registry verified on it, with nothing
//!    staged: `tested`;
//! 2. an untested model (its name and part number are made up for the demo) listing every part
//!    of the first two bundled packs, with a different file already staged for the last one, so
//!    installing on it ends in a `conflict`.
//!
//! After every install or restore the app calls [`unplug`]: the attached watch "restarts",
//! importing (removing) every staged `.tmp` file, and the other watch is attached. So the
//! second visit to the untested watch installs cleanly, as a real one would once restarted.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use georgian_for_garmin_core::fonts::FontSet;
use georgian_for_garmin_core::mtp::{CreateError, Mtp, MtpError, NewObject};
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use georgian_for_garmin_core::{Error, ErrorCode};

const STORAGE: u32 = 0x0002_0001;
const GARMIN: u32 = 0x0100_0001;
const XML: u32 = 0x0100_0002;
const EXTDATA: u32 = 0x0100_0003;
const LEFTOVER: u32 = 0x0100_0004;

/// Connections that find no watch, then connections that find it busy.
const ABSENT: usize = 2;
const BUSY: usize = 2;

/// (description, part number, software version as in `GarminDevice.xml`, stage a leftover).
const WATCHES: [(&str, &str, &str, bool); 2] = [
    ("Venu 2", "006-B3703-00", "1905", false),
    ("Venu 3", "006-B4260-00", "1209", true),
];

type Objects = BTreeMap<u32, (ObjectInfo, Vec<u8>)>;

struct State {
    connects: usize,
    /// Each watch's objects, kept while the other one is attached.
    watches: Vec<Objects>,
    current: usize,
    next_handle: u32,
    transfer: Duration,
}

impl State {
    fn objects(&mut self) -> &mut Objects {
        &mut self.watches[self.current]
    }
}

/// A handle on the shared in-memory watches.
#[derive(Clone)]
pub struct Demo(Arc<Mutex<State>>);

fn device_xml(description: &str, part: &str, software: &str, parts: &[String]) -> Vec<u8> {
    let files: String = parts
        .iter()
        .map(|p| {
            format!(
                "<UpdateFile><PartNumber>{p}</PartNumber>\
                 <Version><Major>2</Major><Minor>10</Minor></Version>\
                 <Path>Garmin/ExtData</Path><FileName>{p}.tmp</FileName></UpdateFile>"
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">\
         <Model><PartNumber>{part}</PartNumber><SoftwareVersion>{software}</SoftwareVersion>\
         <Description>{description}</Description></Model><Id>3300000000</Id>\
         <MassStorageMode>{files}</MassStorageMode></Device>"
    )
    .into_bytes()
}

/// The parts a demo watch lists: on a model the registry verified, the parts verified on it;
/// otherwise every part of the first two bundled packs.
fn listed_parts(fonts: &FontSet, model_part: &str) -> Vec<String> {
    let verified: Vec<String> = fonts
        .packs
        .iter()
        .flat_map(|p| &p.tested)
        .filter(|t| t.model_part == model_part)
        .flat_map(|t| t.parts.iter().cloned())
        .collect();
    if !verified.is_empty() {
        return verified;
    }
    fonts
        .packs
        .iter()
        .take(2)
        .flat_map(|p| p.fonts.iter().map(|f| f.part.clone()))
        .collect()
}

fn object(parent: u32, name: &str, fmt: u16, data: Vec<u8>) -> (ObjectInfo, Vec<u8>) {
    let info = ObjectInfo {
        storage_id: STORAGE,
        format: fmt,
        compressed_size: data.len() as u32,
        parent,
        filename: name.into(),
        ..ObjectInfo::default()
    };
    (info, data)
}

fn invalid_handle(operation: u16) -> MtpError {
    MtpError::Response {
        operation,
        code: 0x2009,
    }
}

impl Demo {
    pub fn new(transfer: Duration) -> Self {
        let fonts = FontSet::bundled();
        let watches = WATCHES
            .iter()
            .map(|&(description, part, software, leftover)| {
                let parts = listed_parts(&fonts, part);
                let mut objects = Objects::new();
                // Objects in the storage root have parent 0.
                objects.insert(GARMIN, object(0, "GARMIN", format::ASSOCIATION, vec![]));
                objects.insert(
                    XML,
                    object(
                        GARMIN,
                        "GarminDevice.xml",
                        format::UNDEFINED,
                        device_xml(description, part, software, &parts),
                    ),
                );
                objects.insert(
                    EXTDATA,
                    object(GARMIN, "ExtData", format::ASSOCIATION, vec![]),
                );
                if leftover {
                    let name = format!("{}.tmp", parts[parts.len() - 1]);
                    objects.insert(
                        LEFTOVER,
                        object(EXTDATA, &name, format::UNDEFINED, b"older font".to_vec()),
                    );
                }
                objects
            })
            .collect();
        Demo(Arc::new(Mutex::new(State {
            connects: 0,
            watches,
            current: 0,
            next_handle: 0x0100_0100,
            transfer,
        })))
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Opens a session, following the script.
    pub fn open(&self) -> Result<Box<dyn Mtp>, Error> {
        let mut s = self.state();
        s.connects += 1;
        if s.connects <= ABSENT {
            Err(Error::new(
                ErrorCode::NoDevice,
                "Demo: no watch is connected yet.",
            ))
        } else if s.connects <= ABSENT + BUSY {
            Err(Error::new(
                ErrorCode::DeviceBusy,
                "Demo: another program holds the watch.",
            ))
        } else {
            Ok(Box::new(self.clone()))
        }
    }

    /// The attached watch restarts, importing its staged files, and the other one is attached.
    pub fn swap(&self) {
        let mut s = self.state();
        s.objects().retain(|_, (info, _)| {
            !(info.parent == EXTDATA && info.filename.to_ascii_lowercase().ends_with(".tmp"))
        });
        s.current = (s.current + 1) % s.watches.len();
    }
}

#[cfg(feature = "demo")]
static WATCH: std::sync::LazyLock<Demo> =
    std::sync::LazyLock::new(|| Demo::new(Duration::from_millis(300)));

/// The app's demo watch. Transfers take a moment so the progress state is visible.
#[cfg(feature = "demo")]
pub fn connect() -> Result<Box<dyn Mtp>, Error> {
    WATCH.open()
}

/// Called after every install or restore: see [`Demo::swap`].
#[cfg(feature = "demo")]
pub fn unplug() {
    WATCH.swap();
}

impl Mtp for Demo {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        Ok(DeviceInfo {
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
            ],
            ..DeviceInfo::default()
        })
    }

    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError> {
        Ok(vec![STORAGE])
    }

    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        Ok(self
            .state()
            .objects()
            .iter()
            .filter(|(_, (i, _))| {
                i.storage_id == storage
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
        self.state()
            .objects()
            .get(&handle)
            .map(|(i, _)| i.clone())
            .ok_or(invalid_handle(op::GET_OBJECT_INFO))
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        self.state()
            .objects()
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
        let transfer = self.state().transfer;
        thread::sleep(transfer);
        let mut s = self.state();
        let handle = s.next_handle;
        s.next_handle += 1;
        let info = ObjectInfo {
            storage_id: storage,
            parent,
            compressed_size: data.len() as u32,
            ..info.clone()
        };
        s.objects().insert(handle, (info, data.to_vec()));
        Ok(NewObject {
            storage,
            parent,
            handle,
        })
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        self.state()
            .objects()
            .remove(&handle)
            .map(drop)
            .ok_or(invalid_handle(op::DELETE_OBJECT))
    }
}

#[cfg(test)]
mod tests {
    use georgian_for_garmin_core::ops;
    use serde_json::{Value, json};

    use super::*;

    fn call(demo: &Demo, name: &str) -> Result<Value, ErrorCode> {
        ops::call(name, json!({}), &FontSet::bundled(), &mut || demo.open()).map_err(|e| e.code)
    }

    fn staged(inspect: &Value) -> Vec<&str> {
        inspect["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["target"] == true)
            .map(|f| f["staged"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn follows_the_script_through_every_screen() {
        let demo = Demo::new(Duration::ZERO);
        for _ in 0..ABSENT {
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::NoDevice));
        }
        for _ in 0..BUSY {
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::DeviceBusy));
        }

        // The tested Venu 2: every listed part of its one pack installs.
        let ready = call(&demo, "inspect").unwrap();
        let sizes = staged(&ready).len();
        assert_eq!(sizes, 4, "the registry verified four parts on the Venu 2");
        assert_eq!(ready["device"]["model"], "Venu 2");
        assert_eq!(ready["device"]["software_version"], "19.05");
        assert_eq!(ready["supported"], true);
        assert_eq!(ready["tested"]["status"], "tested");
        assert_eq!(ready["packs"].as_array().unwrap().len(), 1);
        assert_eq!(staged(&ready), vec!["none"; sizes]);
        let installed = call(&demo, "install").unwrap();
        assert_eq!(installed["status"], "staged_verified");
        assert_eq!(
            installed["packs"][0]["parts"].as_array().unwrap().len(),
            sizes
        );
        assert_eq!(
            staged(&call(&demo, "inspect").unwrap()),
            vec!["georgian"; sizes]
        );
        assert_eq!(call(&demo, "install").unwrap()["status"], "already_staged");
        demo.swap();

        // The untested watch lists two packs; a leftover file makes installing a conflict.
        let other = call(&demo, "inspect").unwrap();
        let other_sizes = staged(&other).len();
        assert_eq!(other["tested"]["status"], "untested");
        assert_eq!(other["packs"].as_array().unwrap().len(), 2);
        assert_eq!(staged(&other).last(), Some(&"other"));
        assert!(
            !other["report"]["text"]
                .as_str()
                .unwrap()
                .contains("3300000000")
        );
        assert_eq!(call(&demo, "install"), Err(ErrorCode::Conflict));
        demo.swap();

        // The Venu 2 imported its files on restart.
        assert_eq!(
            staged(&call(&demo, "inspect").unwrap()),
            vec!["none"; sizes]
        );
        demo.swap();

        // So did the untested watch; now every part of both packs installs.
        let installed = call(&demo, "install").unwrap();
        assert_eq!(installed["status"], "staged_verified");
        let parts: usize = installed["packs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["parts"].as_array().unwrap().len())
            .sum();
        assert_eq!(parts, other_sizes);
    }
}
