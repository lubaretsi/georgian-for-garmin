//! Watches that connect as a USB drive (mass storage) instead of MTP, like the Forerunner 245
//! in issue #16: found as a mounted volume with `GARMIN/GarminDevice.xml`, read-only. Nothing
//! is installed this way; the report lets their owners share the font list.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, ErrorCode};
use crate::garmin_device::{self, GarminDevice};
use crate::report::ListedFile;

/// The device file, relative to a volume's root. FAT volumes match it case-insensitively.
pub const DEVICE_XML: &str = "GARMIN/GarminDevice.xml";
const EXTDATA: &str = "GARMIN/ExtData";

/// Mount points that may be a watch's volume.
#[cfg(target_os = "macos")]
fn mount_points() -> Vec<PathBuf> {
    fs::read_dir("/Volumes")
        .map(|dir| dir.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// Mount points of FAT and exFAT file systems, the ones Garmin watches use.
#[cfg(target_os = "linux")]
fn mount_points() -> Vec<PathBuf> {
    let Ok(mounts) = fs::read_to_string("/proc/self/mounts") else {
        return Vec::new();
    };
    mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(' ');
            let (_, point, kind) = (fields.next()?, fields.next()?, fields.next()?);
            matches!(kind, "vfat" | "msdos" | "exfat" | "fuseblk")
                .then(|| PathBuf::from(unescape_mount(point)))
        })
        .collect()
}

/// `/proc/mounts` writes space, tab, newline and backslash as octal escapes.
#[cfg(any(target_os = "linux", test))]
fn unescape_mount(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|d| bytes[i] == b'\\' && d.iter().all(|b| (b'0'..=b'7').contains(b)));
        match octal {
            Some(d) => {
                out.push(d.iter().fold(0u8, |n, b| n.wrapping_mul(8) + (b - b'0')));
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Drive letters C: to Z: (A: and B: are floppy drives, slow to probe).
#[cfg(windows)]
fn mount_points() -> Vec<PathBuf> {
    (b'C'..=b'Z')
        .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn mount_points() -> Vec<PathBuf> {
    Vec::new()
}

/// Mounted volumes that hold `GARMIN/GarminDevice.xml`.
pub fn find() -> Vec<PathBuf> {
    mount_points()
        .into_iter()
        .filter(|root| root.join(DEVICE_XML).is_file())
        .collect()
}

/// The one watch volume: `path` if given, else the only volume [`find`] returns.
pub fn locate(path: Option<&Path>) -> Result<PathBuf, Error> {
    if let Some(path) = path {
        return if path.join(DEVICE_XML).is_file() {
            Ok(path.to_owned())
        } else {
            Err(Error::new(
                ErrorCode::GarminFolderNotFound,
                format!("{} has no {DEVICE_XML}.", path.display()),
            ))
        };
    }
    let mut found = find();
    match found.len() {
        0 => Err(Error::new(
            ErrorCode::NoDevice,
            format!("No watch is connected as a USB drive: no mounted volume has {DEVICE_XML}."),
        )),
        1 => Ok(found.remove(0)),
        n => Err(Error::new(
            ErrorCode::MultipleDevices,
            format!(
                "{n} mounted volumes have {DEVICE_XML}: {}. Pass one as path.",
                found
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
    }
}

/// The watch's `GarminDevice.xml`, and its `GARMIN/ExtData` listing (names and sizes only;
/// nothing is read) or `None` without that folder.
pub fn read(root: &Path) -> Result<(GarminDevice, Option<Vec<ListedFile>>), Error> {
    let xml_path = root.join(DEVICE_XML);
    let xml = fs::read(&xml_path).map_err(|e| {
        Error::new(
            ErrorCode::GarminFolderNotFound,
            format!("Could not read {}: {e}", xml_path.display()),
        )
    })?;
    let device = garmin_device::parse(&xml)?;
    let listing = fs::read_dir(root.join(EXTDATA)).ok().map(|dir| {
        let mut files: Vec<ListedFile> = dir
            .flatten()
            .filter_map(|entry| {
                let meta = entry.metadata().ok()?;
                meta.is_file().then(|| ListedFile {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    bytes: meta.len(),
                    sha256: None,
                })
            })
            .collect();
        files.sort_by(|a, b| a.name.cmp(&b.name));
        files
    });
    Ok((device, listing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_escapes_are_decoded() {
        assert_eq!(
            unescape_mount(r"/media/me/GARMIN\040DRIVE"),
            "/media/me/GARMIN DRIVE"
        );
        assert_eq!(unescape_mount(r"/a\134b\0"), r"/a\b\0");
    }

    #[test]
    fn reads_a_volume_without_its_extdata_contents() {
        let root = std::env::temp_dir().join(format!("gfg-drive-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(EXTDATA)).unwrap();
        fs::write(
            root.join(DEVICE_XML),
            "<Device><Model><PartNumber>006-B3076-00</PartNumber>\
             <SoftwareVersion>1370</SoftwareVersion><Description>Forerunner 245</Description>\
             </Model><Id>3398765432</Id></Device>",
        )
        .unwrap();
        fs::write(root.join(EXTDATA).join("b.bin"), [0u8; 5]).unwrap();
        fs::write(root.join(EXTDATA).join("a.tmp"), [0u8; 3]).unwrap();
        assert_eq!(locate(Some(&root)).unwrap(), root);
        let (device, listing) = read(&root).unwrap();
        assert_eq!(device.description, "Forerunner 245");
        assert_eq!(device.software_version, "13.70");
        let listing = listing.unwrap();
        let names: Vec<(&str, u64)> = listing.iter().map(|f| (f.name.as_str(), f.bytes)).collect();
        assert_eq!(names, [("a.tmp", 3), ("b.bin", 5)]);
        assert!(listing.iter().all(|f| f.sha256.is_none()));
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            locate(Some(&root)).unwrap_err().code,
            ErrorCode::GarminFolderNotFound
        );
    }
}
