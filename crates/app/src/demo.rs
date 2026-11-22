//! An in-memory watch for trying the app without hardware (`--features demo`).
//!
//! It follows a short script so every screen shows up: the first three connections find no
//! watch (meanwhile a Forerunner 245 is mounted as a USB drive, [`drive`]: `unsupported`, at a
//! software version the support list has), the next two find it held by Garmin Express, and
//! from then on one of five watches is attached, covering every support status:
//!
//! 1. a Forerunner 165 on software 29.52 with Garmin's TrueType fonts, which it shows over MTP
//!    (the real one doesn't): `tested`; after installing, the main screen says Georgian is
//!    installed;
//! 2. a Venu 2 on software 19.05, listing the parts the registry verified on it, with nothing
//!    staged: `tested`;
//! 3. a Venu 3, made to list every part of the first two bundled packs (the real one lists
//!    TrueType fonts), with a different file already staged for the last one, so installing on
//!    it ends in a `conflict`: `expected`;
//! 4. a fenix 6S Pro listing Swiss 721 Hebrew, which no build installs (the watch had our font
//!    and still drew question marks, #6): `unsupported`, with Restore;
//! 5. a Venu listing none of this build's fonts: `unsupported`.
//!
//! After every install or restore the app calls [`unplug`]: the attached watch "restarts",
//! importing (removing) every staged `.tmp` file, and the next watch is attached. So the
//! second visit to the Venu 3 installs cleanly, as a real one would once restarted.
//! `GEORGIAN_FOR_GARMIN_DEMO_WATCH` set to a watch's name attaches that one first.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use georgian_for_garmin_core::fonts::{FontSet, Kind};
use georgian_for_garmin_core::mtp::{CreateError, Mtp, MtpError, NewObject};
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use georgian_for_garmin_core::{Error, ErrorCode};

const STORAGE: u32 = 0x0002_0001;
const GARMIN: u32 = 0x0100_0001;
const XML: u32 = 0x0100_0002;
const EXTDATA: u32 = 0x0100_0003;
const LEFTOVER: u32 = 0x0100_0004;

/// Connections that find no watch, then connections that find it busy.
const ABSENT: usize = 3;
const BUSY: usize = 2;

/// What a demo watch lists under `Garmin/ExtData`.
#[derive(Clone, Copy)]
enum Lists {
    /// The parts the registry verified on this model.
    Verified,
    /// Every part of the first two bundled packs.
    FirstTwoPacks,
    /// Every part of the bundled pack with this id.
    Pack(&'static str),
    /// None of this build's parts.
    Nothing,
}

/// (description, part number, software version as in `GarminDevice.xml`, fonts listed, stage
/// a leftover).
const WATCHES: [(&str, &str, &str, Lists, bool); 5] = [
    (
        "Forerunner 165",
        "006-B4432-00",
        "2952",
        Lists::Verified,
        false,
    ),
    ("Venu 2", "006-B3703-00", "1905", Lists::Verified, false),
    ("Venu 3", "006-B4260-00", "1705", Lists::FirstTwoPacks, true),
    (
        "fenix 6S Pro",
        "006-B3288-00",
        "2802",
        Lists::Pack("hebrew-swiss-721-truetype"),
        false,
    ),
    ("Venu", "006-B3226-00", "780", Lists::Nothing, false),
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

fn device_xml(
    fonts: &FontSet,
    description: &str,
    part: &str,
    software: &str,
    parts: &[String],
) -> Vec<u8> {
    let files: String = fonts
        .fonts()
        .filter(|(_, f)| parts.contains(&f.part))
        .map(|(_, f)| {
            let (major, minor) = f.version.split_once('.').unwrap_or(("2", "10"));
            format!(
                "<UpdateFile><PartNumber>{}</PartNumber>\
                 <Version><Major>{major}</Major><Minor>{minor}</Minor></Version>\
                 <Path>Garmin/ExtData</Path><FileName>{}</FileName></UpdateFile>",
                f.part,
                f.file_name()
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

/// The parts a demo watch lists.
fn listed_parts(fonts: &FontSet, model_part: &str, lists: Lists) -> Vec<String> {
    let pack_parts = |p: &georgian_for_garmin_core::fonts::Pack| {
        p.fonts.iter().map(|f| f.part.clone()).collect::<Vec<_>>()
    };
    match lists {
        Lists::Verified => fonts
            .packs
            .iter()
            .flat_map(|p| &p.tested)
            .filter(|t| t.model_part == model_part)
            .flat_map(|t| t.parts.iter().cloned())
            .collect(),
        Lists::FirstTwoPacks => fonts.packs.iter().take(2).flat_map(pack_parts).collect(),
        Lists::Pack(id) => fonts
            .packs
            .iter()
            .filter(|p| p.id == id)
            .flat_map(pack_parts)
            .collect(),
        Lists::Nothing => Vec::new(),
    }
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
            .map(|&(description, part, software, lists, leftover)| {
                let parts = listed_parts(&fonts, part, lists);
                let mut objects = Objects::new();
                // Objects in the storage root have parent 0.
                objects.insert(GARMIN, object(0, "GARMIN", format::ASSOCIATION, vec![]));
                objects.insert(
                    XML,
                    object(
                        GARMIN,
                        "GarminDevice.xml",
                        format::UNDEFINED,
                        device_xml(&fonts, description, part, software, &parts),
                    ),
                );
                objects.insert(
                    EXTDATA,
                    object(GARMIN, "ExtData", format::ASSOCIATION, vec![]),
                );
                // Garmin's own TrueType fonts, which the watch keeps in ExtData.
                let bins = fonts
                    .fonts()
                    .filter(|(p, f)| p.kind == Kind::Truetype && parts.contains(&f.part));
                for (handle, (_, font)) in (LEFTOVER + 1..).zip(bins) {
                    let data = font.stock.to_vec();
                    objects.insert(
                        handle,
                        object(EXTDATA, &font.file_name(), format::UNDEFINED, data),
                    );
                }
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

    /// Attaches the watch named `model` next, if there is one.
    #[cfg_attr(not(feature = "demo"), allow(dead_code))]
    fn attach(&self, model: &str) -> bool {
        match WATCHES.iter().position(|w| w.0.eq_ignore_ascii_case(model)) {
            Some(index) => {
                self.state().current = index;
                true
            }
            None => false,
        }
    }

    /// Whether the script is still in its no-watch connections.
    #[cfg_attr(not(feature = "demo"), allow(dead_code))]
    fn absent(&self) -> bool {
        self.state().connects <= ABSENT
    }
}

#[cfg(feature = "demo")]
static WATCH: std::sync::LazyLock<Demo> = std::sync::LazyLock::new(|| {
    let demo = Demo::new(Duration::from_millis(300));
    if let Ok(model) = std::env::var("GEORGIAN_FOR_GARMIN_DEMO_WATCH")
        && !demo.attach(&model)
    {
        eprintln!("GEORGIAN_FOR_GARMIN_DEMO_WATCH: no demo watch is called {model:?}");
    }
    demo
});

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

/// While no demo watch is attached over MTP: a Forerunner 245 "mounted as a USB drive" (issue
/// #16), a folder in the temp directory with its `GARMIN/GarminDevice.xml`.
#[cfg(feature = "demo")]
pub fn drive() -> Option<std::path::PathBuf> {
    if !WATCH.absent() {
        return None;
    }
    let root = std::env::temp_dir().join("georgian-for-garmin-demo-drive");
    let text_files: String = (0..4)
        .map(|i| {
            format!(
                "<UpdateFile><PartNumber>006-D7547-{i:02}</PartNumber>\
                 <Version><Major>4</Major><Minor>20</Minor></Version>\
                 <Path>Garmin/Text</Path><FileName>lang{i}.ln4</FileName></UpdateFile>"
            )
        })
        .collect();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">\
         <Model><PartNumber>006-B3076-00</PartNumber><SoftwareVersion>1370</SoftwareVersion>\
         <Description>Forerunner 245</Description></Model><Id>3300000001</Id>\
         <MassStorageMode>{text_files}</MassStorageMode></Device>"
    );
    std::fs::create_dir_all(root.join("GARMIN")).ok()?;
    std::fs::write(root.join("GARMIN/GarminDevice.xml"), xml).ok()?;
    Some(root)
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
            .map(|f| f["file"].as_str().unwrap())
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

        // The Forerunner 165: Garmin's TrueType fonts get Georgian; then the main screen says
        // it is installed.
        let fr165 = call(&demo, "inspect").unwrap();
        assert_eq!(fr165["device"]["model"], "Forerunner 165");
        assert_eq!(fr165["hosts"]["kind"], "truetype");
        assert_eq!(fr165["state"], "not_installed");
        assert_eq!(staged(&fr165), vec!["stock"; 4]);
        assert_eq!(fr165["support"]["status"], "tested");
        assert_eq!(call(&demo, "install").unwrap()["status"], "staged_verified");
        let after = call(&demo, "inspect").unwrap();
        assert_eq!(after["state"], "installed");
        assert_eq!(staged(&after), vec!["georgian"; 4]);
        demo.swap();

        // The tested Venu 2: every listed part of its one pack installs.
        let ready = call(&demo, "inspect").unwrap();
        let sizes = staged(&ready).len();
        assert_eq!(sizes, 4, "the registry verified four parts on the Venu 2");
        assert_eq!(ready["device"]["model"], "Venu 2");
        assert_eq!(ready["device"]["software_version"], "19.05");
        assert_eq!(ready["supported"], true);
        assert_eq!(ready["tested"]["status"], "tested");
        assert_eq!(ready["support"]["status"], "tested");
        assert_eq!(ready["support"]["report_wanted"], false);
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

        // The Venu 3 lists two packs; a leftover file makes installing a conflict.
        let other = call(&demo, "inspect").unwrap();
        let other_sizes = staged(&other).len();
        assert_eq!(other["tested"]["status"], "untested");
        assert_eq!(other["support"]["status"], "expected");
        assert_eq!(other["support"]["issue"], 5);
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

        // The fenix 6S Pro: Swiss 721 Hebrew is restore-only in every build.
        let fenix = call(&demo, "inspect").unwrap();
        assert_eq!(fenix["device"]["model"], "fenix 6S Pro");
        assert_eq!(fenix["supported"], false);
        assert_eq!(fenix["restorable"], true);
        assert_eq!(fenix["support"]["status"], "unsupported");
        assert_eq!(fenix["support"]["issue"], 6);
        demo.swap();

        // The Venu: known not to work, on the software the list has; no report asked.
        let venu = call(&demo, "inspect").unwrap();
        assert_eq!(venu["supported"], false);
        assert_eq!(venu["support"]["status"], "unsupported");
        assert_eq!(venu["support"]["issue"], 12);
        assert_eq!(venu["support"]["report_wanted"], false);
        demo.swap();

        // The Forerunner 165 keeps its fonts across restarts.
        assert_eq!(call(&demo, "inspect").unwrap()["state"], "installed");
        demo.swap();

        // The Venu 2 imported its files on restart.
        assert_eq!(
            staged(&call(&demo, "inspect").unwrap()),
            vec!["none"; sizes]
        );
        demo.swap();

        // So did the Venu 3; now every part of both packs installs.
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
