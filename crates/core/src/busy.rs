//! Who holds a busy watch (`device_busy`), as far as the system tells, and what to do about it.
//!
//! - macOS: the IOKit registry names the process that opened the watch's MTP interface for
//!   exclusive access, as `UsbExclusiveOwner` = `"pid 1234, name"` on the `IOUSBHostInterface`
//!   (and on its `IOUSBHostDevice`), readable by any user with `ioreg`. Process names there are
//!   cut to 16 bytes. The USB product name is the device's `USB Product Name`, absent when the
//!   watch has no product string (the Venu 2 has none).
//! - Linux: the process with the watch's `/dev/bus/usb/BBB/DDD` open, from `/proc/*/fd` (the
//!   user's own processes only, as the desktop's MTP helpers are).
//! - Windows doesn't say who holds a device, so only whether Garmin Express is running
//!   (`tasklist`): `confirmed` stays false.
//!
//! A holder whose pid is this process's own is `this_app`: another call of this program still
//! has the watch open. Nothing here stops or signals a process.

use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What kind of program holds the watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HolderKind {
    /// Garmin Express or one of its helpers.
    GarminExpress,
    /// macOS's camera service (`ptpcamerad`), which takes still-image (PTP) devices for Photos,
    /// Image Capture and other apps that import photos.
    MacosCameraService,
    /// A desktop's MTP file access: GNOME's `gvfsd-mtp`, KDE's `kiod`, or an MTP app such as
    /// Android File Transfer, OpenMTP or MacDroid.
    DesktopMtp,
    /// This program itself: the system names this process as holding the watch.
    ThisApp,
    /// A program none of the lists below knows.
    Other,
    /// The system didn't say.
    Unknown,
}

/// The program holding the watch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Holder {
    pub kind: HolderKind,
    /// Its process name as the system gives it (macOS cuts it to 16 bytes), or a driver's name;
    /// null when unknown.
    pub name: Option<String>,
    pub pid: Option<u32>,
    /// The system named it as holding the watch (macOS, Linux). False for a program only known
    /// to be running (Windows) and for `unknown`.
    pub confirmed: bool,
}

impl Holder {
    pub fn unknown() -> Holder {
        Holder {
            kind: HolderKind::Unknown,
            name: None,
            pid: None,
            confirmed: false,
        }
    }

    /// The program called `name`, classified by name; `this_app` when `pid` is this process.
    pub fn named(name: &str, pid: Option<u32>, confirmed: bool) -> Holder {
        Holder {
            kind: if pid == Some(std::process::id()) {
                HolderKind::ThisApp
            } else {
                classify(name)
            },
            name: Some(name.to_owned()),
            pid,
            confirmed,
        }
    }

    /// "name (pid 1234)", "name", or "Another program".
    fn describe(&self) -> String {
        match (&self.name, self.pid) {
            (Some(name), Some(pid)) => format!("{name} (pid {pid})"),
            (Some(name), None) => name.clone(),
            (None, _) => "Another program".into(),
        }
    }
}

/// A USB interface's class, subclass and protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsbInterface {
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
}

/// What is known about a busy watch: `device_busy`'s `busy` in the error JSON, and the report's
/// `busy` line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Busy {
    pub holder: Holder,
    /// The watch's USB product ID (e.g. 20087, 0x4E77, for a Venu 2); null when unknown.
    pub product_id: Option<u16>,
    /// The watch's USB product name, when it has one and the system shows it.
    pub product_name: Option<String>,
    /// The watch's MTP interface; null when unknown.
    pub interface: Option<UsbInterface>,
}

impl Busy {
    /// Nothing known: the holder is `unknown`.
    pub fn unknown() -> Busy {
        Busy {
            holder: Holder::unknown(),
            product_id: None,
            product_name: None,
            interface: None,
        }
    }
}

/// Known holders by process name: lowercase, without `.exe`. Garmin Express is `Garmin
/// Express` and `Garmin Express Service` on macOS, `express.exe` and `ExpressTray.exe` on
/// Windows.
const GARMIN_EXPRESS: &[&str] = &[
    "garmin express",
    "garmin express service",
    "express",
    "expresstray",
];
const CAMERA_SERVICE: &[&str] = &["ptpcamerad", "ptpcamera"];
const DESKTOP_MTP: &[&str] = &[
    "gvfsd-mtp",
    "kiod6",
    "kiod5",
    "kio_mtp",
    "kioworker",
    "android file transfer",
    "android file transfer agent",
    "openmtp",
    "macdroid",
    "jmtpfs",
    "simple-mtpfs",
    "go-mtpfs",
];

/// Linux keeps 15 bytes of a process name, macOS's registry 16: a name this long may be cut.
const CUT_NAME: usize = 15;

/// The kind of program called `name`. A name that may have been cut matches the known names
/// it starts.
pub fn classify(name: &str) -> HolderKind {
    let lower = name.trim().to_lowercase();
    let name = lower.strip_suffix(".exe").unwrap_or(&lower);
    let known = |list: &[&str]| {
        list.iter()
            .any(|k| *k == name || (name.len() >= CUT_NAME && k.starts_with(name)))
    };
    if name.is_empty() {
        HolderKind::Unknown
    } else if known(GARMIN_EXPRESS) {
        HolderKind::GarminExpress
    } else if known(CAMERA_SERVICE) {
        HolderKind::MacosCameraService
    } else if known(DESKTOP_MTP) {
        HolderKind::DesktopMtp
    } else {
        HolderKind::Other
    }
}

/// The holder in a registry owner string: `"pid 1234, name"`, or a driver's bare name.
pub fn parse_owner(owner: &str) -> Option<Holder> {
    let owner = owner.trim();
    if owner.is_empty() {
        return None;
    }
    let parsed = owner
        .strip_prefix("pid ")
        .and_then(|rest| rest.split_once(", "))
        .and_then(|(pid, name)| Some((pid.parse().ok()?, name)));
    Some(match parsed {
        Some((pid, name)) => Holder::named(name, Some(pid), true),
        None => Holder::named(owner, None, true),
    })
}

/// In `ioreg -a -l -r -c IOUSBHostDevice`'s plist, the device with registry entry `entry_id`:
/// who holds it (its interfaces' `UsbExclusiveOwner`, else its own) and its product name. None
/// when the device isn't there or the plist can't be read.
pub fn from_ioreg(plist: &str, entry_id: u64) -> Option<(Option<Holder>, Option<String>)> {
    use roxmltree::{Document, Node, ParsingOptions};

    /// The value element after `<key>key</key>` in `dict`.
    fn value<'a, 'i>(dict: Node<'a, 'i>, key: &str) -> Option<Node<'a, 'i>> {
        let mut children = dict.children().filter(Node::is_element);
        while let Some(k) = children.next() {
            let v = children.next()?;
            if k.has_tag_name("key") && k.text() == Some(key) {
                return Some(v);
            }
        }
        None
    }
    fn text<'a>(dict: Node<'a, '_>, key: &str) -> Option<&'a str> {
        value(dict, key).and_then(|v| v.text())
    }
    fn owner(dict: Node<'_, '_>) -> Option<Holder> {
        text(dict, "UsbExclusiveOwner").and_then(parse_owner)
    }

    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(plist, options).ok()?;
    let id = entry_id.to_string();
    let device = doc.descendants().find(|n| {
        n.has_tag_name("dict")
            && text(*n, "IOObjectClass") == Some("IOUSBHostDevice")
            && text(*n, "IORegistryEntryID") == Some(id.as_str())
    })?;
    let interfaces = value(device, "IORegistryEntryChildren")
        .into_iter()
        .flat_map(|a| a.children())
        .filter(|c| {
            c.has_tag_name("dict") && text(*c, "IOObjectClass") == Some("IOUSBHostInterface")
        });
    let holder = interfaces
        .filter_map(owner)
        .next()
        .or_else(|| owner(device));
    let product = text(device, "USB Product Name").map(str::to_owned);
    Some((holder, product))
}

/// Under the `/proc`-like folder `proc`, the process other than `own_pid` with `device` open.
pub fn from_proc(proc: &std::path::Path, device: &std::path::Path, own_pid: u32) -> Option<Holder> {
    let entries = std::fs::read_dir(proc).ok()?;
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own_pid {
            continue;
        }
        // Other users' processes can't be read; skipping them is all this can do.
        let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        let holds = fds
            .flatten()
            .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|target| target == device));
        if holds {
            let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            let name = comm.trim();
            return Some(if name.is_empty() {
                Holder {
                    pid: Some(pid),
                    confirmed: true,
                    ..Holder::unknown()
                }
            } else {
                Holder::named(name, Some(pid), true)
            });
        }
    }
    None
}

/// In `tasklist /fo csv /nh` output (`"image","pid",…` per line), the first Garmin Express
/// process: only running, not known to hold the watch.
pub fn from_tasklist(csv: &str) -> Option<Holder> {
    csv.lines().find_map(|line| {
        let mut fields = line.trim().trim_matches('"').split("\",\"");
        let name = fields.next()?;
        let pid = fields.next().and_then(|p| p.parse().ok());
        (classify(name) == HolderKind::GarminExpress).then(|| Holder::named(name, pid, false))
    })
}

/// `ioreg -a -l -r -c IOUSBHostDevice`'s plist: every USB device with its interfaces. None when
/// `ioreg` fails.
#[cfg(target_os = "macos")]
pub(crate) fn ioreg() -> Option<String> {
    std::process::Command::new("/usr/sbin/ioreg")
        .args(["-a", "-l", "-r", "-c", "IOUSBHostDevice"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
}

/// The holder of the macOS device with registry entry `entry_id`, and its product name.
#[cfg(target_os = "macos")]
pub(crate) fn macos(entry_id: u64) -> (Holder, Option<String>) {
    match ioreg().and_then(|plist| from_ioreg(&plist, entry_id)) {
        Some((holder, product)) => (holder.unwrap_or_else(Holder::unknown), product),
        None => (Holder::unknown(), None),
    }
}

/// The holder of the Linux device at `bus`/`address`.
#[cfg(target_os = "linux")]
pub(crate) fn linux(bus: u8, address: u8) -> Holder {
    let device = format!("/dev/bus/usb/{bus:03}/{address:03}");
    from_proc(
        std::path::Path::new("/proc"),
        std::path::Path::new(&device),
        std::process::id(),
    )
    .unwrap_or_else(Holder::unknown)
}

/// Garmin Express, if it is running on this Windows computer.
#[cfg(windows)]
pub(crate) fn windows() -> Holder {
    use std::os::windows::process::CommandExt;
    /// No console window flashes up from the desktop app.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("tasklist")
        .args(["/fo", "csv", "/nh"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| from_tasklist(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_else(Holder::unknown)
}

/// Where Garmin Express keeps its helper on this system.
const EXPRESS_HELPER: &str = if cfg!(windows) {
    "its icon in the notification area"
} else {
    "its menu-bar icon"
};

/// The error message for `busy` on this system: who holds the watch and how to free it.
pub fn message(busy: &Busy) -> String {
    let holder = &busy.holder;
    let who = holder.describe();
    let mut out = match holder.kind {
        HolderKind::GarminExpress if holder.confirmed => format!(
            "{who} is using the watch's USB connection. Quit Garmin Express, including \
             {EXPRESS_HELPER}, and try again."
        ),
        HolderKind::GarminExpress => format!(
            "Another program is using the watch, and Garmin Express is running ({who}). Quit \
             Garmin Express, including {EXPRESS_HELPER}, and try again."
        ),
        HolderKind::MacosCameraService => format!(
            "macOS's camera service, {who}, is using the watch's USB connection. Quit Photos, \
             Image Capture, Preview and other apps that import photos, then unplug the watch \
             and plug it back in. If it is still busy, please open an issue with this error at \
             {} so we can look.",
            crate::report::ISSUES
        ),
        HolderKind::DesktopMtp if cfg!(target_os = "linux") => format!(
            "{who}, the desktop's file access, is using the watch's USB connection. On GNOME, \
             eject the watch in Files (or `gio mount -u` its mtp:// address); on KDE, run \
             `kquitapp6 kiod6`. Then try again."
        ),
        HolderKind::DesktopMtp | HolderKind::Other => {
            format!("{who} is using the watch's USB connection. Quit it and try again.")
        }
        HolderKind::ThisApp => "This program already has the watch open for another \
             operation. Wait for it to finish and try again; if the watch stays busy, quit \
             every Georgian for Garmin window and start again."
            .into(),
        HolderKind::Unknown if cfg!(target_os = "linux") => "Another program is using the \
             watch's USB connection, usually the desktop's file manager. On GNOME, eject the \
             watch in Files (or `gio mount -u` its mtp:// address); on KDE, run `kquitapp6 \
             kiod6`. Then try again."
            .into(),
        HolderKind::Unknown => "Another program is using the watch's USB connection. Quit \
             Garmin Express (including its background helper) and other programs that open \
             phones, cameras or watches, then try again; restarting the computer frees it too."
            .into(),
    };
    if let Some(name) = &busy.product_name {
        write!(out, " (USB device: {name})").unwrap();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_programs_are_classified_by_name() {
        for (name, kind) in [
            ("Garmin Express", HolderKind::GarminExpress),
            // Cut to 16 bytes in the macOS registry.
            ("Garmin Express S", HolderKind::GarminExpress),
            ("express.exe", HolderKind::GarminExpress),
            ("ExpressTray.exe", HolderKind::GarminExpress),
            ("ptpcamerad", HolderKind::MacosCameraService),
            ("PTPCamera", HolderKind::MacosCameraService),
            ("gvfsd-mtp", HolderKind::DesktopMtp),
            ("kiod6", HolderKind::DesktopMtp),
            ("Android File Tra", HolderKind::DesktopMtp),
            ("OpenMTP", HolderKind::DesktopMtp),
            ("gfg-hold", HolderKind::Other),
            ("AppleUserUSBHostHIDDevice", HolderKind::Other),
            // Short names match only whole.
            ("ptp", HolderKind::Other),
            ("expressvpn", HolderKind::Other),
            ("", HolderKind::Unknown),
        ] {
            assert_eq!(classify(name), kind, "{name:?}");
        }
    }

    #[test]
    fn registry_owners_give_pid_and_name() {
        assert_eq!(
            parse_owner("pid 77818, gfg-hold"),
            Some(Holder {
                kind: HolderKind::Other,
                name: Some("gfg-hold".into()),
                pid: Some(77818),
                confirmed: true,
            })
        );
        let driver = parse_owner("AppleUserUSBHostHIDDevice").unwrap();
        assert_eq!(
            (driver.name.as_deref(), driver.pid),
            (Some("AppleUserUSBHostHIDDevice"), None)
        );
        assert_eq!(parse_owner(" "), None);
        let own = parse_owner(&format!("pid {}, georgian-for-ga", std::process::id())).unwrap();
        assert_eq!(own.kind, HolderKind::ThisApp);
        assert!(own.confirmed);
        assert_ne!(
            Holder::named("georgian-for-ga", Some(std::process::id() + 1), true).kind,
            HolderKind::ThisApp
        );
        assert!(
            message(&Busy {
                holder: own,
                ..Busy::unknown()
            })
            .contains("This program")
        );
    }

    const IOREG: &str = include_str!("../tests/busy/ioreg-held.plist");

    #[test]
    fn the_registry_names_the_interface_owner() {
        // The Venu 2: no product string; the hub's name above it is not the watch's.
        let (holder, product) = from_ioreg(IOREG, 4297301549).unwrap();
        let holder = holder.unwrap();
        assert_eq!(
            (holder.name.as_deref(), holder.pid),
            (Some("gfg-hold"), Some(77818))
        );
        assert_eq!(product, None);

        let (holder, product) = from_ioreg(IOREG, 4297400001).unwrap();
        assert_eq!(holder.unwrap().kind, HolderKind::MacosCameraService);
        assert_eq!(product.as_deref(), Some("Example Watch"));

        // Not a device, or not there.
        assert_eq!(from_ioreg(IOREG, 4297301554), None);
        assert_eq!(from_ioreg(IOREG, 1), None);
        assert_eq!(from_ioreg("not a plist", 4297301549), None);
    }

    #[cfg(unix)]
    #[test]
    fn proc_finds_the_process_with_the_device_open() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("gfg-busy-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let device = std::path::Path::new("/dev/bus/usb/003/014");
        let process = |pid: u32, comm: &str, fds: &[&str]| {
            let dir = root.join(pid.to_string());
            std::fs::create_dir_all(dir.join("fd")).unwrap();
            std::fs::write(dir.join("comm"), format!("{comm}\n")).unwrap();
            for (i, target) in fds.iter().enumerate() {
                symlink(target, dir.join("fd").join(i.to_string())).unwrap();
            }
        };
        process(10, "bash", &["/dev/pts/0", "/dev/bus/usb/003/001"]);
        process(20, "georgian-for-g", &["/dev/bus/usb/003/014"]);
        process(30, "gvfsd-mtp", &["/dev/null", "/dev/bus/usb/003/014"]);
        std::fs::create_dir_all(root.join("self")).unwrap();
        std::fs::create_dir_all(root.join("40")).unwrap(); // no fd folder: another user's

        let holder = from_proc(&root, device, 20).unwrap();
        assert_eq!(holder.kind, HolderKind::DesktopMtp);
        assert_eq!((holder.pid, holder.confirmed), (Some(30), true));
        assert_eq!(
            from_proc(&root, std::path::Path::new("/dev/bus/usb/001/002"), 20),
            None
        );
        assert_eq!(from_proc(&root.join("missing"), device, 20), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn tasklist_finds_garmin_express_as_running_only() {
        let csv = "\"System Idle Process\",\"0\",\"Services\",\"0\",\"8 K\"\r\n\
                   \"explorer.exe\",\"4120\",\"Console\",\"1\",\"150.000 K\"\r\n\
                   \"ExpressTray.exe\",\"6312\",\"Console\",\"1\",\"40.112 K\"\r\n";
        let holder = from_tasklist(csv).unwrap();
        assert_eq!(holder.kind, HolderKind::GarminExpress);
        assert_eq!(
            (holder.name.as_deref(), holder.pid),
            (Some("ExpressTray.exe"), Some(6312))
        );
        assert!(!holder.confirmed);
        assert_eq!(
            from_tasklist("\"explorer.exe\",\"4120\",\"Console\",\"1\",\"1 K\""),
            None
        );
        assert_eq!(from_tasklist(""), None);
    }

    #[test]
    fn messages_name_the_holder_and_only_this_systems_steps() {
        let busy = |holder| Busy {
            holder,
            product_id: Some(0x4E77),
            product_name: None,
            interface: None,
        };
        let express = message(&busy(Holder::named("Garmin Express", Some(5), true)));
        assert!(
            express.starts_with("Garmin Express (pid 5) is using"),
            "{express}"
        );
        let camera = message(&busy(Holder::named("ptpcamerad", Some(7), true)));
        assert!(
            camera.contains("Photos") && camera.contains("/issues"),
            "{camera}"
        );
        assert!(
            !camera.contains("pkill") && !camera.contains("Terminal"),
            "{camera}"
        );
        let other = message(&busy(Holder::named("gfg-hold", Some(9), true)));
        assert!(other.starts_with("gfg-hold (pid 9) is using"), "{other}");
        for text in [express, other, message(&Busy::unknown())] {
            if cfg!(target_os = "linux") {
                assert!(!text.contains("Garmin Express (including"), "{text}");
            } else {
                assert!(!text.contains("GNOME") && !text.contains("KDE"), "{text}");
            }
            assert!(
                !text.contains(if cfg!(windows) {
                    "menu-bar"
                } else {
                    "notification area"
                }),
                "{text}"
            );
        }
    }
}
