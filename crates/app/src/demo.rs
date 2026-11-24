//! An in-memory watch for trying the app without hardware (`--features demo`).
//!
//! It follows a short script so the connect screens show up: the first three connections find
//! no watch (meanwhile a Forerunner 245 is mounted as a USB drive, [`drive`]: `unsupported`, at
//! a software version the support list has), the next two find it held by Garmin Express, and
//! from then on one of nine watches is attached, covering every support status:
//!
//! 1. a Venu 2 on software 19.05, listing the parts the registry verified on it, with nothing
//!    staged: `tested`, so installing ends in Georgian confirmed after the restart;
//! 2. a Forerunner 165 on software 29.52 with Garmin's TrueType fonts, which it shows over MTP
//!    (the real one doesn't): `tested`; its fonts change without a restart, so the app can't
//!    verify one and asks for the check on the watch;
//! 3. a Venu 3, made to list every part of the first two bundled packs (the real one lists
//!    TrueType fonts), with a different file already staged for the last one, so installing on
//!    it ends in a `conflict`: `expected`;
//! 4. a fenix 6S Pro listing Swiss 721 Hebrew, which no build installs (the watch had our font
//!    and still drew question marks, #6): `unsupported`, with Restore;
//! 5. a Venu listing the Korean font `006-D8945-22`, which the support list only predicts to
//!    work: `expected`, with the note that no one has confirmed Georgian on the model yet;
//! 6. a Lily 2 Active whose Korean fonts already have Georgian, imported at an earlier restart:
//!    `expected` and `installed`, so the main screen's action is to check it on the watch;
//! 7. an Instinct 3 AMOLED 50mm whose Korean font has Georgian waiting for a restart:
//!    `expected` and `waiting`, so the app asks for the restart;
//! 8. a tactix 7 on software 27.18 with Georgian imported in the parts verified on it:
//!    `tested` and `installed`, with only a quiet link to check if it didn't work;
//! 9. a Forerunner 265, made to list none of this build's fonts (the support list predicts it
//!    to work through Noto Sans Armenian): `unknown`, with the prediction and a report asked.
//!
//! After every install or restore the app calls [`written`]: the watch is "unplugged and
//! restarted", so the next [`RESTART`] connections find no watch, then the same watch is back
//! with every staged `.tmp` file imported as a hidden read-only `<part>.fnt` (replacing the
//! earlier one), which only Garmin's `GetObjectIds` lists, as the Venu 2 does. So the Venu 2
//! comes back with Georgian installed.
//! `GEORGIAN_FOR_GARMIN_DEMO_WATCH` set to a watch's name attaches that one instead of the Venu
//! 2; set to `none`, no watch is ever found, over MTP or as a drive, so the waiting screen stays
//! (and shows its help). `GEORGIAN_FOR_GARMIN_DEMO_BUSY` set to a holder kind (`garmin_express`,
//! `macos_camera_service`, `desktop_mtp`, `this_app`, `other`, `unknown`) finds the watch held
//! by such a program on every connection. `GEORGIAN_FOR_GARMIN_DEMO_PLATFORM` (`macos`,
//! `windows`, `linux`) shows another platform's words, and its holders' names.
//!
//! The rest of the app's outside world is kept here too, so the demo build touches nothing of
//! the user's: diagnostic info is saved to a folder in the temp directory ([`downloads`]), the
//! clipboard is left alone, "shared before" is kept in memory, and nothing is opened. [`probe`]
//! lists the demo watch and drive for `diagnose` instead of this computer's USB devices and
//! volumes.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use georgian_for_garmin_core::busy::{Busy, Holder, HolderKind, UsbInterface};
#[cfg(feature = "demo")]
use georgian_for_garmin_core::diagnose::{Probe, UsbDevice, UsbReport, drives_at};
use georgian_for_garmin_core::fonts::{FontSet, Kind};
use georgian_for_garmin_core::mtp::{CreateError, Mtp, MtpError, NewObject};
use georgian_for_garmin_core::ptp::{ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op};
use georgian_for_garmin_core::{Error, ErrorCode};

#[cfg(feature = "demo")]
use crate::settings::ShareKey;

const STORAGE: u32 = 0x0002_0001;
const GARMIN: u32 = 0x0100_0001;
const XML: u32 = 0x0100_0002;
const EXTDATA: u32 = 0x0100_0003;
const LEFTOVER: u32 = 0x0100_0004;

/// Connections that find no watch, then connections that find it busy.
const ABSENT: usize = 3;
const BUSY: usize = 2;
/// Connections that find no watch after a write, while it is unplugged and restarting.
pub const RESTART: usize = 3;

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

/// What a demo watch has under `Garmin/ExtData` besides Garmin's TrueType fonts.
#[derive(Clone, Copy)]
enum Extra {
    Nothing,
    /// A file this build doesn't know, waiting for a restart as the last listed part's `.tmp`.
    Leftover,
    /// This build's Georgian in every listed bitmap part, imported as a hidden `<part>.fnt`.
    Installed,
    /// This build's Georgian in every listed bitmap part, waiting as `<part>.tmp`.
    Waiting,
}

/// (description, part number, software version as in `GarminDevice.xml`, fonts listed, what
/// else is in `ExtData`).
const WATCHES: [(&str, &str, &str, Lists, Extra); 9] = [
    (
        "Venu 2",
        "006-B3703-00",
        "1905",
        Lists::Verified,
        Extra::Nothing,
    ),
    (
        "Forerunner 165",
        "006-B4432-00",
        "2952",
        Lists::Verified,
        Extra::Nothing,
    ),
    (
        "Venu 3",
        "006-B4260-00",
        "1705",
        Lists::FirstTwoPacks,
        Extra::Leftover,
    ),
    (
        "fenix 6S Pro",
        "006-B3288-00",
        "2802",
        Lists::Pack("hebrew-swiss-721-truetype"),
        Extra::Nothing,
    ),
    (
        "Venu",
        "006-B3226-00",
        "780",
        Lists::Pack("korean-nanum-gothic-medium"),
        Extra::Nothing,
    ),
    (
        "Lily 2 Active",
        "006-B4477-00",
        "510",
        Lists::Pack("korean-nanum-gothic-medium-compressed"),
        Extra::Installed,
    ),
    (
        "Instinct 3 AMOLED 50mm",
        "006-B4587-00",
        "1612",
        Lists::Pack("korean-nanum-gothic-bold"),
        Extra::Waiting,
    ),
    (
        "tactix 7",
        "006-B4135-00",
        "2718",
        Lists::Verified,
        Extra::Installed,
    ),
    (
        "Forerunner 265",
        "006-B4257-00",
        "2952",
        Lists::Nothing,
        Extra::Nothing,
    ),
];

type Objects = BTreeMap<u32, (ObjectInfo, Vec<u8>)>;

struct State {
    connects: usize,
    /// `GEORGIAN_FOR_GARMIN_DEMO_WATCH=none`: no watch, over MTP or as a drive, ever.
    unplugged: bool,
    /// What holds the watch in the script's busy connections.
    busy: Busy,
    /// `GEORGIAN_FOR_GARMIN_DEMO_BUSY`: every connection finds the watch busy.
    always_busy: bool,
    /// Connections left that find no watch while it restarts after a write.
    restarting: usize,
    /// Each watch's objects.
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
                f.long_file_name()
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

/// A restart: every staged `.tmp` file becomes a hidden read-only `<part>.fnt`, replacing the
/// earlier one.
fn import_staged(objects: &mut Objects) {
    let waiting: Vec<u32> = objects
        .iter()
        .filter(|(_, (info, _))| {
            info.parent == EXTDATA
                && info.protection_status == 0
                && info.filename.to_ascii_lowercase().ends_with(".tmp")
        })
        .map(|(&h, _)| h)
        .collect();
    for handle in waiting {
        let (mut info, data) = objects.remove(&handle).expect("listed above");
        let stem = info.filename.len() - ".tmp".len();
        info.filename = format!("{}.fnt", &info.filename[..stem]);
        info.protection_status = 1;
        objects.retain(|_, (old, _)| {
            !(old.parent == EXTDATA && old.filename.eq_ignore_ascii_case(&info.filename))
        });
        objects.insert(handle, (info, data));
    }
}

impl Demo {
    pub fn new(transfer: Duration) -> Self {
        let fonts = FontSet::bundled();
        let watches = WATCHES
            .iter()
            .map(|&(description, part, software, lists, extra)| {
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
                let mut handles = LEFTOVER + 1..;
                // Garmin's own TrueType fonts, which the watch keeps in ExtData.
                let bins = fonts
                    .fonts()
                    .filter(|(p, f)| p.kind == Kind::Truetype && parts.contains(&f.part));
                for (handle, (_, font)) in handles.by_ref().zip(bins) {
                    let data = font.stock.to_vec();
                    objects.insert(
                        handle,
                        object(EXTDATA, &font.long_file_name(), format::UNDEFINED, data),
                    );
                }
                match extra {
                    Extra::Nothing => {}
                    Extra::Leftover => {
                        let name = format!("{}.tmp", parts[parts.len() - 1]);
                        objects.insert(
                            LEFTOVER,
                            object(EXTDATA, &name, format::UNDEFINED, b"older font".to_vec()),
                        );
                    }
                    Extra::Installed | Extra::Waiting => {
                        let imported = matches!(extra, Extra::Installed);
                        let bitmaps = fonts
                            .fonts()
                            .filter(|(p, f)| p.kind == Kind::Bitmap && parts.contains(&f.part));
                        for (handle, (_, font)) in handles.zip(bitmaps) {
                            let suffix = if imported { "fnt" } else { "tmp" };
                            let name = format!("{}.{suffix}", font.part);
                            let data = font.georgian.to_vec();
                            let (mut info, data) = object(EXTDATA, &name, format::UNDEFINED, data);
                            info.protection_status = u16::from(imported);
                            objects.insert(handle, (info, data));
                        }
                    }
                }
                objects
            })
            .collect();
        Demo(Arc::new(Mutex::new(State {
            connects: 0,
            unplugged: false,
            busy: busy_sample(HolderKind::GarminExpress, "macos"),
            always_busy: false,
            restarting: 0,
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
        if s.always_busy {
            Err(Error::busy(s.busy.clone()))
        } else if s.unplugged || s.connects <= ABSENT {
            Err(Error::new(
                ErrorCode::NoDevice,
                "Demo: no watch is connected yet.",
            ))
        } else if s.restarting > 0 {
            s.restarting -= 1;
            if s.restarting == 0 {
                import_staged(s.objects());
            }
            Err(Error::new(
                ErrorCode::NoDevice,
                "Demo: the watch is restarting.",
            ))
        } else if s.connects <= ABSENT + BUSY {
            Err(Error::busy(s.busy.clone()))
        } else {
            Ok(Box::new(self.clone()))
        }
    }

    /// After a write: the watch is unplugged and restarted, and comes back after [`RESTART`]
    /// connections with its staged files imported.
    pub fn restart(&self) {
        self.state().restarting = RESTART;
    }

    /// The demo watch as `diagnose` lists it on USB: absent while unplugged or restarting, and
    /// with its holder while the next connection finds it busy.
    #[cfg_attr(not(feature = "demo"), allow(dead_code))]
    fn usb(&self) -> Option<Option<Holder>> {
        let s = self.state();
        let held = s.always_busy || (ABSENT..ABSENT + BUSY).contains(&s.connects);
        if held {
            Some(Some(s.busy.holder.clone()))
        } else if s.unplugged || s.connects < ABSENT || s.restarting > 0 {
            None
        } else {
            Some(None)
        }
    }

    /// Attaches the watch named `model` next, if there is one; `none` attaches none, ever.
    #[cfg_attr(not(feature = "demo"), allow(dead_code))]
    fn attach(&self, model: &str) -> bool {
        if model.eq_ignore_ascii_case("none") {
            self.state().unplugged = true;
            return true;
        }
        match WATCHES.iter().position(|w| w.0.eq_ignore_ascii_case(model)) {
            Some(index) => {
                self.state().current = index;
                true
            }
            None => false,
        }
    }

    /// Whether the Forerunner 245 is mounted as a drive: during the script's no-watch
    /// connections, unless no watch is attached at all.
    #[cfg_attr(not(feature = "demo"), allow(dead_code))]
    fn drive_mounted(&self) -> bool {
        let s = self.state();
        !s.unplugged && !s.always_busy && s.connects <= ABSENT
    }

    /// Names holders as on `platform`; with `kind`, every connection finds the watch held by a
    /// program of that kind.
    #[cfg_attr(not(feature = "demo"), allow(dead_code))]
    fn hold(&self, kind: Option<HolderKind>, platform: &str) {
        let mut s = self.state();
        s.busy = busy_sample(kind.unwrap_or(HolderKind::GarminExpress), platform);
        s.always_busy = kind.is_some();
    }
}

/// What holds a demo watch for `kind`, named as on `platform`. Windows names no holder, only a
/// running Garmin Express, and shows no USB interface.
fn busy_sample(kind: HolderKind, platform: &str) -> Busy {
    let windows = platform == "windows";
    let holder = match kind {
        HolderKind::GarminExpress if windows => Holder::named("ExpressTray.exe", Some(6312), false),
        HolderKind::GarminExpress => Holder::named("Garmin Express", Some(812), true),
        HolderKind::MacosCameraService => Holder::named("ptpcamerad", Some(512), true),
        HolderKind::DesktopMtp if platform == "linux" => {
            Holder::named("gvfsd-mtp", Some(2301), true)
        }
        HolderKind::DesktopMtp => Holder::named("Android File Tra", Some(1450), true),
        HolderKind::Other => Holder::named("usb-helper", Some(3070), true),
        HolderKind::ThisApp => Holder::named("georgian-for-ga", Some(std::process::id()), true),
        HolderKind::Unknown => Holder::unknown(),
    };
    // A still-image interface where macOS's camera service takes it, Garmin's MTP one elsewhere.
    let interface = match kind {
        HolderKind::MacosCameraService => (6, 1, 1),
        _ => (255, 255, 0),
    };
    Busy {
        holder,
        product_id: Some(0x4E77),
        product_name: None,
        interface: (!windows).then_some(UsbInterface {
            class: interface.0,
            subclass: interface.1,
            protocol: interface.2,
        }),
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
    let platform = platform().unwrap_or_else(crate::built_for);
    let busy = std::env::var("GEORGIAN_FOR_GARMIN_DEMO_BUSY")
        .ok()
        .and_then(|name| {
            let kind = serde_json::from_value(serde_json::Value::String(name.clone())).ok();
            if kind.is_none() {
                eprintln!("GEORGIAN_FOR_GARMIN_DEMO_BUSY: {name:?} is no holder kind");
            }
            kind
        });
    demo.hold(busy, platform);
    demo
});

/// The app's demo watch. Transfers take a moment so the progress state is visible.
#[cfg(feature = "demo")]
pub fn connect() -> Result<Box<dyn Mtp>, Error> {
    WATCH.open()
}

/// Called after every install or restore that succeeded: see [`Demo::restart`].
#[cfg(feature = "demo")]
pub fn written() {
    WATCH.restart();
}

/// What `diagnose` lists in the demo build: the demo watch on USB and the demo drive.
#[cfg(feature = "demo")]
pub fn probe() -> Probe<'static> {
    Probe {
        usb: Box::new(|| UsbReport {
            listed: true,
            note: None,
            devices: WATCH
                .usb()
                .map(|holder| UsbDevice {
                    product_id: 0x4E77,
                    product_name: None,
                    mtp: true,
                    interfaces: vec![UsbInterface {
                        class: 255,
                        subclass: 255,
                        protocol: 0,
                    }],
                    holder,
                })
                .into_iter()
                .collect(),
        }),
        drives: Box::new(|| drives_at(&drive().into_iter().collect::<Vec<_>>())),
    }
}

/// Where the demo build saves diagnostic info instead of the Downloads folder.
#[cfg(feature = "demo")]
pub fn downloads() -> std::path::PathBuf {
    std::env::temp_dir().join("georgian-for-garmin-demo-downloads")
}

/// What the demo build's shares were about, for this run only: the demo opens nothing, so
/// nothing it "shares" reaches the app's settings.
#[cfg(feature = "demo")]
static SHARED: Mutex<Vec<ShareKey>> = Mutex::new(Vec::new());

#[cfg(feature = "demo")]
pub fn shared_before(key: &ShareKey) -> bool {
    SHARED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(key)
}

#[cfg(feature = "demo")]
pub fn mark_shared(key: ShareKey) {
    let mut shared = SHARED.lock().unwrap_or_else(PoisonError::into_inner);
    if !shared.contains(&key) {
        shared.push(key);
    }
}

/// The language chosen in the demo build, for this run only, so the demo leaves the app's
/// settings alone.
#[cfg(feature = "demo")]
static LANGUAGE: Mutex<Option<String>> = Mutex::new(None);

#[cfg(feature = "demo")]
pub fn language() -> Option<String> {
    LANGUAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

#[cfg(feature = "demo")]
pub fn set_language(language: String) {
    *LANGUAGE.lock().unwrap_or_else(PoisonError::into_inner) = Some(language);
}

/// The platform named by `GEORGIAN_FOR_GARMIN_DEMO_PLATFORM`, if it names one of the app's.
#[cfg(feature = "demo")]
pub fn platform() -> Option<&'static str> {
    let name = std::env::var("GEORGIAN_FOR_GARMIN_DEMO_PLATFORM").ok()?;
    let found = crate::PLATFORMS
        .into_iter()
        .find(|p| p.eq_ignore_ascii_case(&name));
    if found.is_none() {
        eprintln!(
            "GEORGIAN_FOR_GARMIN_DEMO_PLATFORM: {name:?} is none of {:?}",
            crate::PLATFORMS
        );
    }
    found
}

/// While no demo watch is attached over MTP: a Forerunner 245 "mounted as a USB drive" (issue
/// #16), a folder in the temp directory with its `GARMIN/GarminDevice.xml`.
#[cfg(feature = "demo")]
pub fn drive() -> Option<std::path::PathBuf> {
    if !WATCH.drive_mounted() {
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
                op::GARMIN_GET_OBJECT_IDS,
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
                // Imports are read-only and left out, as on the Venu 2.
                i.protection_status == 0
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

    /// The children the normal listing leaves out: the imports.
    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        Ok(self
            .state()
            .objects()
            .iter()
            .filter(|(_, (i, _))| {
                i.protection_status != 0 && i.storage_id == storage && i.parent == parent
            })
            .map(|(&h, _)| h)
            .collect())
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

    /// A demo with `model` attached and the script's connect screens done.
    fn attached(model: &str) -> Demo {
        let demo = Demo::new(Duration::ZERO);
        assert!(demo.attach(model), "{model}");
        for _ in 0..ABSENT + BUSY {
            assert!(demo.open().is_err());
        }
        demo
    }

    #[test]
    fn follows_the_script_through_install_and_restart() {
        let demo = Demo::new(Duration::ZERO);
        for _ in 0..ABSENT {
            assert!(demo.usb().is_none());
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::NoDevice));
        }
        for _ in 0..BUSY {
            assert!(demo.usb().is_some_and(|holder| holder.is_some()));
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::DeviceBusy));
        }
        assert_eq!(demo.usb(), Some(None));

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
        let waiting = call(&demo, "inspect").unwrap();
        assert_eq!(staged(&waiting), vec!["georgian"; sizes]);
        assert_eq!(waiting["state"], "waiting");
        assert_eq!(call(&demo, "install").unwrap()["status"], "already_staged");

        // Unplugged and restarted: gone for a while, then back with Georgian imported.
        demo.restart();
        for _ in 0..RESTART {
            assert!(demo.usb().is_none());
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::NoDevice));
        }
        let imported = call(&demo, "inspect").unwrap();
        assert_eq!(imported["device"]["model"], "Venu 2");
        assert_eq!(staged(&imported), vec!["georgian"; sizes]);
        assert_eq!(imported["state"], "installed");

        // Restoring: Garmin's font is back after the next restart.
        assert_eq!(
            call(&demo, "restore_stock").unwrap()["status"],
            "staged_verified"
        );
        demo.restart();
        for _ in 0..RESTART {
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::NoDevice));
        }
        let restored = call(&demo, "inspect").unwrap();
        assert_eq!(staged(&restored), vec!["stock"; sizes]);
    }

    #[test]
    fn every_watch_shows_its_status() {
        // The Forerunner 165: Garmin's TrueType fonts get Georgian, in use without a restart.
        let demo = attached("Forerunner 165");
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

        // The Venu 3 lists two packs; a leftover file makes installing a conflict.
        let demo = attached("Venu 3");
        let other = call(&demo, "inspect").unwrap();
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

        // The fenix 6S Pro: Swiss 721 Hebrew is restore-only in every build.
        let fenix = call(&attached("fenix 6S Pro"), "inspect").unwrap();
        assert_eq!(fenix["device"]["model"], "fenix 6S Pro");
        assert_eq!(fenix["supported"], false);
        assert_eq!(fenix["restorable"], true);
        assert_eq!(fenix["support"]["status"], "unsupported");
        assert_eq!(fenix["support"]["issue"], 6);

        // The Venu: its Korean font gets Georgian, but the model is only predicted to work.
        let venu = call(&attached("Venu"), "inspect").unwrap();
        assert_eq!(venu["supported"], true);
        assert_eq!(venu["support"]["status"], "expected");
        assert_eq!(venu["support"]["prediction"], "bitmap_korean");
        assert_eq!(venu["support"]["issue"], 12);
        assert_eq!(venu["support"]["report_wanted"], true);

        // The Lily 2 Active already has Georgian, on a model no one has confirmed it on: the
        // main screen's action is to check it.
        let lily = call(&attached("Lily 2 Active"), "inspect").unwrap();
        assert_eq!(lily["device"]["model"], "Lily 2 Active");
        assert_eq!(lily["state"], "installed");
        assert_eq!(lily["hosts"]["kind"], "bitmap");
        assert_eq!(staged(&lily), vec!["georgian"; 2]);
        assert_eq!(lily["support"]["status"], "expected");
        assert_eq!(lily["support"]["prediction"], "bitmap_korean");
        assert_eq!(lily["support"]["report_wanted"], true);
        assert_eq!(lily["tested"]["status"], "untested");
        assert_eq!(lily["tested"]["verified_software"], json!([]));

        // The Instinct 3 AMOLED's Georgian waits for a restart, and is in use after one.
        let demo = attached("Instinct 3 AMOLED 50mm");
        let instinct = call(&demo, "inspect").unwrap();
        assert_eq!(instinct["device"]["model"], "Instinct 3 AMOLED 50mm");
        assert_eq!(instinct["state"], "waiting");
        assert_eq!(staged(&instinct), vec!["georgian"]);
        assert_eq!(instinct["support"]["status"], "expected");
        assert_eq!(instinct["support"]["prediction"], "bitmap_korean");
        assert_eq!(instinct["support"]["report_wanted"], true);
        demo.restart();
        for _ in 0..RESTART {
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::NoDevice));
        }
        assert_eq!(call(&demo, "inspect").unwrap()["state"], "installed");

        // The tactix 7 has Georgian in every part verified on it: confirmed, no report asked.
        let tactix = call(&attached("tactix 7"), "inspect").unwrap();
        assert_eq!(tactix["device"]["model"], "tactix 7");
        assert_eq!(tactix["state"], "installed");
        assert_eq!(staged(&tactix), vec!["georgian"; 5]);
        assert_eq!(tactix["support"]["status"], "tested");
        assert_eq!(tactix["support"]["report_wanted"], false);

        // The Forerunner 265 lists none of this build's fonts: `unknown`, nothing to install,
        // and a report asked; its prediction is shown for triage.
        let demo = attached("Forerunner 265");
        let fr265 = call(&demo, "inspect").unwrap();
        assert_eq!(fr265["device"]["model"], "Forerunner 265");
        assert_eq!(fr265["supported"], false);
        assert_eq!(fr265["restorable"], false);
        assert_eq!(fr265["support"]["status"], "unknown");
        assert_eq!(fr265["support"]["listed"], true);
        assert_eq!(fr265["support"]["prediction"], "noto_truetype");
        assert_eq!(fr265["support"]["report_wanted"], true);
        assert_eq!(call(&demo, "install"), Err(ErrorCode::UnsupportedDevice));
    }

    #[test]
    fn none_never_finds_a_watch() {
        let demo = Demo::new(Duration::ZERO);
        assert!(demo.drive_mounted());
        assert!(demo.attach("none"));
        for _ in 0..ABSENT + BUSY + 3 {
            assert_eq!(call(&demo, "inspect"), Err(ErrorCode::NoDevice));
            assert!(!demo.drive_mounted());
        }
    }

    #[test]
    fn busy_holds_the_watch_on_every_connection() {
        let kinds = [
            HolderKind::GarminExpress,
            HolderKind::MacosCameraService,
            HolderKind::DesktopMtp,
            HolderKind::ThisApp,
            HolderKind::Other,
            HolderKind::Unknown,
        ];
        for kind in kinds {
            for platform in crate::PLATFORMS {
                let demo = Demo::new(Duration::ZERO);
                demo.hold(Some(kind), platform);
                for _ in 0..ABSENT + BUSY + 2 {
                    let error = demo.open().err().expect("busy");
                    assert_eq!(error.code, ErrorCode::DeviceBusy);
                    assert_eq!(error.busy.expect("details").holder.kind, kind);
                    assert!(!demo.drive_mounted());
                }
            }
        }
    }
}
