//! `diagnose`: what we ask for when we help someone who can't use GitHub. It says which program
//! and system this is, which Garmin USB devices the system lists, what opening the watch gave,
//! which mounted volumes look like a watch, the watch report when the watch could be read, and
//! the app's recent steps; `text` is all of it as readable text, the one format people send.
//!
//! Nothing here holds a serial number or watch ID: USB devices are listed by product ID, name,
//! interface classes and holder only; volumes by their name and the watch's model, part number
//! and software version; the watch by the allowlisted report ([`crate::report`]). Events and
//! product names that look like an identifier are refused or left out.
//!
//! Per platform, best effort:
//! - macOS: USB devices from IOKit (through `nusb`), product names and holders from the IOKit
//!   registry (`ioreg`), as [`crate::busy`] reads them.
//! - Linux: USB devices from sysfs (through `nusb`), holders from `/proc` (this user's
//!   processes only).
//! - Windows: the Garmin devices Windows Portable Devices lists (MTP only), with their product
//!   ID and description; Windows says nothing about interfaces or holders.
//!
//! What a platform can't list is said in `listed` and `note`; it never fails the operation.
//! Device problems are data in `connection`; only invalid input is an error.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::busy::{self, Busy, Holder, HolderKind, UsbInterface};
use crate::drive;
use crate::error::{Error, ErrorCode};
use crate::fonts::FontSet;
use crate::ops::{APP_VERSION, InspectOutput};
use crate::report::{self, Platform, WatchReport, column, looks_like_identifier, snake};

/// The most events one call takes.
pub const MAX_EVENTS: usize = 50;

/// What kind of step an event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A poll for the watch whose result changed.
    Poll,
    OperationStart,
    OperationEnd,
    /// An answer the owner gave.
    Answer,
    /// A share (a report link opened or copied).
    Share,
}

/// One of the app's recent steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Local time without a zone, e.g. "2026-11-24T03:14:05".
    #[schemars(regex(pattern = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}$"))]
    pub time: String,
    pub kind: EventKind,
    /// The error code or state, e.g. "no_device", "install: staged_verified", "result: boxes".
    #[schemars(length(max = 200))]
    pub detail: String,
    /// The watch's model name, e.g. "Venu 2"; null when none was known.
    #[schemars(length(max = 100))]
    pub model: Option<String>,
}

/// `diagnose`'s input.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnoseInput {
    /// The app's recent steps, oldest first, at most 50. Nothing in them may look like an
    /// identifier (a run of 9 or more digits, or a word of 12 or more letters and digits with 4
    /// or more digits): such input is refused.
    #[schemars(length(max = 50))]
    pub events: Option<Vec<Event>>,
}

/// This program.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AppInfo {
    pub version: String,
    /// "release" or "debug", with ", dev operations" when built with them and ", beta fonts"
    /// when its Georgian fonts carry test marks.
    pub build: String,
}

/// A Garmin USB device (vendor 0x091E) as the system lists it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UsbDevice {
    /// The USB product ID (20087, 0x4E77, for a Venu 2).
    pub product_id: u16,
    /// The USB product name, when the device has one and the system shows it.
    pub product_name: Option<String>,
    /// Whether it has an MTP interface (on Windows: WPD lists only MTP devices).
    pub mtp: bool,
    /// Its interfaces' class, subclass and protocol; empty on Windows.
    pub interfaces: Vec<UsbInterface>,
    /// The program that has it open, when the system tells (macOS, Linux); null otherwise.
    pub holder: Option<Holder>,
}

/// The Garmin USB devices the system lists.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UsbReport {
    /// Whether the system's devices could be listed.
    pub listed: bool,
    /// Why they couldn't be, or what this system doesn't tell.
    pub note: Option<String>,
    pub devices: Vec<UsbDevice>,
}

/// What opening and reading the watch gave.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ConnectionReport {
    /// The watch was opened and its report read.
    pub ok: bool,
    /// The error's code when not.
    pub code: Option<ErrorCode>,
    /// The error's message when not.
    pub message: Option<String>,
    /// `device_busy`: who holds the watch, as far as the system tells.
    pub busy: Option<Busy>,
}

/// A mounted volume that looks like a Garmin watch (it has `GARMIN/GarminDevice.xml`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Volume {
    /// The volume's name ("GARMIN") or drive ("E:"), never its full path.
    pub volume: String,
    pub model: Option<String>,
    pub part_number: Option<String>,
    pub software_version: Option<String>,
    /// The error code when its `GarminDevice.xml` couldn't be read.
    pub error: Option<String>,
}

/// Mounted volumes that look like a Garmin watch.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DrivesReport {
    /// Whether this system's volumes could be looked through.
    pub listed: bool,
    pub note: Option<String>,
    pub volumes: Vec<Volume>,
}

/// `diagnose`'s output.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DiagnoseOutput {
    pub app: AppInfo,
    pub system: Platform,
    pub usb: UsbReport,
    pub connection: ConnectionReport,
    pub drives: DrivesReport,
    /// The watch report (the one `issue_link` takes) when the connection worked; else null.
    pub watch: Option<WatchReport>,
    /// The input's events, as given.
    pub events: Vec<Event>,
    /// All of the above as readable text; its first line says it holds no serial number or
    /// watch ID.
    pub text: String,
}

/// The first line of `text`.
pub const HEADER: &str =
    "Georgian for Garmin diagnostic info. It contains no serial number or watch ID.";

/// Refuses events with a field that looks like an identifier.
pub fn check_events(events: &[Event]) -> Result<(), Error> {
    for (i, event) in events.iter().enumerate() {
        let fields = [
            ("detail", Some(&event.detail)),
            ("model", event.model.as_ref()),
        ];
        for (name, value) in fields {
            if value.is_some_and(|v| looks_like_identifier(v)) {
                return Err(Error::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "events[{i}].{name} looks like an identifier (a serial number or watch \
                         ID); events carry codes, states and model names only"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Where `diagnose` looks for USB devices and mounted volumes: the system, or a test's.
pub struct Probe<'a> {
    pub usb: Box<dyn Fn() -> UsbReport + 'a>,
    pub drives: Box<dyn Fn() -> DrivesReport + 'a>,
}

impl Probe<'_> {
    /// This computer's USB devices and volumes.
    pub fn system() -> Probe<'static> {
        Probe {
            usb: Box::new(usb),
            drives: Box::new(drives),
        }
    }
}

/// The Garmin USB devices this computer lists, with their holders where the system tells.
#[cfg(not(windows))]
pub fn usb() -> UsbReport {
    use nusb::MaybeFuture;

    let list = match nusb::list_devices().wait() {
        Ok(list) => list,
        Err(e) => {
            return UsbReport {
                listed: false,
                note: Some(format!("The USB devices could not be listed: {e}")),
                devices: Vec::new(),
            };
        }
    };
    #[cfg(target_os = "macos")]
    let plist = crate::busy::ioreg();
    let devices = list
        .filter(|d| d.vendor_id() == crate::GARMIN_VENDOR_ID)
        .map(|info| {
            let interfaces: Vec<UsbInterface> = info
                .interfaces()
                .map(|i| UsbInterface {
                    class: i.class(),
                    subclass: i.subclass(),
                    protocol: i.protocol(),
                })
                .collect();
            // nusb's product string on macOS can be a hub's when the watch has none (the Venu
            // 2's), so there it comes from the watch's own registry entry, as in `busy`.
            #[cfg(target_os = "macos")]
            let (holder, name) = plist
                .as_deref()
                .and_then(|p| crate::busy::from_ioreg(p, info.registry_entry_id()))
                .unwrap_or((None, None));
            // Pid 0 is never a process, so this program is found too when it holds the watch.
            #[cfg(target_os = "linux")]
            let (holder, name) = (
                crate::busy::from_proc(
                    Path::new("/proc"),
                    Path::new(&format!(
                        "/dev/bus/usb/{:03}/{:03}",
                        info.busnum(),
                        info.device_address()
                    )),
                    0,
                ),
                info.product_string().map(str::to_owned),
            );
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            let (holder, name) = (None, info.product_string().map(str::to_owned));
            UsbDevice {
                product_id: info.product_id(),
                product_name: busy::product_name(name.as_deref()),
                mtp: interfaces
                    .iter()
                    .any(|i| crate::usb::is_mtp_class(i.class, i.subclass, i.protocol)),
                interfaces,
                holder,
            }
        })
        .collect();
    #[cfg(target_os = "macos")]
    let note = plist
        .is_none()
        .then_some("The IOKit registry (ioreg) could not be read: no holders or product names.");
    #[cfg(target_os = "linux")]
    let note = Some("Linux: holders are looked for among this user's processes only.");
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let note = Some("This system doesn't tell who holds a USB device.");
    UsbReport {
        listed: true,
        note: note.map(str::to_owned),
        devices,
    }
}

/// The Garmin devices Windows Portable Devices lists.
#[cfg(windows)]
pub fn usb() -> UsbReport {
    const NOTE: &str = "Windows: only the Garmin devices Windows Portable Devices lists (MTP \
                        mode), without interface classes or holders; Windows doesn't say who \
                        holds a device.";
    match crate::wpd::garmin_devices() {
        Ok(found) => {
            let total = found.len();
            let devices: Vec<UsbDevice> = found
                .into_iter()
                .filter_map(|device| {
                    Some(UsbDevice {
                        product_id: device.product_id?,
                        product_name: busy::product_name(device.description.as_deref()),
                        mtp: true,
                        interfaces: Vec::new(),
                        holder: None,
                    })
                })
                .collect();
            let mut note = NOTE.to_owned();
            if devices.len() < total {
                note.push_str(&format!(
                    " {} Garmin device(s) without a readable product ID are left out.",
                    total - devices.len()
                ));
            }
            UsbReport {
                listed: true,
                note: Some(note),
                devices,
            }
        }
        Err(e) => UsbReport {
            listed: false,
            note: Some(format!("{NOTE} They could not be listed: {}", e.message)),
            devices: Vec::new(),
        },
    }
}

/// Mounted volumes that look like a Garmin watch, as `inspect_drive` finds them.
pub fn drives() -> DrivesReport {
    let mut report = drives_at(&drive::find());
    if !cfg!(any(target_os = "macos", target_os = "linux", windows)) {
        report.listed = false;
        report.note = Some("This system's mounted volumes aren't looked through.".into());
    }
    report
}

/// The volume's name, never its full path (which can hold the user's name).
fn volume_name(root: &Path) -> String {
    let name = match root.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => root
            .display()
            .to_string()
            .trim_end_matches(['\\', '/'])
            .to_owned(),
    };
    if looks_like_identifier(&name) {
        "(volume)".into()
    } else {
        name
    }
}

/// The volumes at `roots`, each with the watch its `GarminDevice.xml` describes.
pub fn drives_at(roots: &[PathBuf]) -> DrivesReport {
    let volumes = roots
        .iter()
        .map(|root| {
            let volume = volume_name(root);
            match drive::read(root) {
                Ok((device, _)) => Volume {
                    volume,
                    model: Some(device.description),
                    part_number: Some(device.part_number),
                    software_version: Some(device.software_version),
                    error: None,
                },
                Err(e) => Volume {
                    volume,
                    model: None,
                    part_number: None,
                    software_version: None,
                    error: Some(e.code.as_str().into()),
                },
            }
        })
        .collect();
    DrivesReport {
        listed: true,
        note: None,
        volumes,
    }
}

/// "release" or "debug", with what this build adds.
fn build(fonts: &FontSet) -> String {
    let mut build = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
    .to_owned();
    if cfg!(feature = "dev") {
        build.push_str(", dev operations");
    }
    if fonts.test_marks() {
        build.push_str(", beta fonts");
    }
    build
}

/// The whole output from what was found: `connection` is what reading the watch (`inspect`)
/// gave.
pub fn diagnose(
    fonts: &FontSet,
    events: Vec<Event>,
    usb: UsbReport,
    drives: DrivesReport,
    connection: Result<InspectOutput, Error>,
) -> DiagnoseOutput {
    let (connection, watch) = match connection {
        Ok(inspect) => (
            ConnectionReport {
                ok: true,
                code: None,
                message: None,
                busy: None,
            },
            Some(inspect.report),
        ),
        Err(e) => (
            ConnectionReport {
                ok: false,
                code: Some(e.code),
                message: Some(e.message),
                busy: e.busy.map(|b| *b),
            },
            None,
        ),
    };
    let mut out = DiagnoseOutput {
        app: AppInfo {
            version: APP_VERSION.into(),
            build: build(fonts),
        },
        system: Platform::current(),
        usb,
        connection,
        drives,
        watch,
        events,
        text: String::new(),
    };
    out.text = text(&out);
    out
}

fn optional(value: Option<&str>) -> String {
    column(value.unwrap_or(""))
}

/// `<kind> confirmed|running name=… pid=…`, or `-`.
fn holder_words(holder: Option<&Holder>) -> String {
    let Some(h) = holder else {
        return "-".into();
    };
    let seen = match (h.confirmed, h.kind) {
        (true, _) => "confirmed",
        (false, HolderKind::Unknown) => "unknown",
        (false, _) => "running",
    };
    let pid = h.pid.map_or_else(|| "-".to_owned(), |p| p.to_string());
    format!(
        "{} {seen} name={} pid={pid}",
        snake(&h.kind),
        optional(h.name.as_deref())
    )
}

/// The readable text: one item per line, space-separated columns, `-` for what isn't known.
fn text(out: &DiagnoseOutput) -> String {
    let mut t = String::new();
    let mut line = |s: String| {
        t.push_str(&s);
        t.push('\n');
    };
    line(HEADER.into());
    line(format!(
        "app {} {}",
        column(&out.app.version),
        column(&out.app.build)
    ));
    let s = &out.system;
    line(format!(
        "system {} {} {}",
        column(&s.os),
        column(&s.os_version),
        column(&s.arch)
    ));

    let usb = &out.usb;
    line(format!(
        "usb {} garmin_devices={}",
        if usb.listed { "listed" } else { "not_listed" },
        usb.devices.len()
    ));
    for d in &usb.devices {
        let interfaces: Vec<String> = d
            .interfaces
            .iter()
            .map(|i| format!("{:02x}/{:02x}/{:02x}", i.class, i.subclass, i.protocol))
            .collect();
        line(format!(
            "  device product_id={:04x} product={} mtp={} interfaces={} holder={}",
            d.product_id,
            optional(d.product_name.as_deref()),
            if d.mtp { "yes" } else { "no" },
            if interfaces.is_empty() {
                "-".into()
            } else {
                interfaces.join(",")
            },
            holder_words(d.holder.as_ref()),
        ));
    }
    if let Some(note) = &usb.note {
        line(format!("  note {}", column(note)));
    }

    let c = &out.connection;
    match c.code {
        None => line("connection ok".into()),
        Some(code) => {
            line(format!(
                "connection {} message={}",
                code.as_str(),
                optional(c.message.as_deref())
            ));
            if let Some(busy) = &c.busy {
                line(format!("  {}", report::busy_line(busy)));
            }
        }
    }

    let drives = &out.drives;
    line(format!(
        "drives {} watch_volumes={}",
        if drives.listed {
            "listed"
        } else {
            "not_listed"
        },
        drives.volumes.len()
    ));
    for v in &drives.volumes {
        line(format!(
            "  volume {} model={} part={} software={} error={}",
            column(&v.volume),
            optional(v.model.as_deref()),
            optional(v.part_number.as_deref()),
            optional(v.software_version.as_deref()),
            optional(v.error.as_deref()),
        ));
    }
    if let Some(note) = &drives.note {
        line(format!("  note {}", column(note)));
    }

    match &out.watch {
        Some(report) => {
            line("watch report:".into());
            for l in report.text.lines() {
                line(format!("  {l}"));
            }
        }
        None => line("watch -".into()),
    }

    line(format!("events {}", out.events.len()));
    for e in &out.events {
        line(format!(
            "  {} {} {} model={}",
            column(&e.time),
            snake(&e.kind),
            column(&e.detail),
            optional(e.model.as_deref()),
        ));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_recognised() {
        for value in [
            "FAKESERIAL0042",
            "3999999999",
            "unit 123456789",
            "0000c1234567",
            "serial: A1B2C3D4E5F6",
        ] {
            assert!(looks_like_identifier(value), "{value}");
        }
        for value in [
            "no_device",
            "install: staged_verified",
            "result: boxes",
            "Venu 2",
            "Forerunner 265",
            "006-D9035-03",
            "006-B3703-00",
            "fēnix 6S Pro",
            "2026-11-24T03:14:05",
            "",
        ] {
            assert!(!looks_like_identifier(value), "{value}");
        }
    }

    #[test]
    fn volumes_are_named_without_their_path() {
        assert_eq!(volume_name(Path::new("/media/ann/GARMIN")), "GARMIN");
        assert_eq!(volume_name(Path::new("/Volumes/GARMIN")), "GARMIN");
        assert_eq!(volume_name(Path::new("/Volumes/3999999999")), "(volume)");
    }
}
