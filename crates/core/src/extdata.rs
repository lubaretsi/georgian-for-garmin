//! Locating `GARMIN` and its hidden `ExtData` staging folder, read-only.
//!
//! The Venu 2 omits `ExtData` from `GetObjectHandles` scoped to `GARMIN`, and from storage-wide
//! listings when it is empty, yet answers `GetObjectInfo` for its handle. Discovery therefore
//! tries, in order: `GARMIN`'s listed children, the storage-wide listing, parents of staged
//! font files, and finally a bounded probe of early handles in `GARMIN`'s handle range.

use std::collections::{BTreeSet, HashMap};

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::{Error, ErrorCode};
use crate::mtp::{Mtp, MtpError};
use crate::ptp::{ALL_OBJECTS, ObjectInfo, ROOT};

pub const EXTDATA_NAME: &str = "ExtData";
/// Handles probed after `(GARMIN & 0xFF000000)`. On the tested Venu 2, firmware-created folders
/// sit early in `GARMIN`'s range; MTP does not specify this allocation.
pub const PROBE_WINDOW: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub handle: u32,
    pub info: ObjectInfo,
}

/// How `ExtData` was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FoundBy {
    /// Listed among `GARMIN`'s children.
    GarminListing,
    /// Listed in the storage-wide object list.
    StorageListing,
    /// The parent of a staged Garmin font file.
    FontParent,
    /// Unlisted; found by the bounded handle probe.
    HandleProbe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtData {
    pub handle: u32,
    pub listed_under_garmin: bool,
    pub found_by: FoundBy,
    pub files: Vec<Object>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub storage: u32,
    pub garmin: u32,
    pub garmin_device_xml: Option<u32>,
    pub extdata: Option<ExtData>,
}

/// Garmin standalone font packs (Armenian, CJK, Thai, emoji), as staged or downloaded.
fn is_font_package(name: &str) -> bool {
    const SERIES: [&str; 6] = ["8168", "8448", "8944", "8945", "8947", "9035"];
    let n = name.as_bytes();
    n.len() == 16
        && n[..5].eq_ignore_ascii_case(b"006-d")
        && SERIES.iter().any(|s| &n[5..9] == s.as_bytes())
        && n[9] == b'-'
        && n[10].is_ascii_digit()
        && n[11].is_ascii_digit()
        && (n[12..].eq_ignore_ascii_case(b".fnt") || n[12..].eq_ignore_ascii_case(b".tmp"))
}

fn is_extdata(info: &ObjectInfo, garmin: u32, storage: u32) -> bool {
    info.filename == EXTDATA_NAME
        && info.is_folder()
        && info.parent == garmin
        && info.storage_id == storage
}

/// `GetObjectInfo` where a missing handle is an answer, not a failure.
fn optional_info(dev: &mut dyn Mtp, handle: u32) -> Result<Option<ObjectInfo>, MtpError> {
    match dev.object_info(handle) {
        Ok(info) => Ok(Some(info)),
        Err(MtpError::Response { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

fn find_garmin(dev: &mut dyn Mtp) -> Result<Object, Error> {
    let mut found = Vec::new();
    for storage in dev.storage_ids()? {
        for handle in dev.object_handles(storage, ROOT)? {
            let info = dev.object_info(handle)?;
            if info.is_folder() && info.filename.eq_ignore_ascii_case("garmin") {
                found.push(Object { handle, info });
            }
        }
    }
    match found.len() {
        1 => Ok(found.pop().unwrap()),
        0 => Err(Error::new(
            ErrorCode::GarminFolderNotFound,
            "The watch has no GARMIN folder.",
        )),
        n => Err(Error::new(
            ErrorCode::GarminFolderNotFound,
            format!("The watch has {n} GARMIN folders."),
        )),
    }
}

pub fn discover(dev: &mut dyn Mtp) -> Result<Layout, Error> {
    let garmin = find_garmin(dev)?;
    let storage = garmin.info.storage_id;
    let direct: BTreeSet<u32> = dev
        .object_handles(storage, garmin.handle)?
        .into_iter()
        .collect();
    let mut infos: HashMap<u32, ObjectInfo> = HashMap::new();
    for handle in dev.object_handles(storage, ALL_OBJECTS)? {
        let info = dev.object_info(handle)?;
        infos.insert(handle, info);
    }

    let mut found: BTreeSet<u32> = infos
        .iter()
        .filter(|(_, i)| is_extdata(i, garmin.handle, storage))
        .map(|(&h, _)| h)
        .collect();
    let from_listing = found.clone();
    let font_parents: BTreeSet<u32> = infos
        .values()
        .filter(|i| is_font_package(&i.filename))
        .map(|i| i.parent)
        .collect();
    for &handle in font_parents.union(&direct) {
        if found.contains(&handle) {
            continue;
        }
        let info = match infos.get(&handle) {
            Some(info) => info.clone(),
            None => dev.object_info(handle)?,
        };
        if is_extdata(&info, garmin.handle, storage) {
            found.insert(handle);
        }
    }
    if found.len() > 1 {
        return Err(Error::new(
            ErrorCode::ExtdataNotFound,
            format!(
                "The watch reports {} ExtData folders under GARMIN; refusing to choose one.",
                found.len()
            ),
        ));
    }

    let located = match found.pop_first() {
        Some(handle) => Some((
            handle,
            if direct.contains(&handle) {
                FoundBy::GarminListing
            } else if from_listing.contains(&handle) {
                FoundBy::StorageListing
            } else {
                FoundBy::FontParent
            },
        )),
        None => {
            let prefix = garmin.handle & 0xFF00_0000;
            let mut hit = None;
            for handle in prefix + 1..=prefix + PROBE_WINDOW {
                if let Some(info) = optional_info(dev, handle)?
                    && is_extdata(&info, garmin.handle, storage)
                {
                    hit = Some((handle, FoundBy::HandleProbe));
                    break;
                }
            }
            hit
        }
    };

    let lookup = |dev: &mut dyn Mtp, handle: u32| -> Result<Object, Error> {
        let info = match infos.get(&handle) {
            Some(info) => info.clone(),
            None => dev.object_info(handle)?,
        };
        Ok(Object { handle, info })
    };

    let extdata = match located {
        Some((handle, found_by)) => {
            let mut files = Vec::new();
            for child in dev.object_handles(storage, handle)? {
                files.push(lookup(dev, child)?);
            }
            Some(ExtData {
                handle,
                listed_under_garmin: direct.contains(&handle),
                found_by,
                files,
            })
        }
        None => None,
    };

    let mut garmin_device_xml = None;
    for &handle in &direct {
        let object = lookup(dev, handle)?;
        if !object.info.is_folder()
            && object
                .info
                .filename
                .eq_ignore_ascii_case("GarminDevice.xml")
        {
            garmin_device_xml = Some(handle);
            break;
        }
    }

    Ok(Layout {
        storage,
        garmin: garmin.handle,
        garmin_device_xml,
        extdata,
    })
}

#[cfg(test)]
mod tests {
    use super::is_font_package;

    #[test]
    fn font_package_names() {
        for name in ["006-D9035-03.tmp", "006-d8945-20.TMP", "006-D8168-13.fnt"] {
            assert!(is_font_package(name), "{name}");
        }
        for name in [
            "006-D9035-3.tmp",
            "006-D1234-03.tmp",
            "006-D9035-03.bin",
            "006-D9035-03.tmpx",
            "GarminDevice.xml",
        ] {
            assert!(!is_font_package(name), "{name}");
        }
    }
}
