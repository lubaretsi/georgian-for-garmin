//! `extdata_list`: every file in `GARMIN/ExtData`, the normal listing and the hidden one.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorCode};
use crate::extdata::{self, FoundBy};
use crate::fonts::sha256_hex;
use crate::mtp::Mtp;
use crate::ops::{HiddenScan, hex_handle, scan_hidden};
use crate::ptp::ObjectInfo;
use crate::session::HashCache;

const DEFAULT_LIMIT: usize = 200;

/// Which files to list, and which to hash.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtdataListInput {
    /// File names (case-insensitive) to read and give a SHA-256 for, among the files returned.
    pub hash: Option<Vec<String>>,
    /// Files to skip, in name order. Default 0.
    #[schemars(range(min = 0))]
    pub offset: Option<usize>,
    /// Files to return at most. Default 200.
    #[schemars(range(min = 1, max = 1000))]
    pub limit: Option<usize>,
    /// A local path to write the whole list to as JSON (`{"files": [...]}`), instead of
    /// returning a page; an existing file is replaced.
    pub output_file: Option<String>,
}

/// Where `ExtData` is.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ExtdataFolder {
    pub handle: String,
    pub storage_id: String,
    pub found_by: FoundBy,
}

/// Whether the hidden listing was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HiddenListingStatus {
    /// Read; `files` has the hidden ones.
    Listed,
    /// The watch doesn't list Garmin's GetObjectIds.
    Unsupported,
    /// Reading it failed; `note` says why.
    Error,
}

/// A file in `ExtData`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtdataFile {
    pub name: String,
    /// The MTP object handle, e.g. "0x01000123".
    pub handle: String,
    pub bytes: u64,
    /// Only Garmin's GetObjectIds lists it (on the Venu 2, the fonts it imported).
    pub hidden: bool,
    /// The watch marks it read-only; null when this backend can't tell.
    pub read_only: Option<bool>,
    /// Lowercase hex SHA-256, for the names in `hash`; null otherwise.
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExtdataListOutput {
    pub extdata: ExtdataFolder,
    pub hidden_listing: HiddenListingStatus,
    /// Why the hidden listing, or some of it, couldn't be read; null when it could.
    pub note: Option<String>,
    /// Files in both listings.
    pub total: usize,
    pub offset: usize,
    /// The offset of the next page; null on the last one.
    pub next_offset: Option<usize>,
    /// This page, sorted by name; empty when `output_file` is given.
    pub files: Vec<ExtdataFile>,
    /// Where the whole list was written; null without `output_file`.
    pub output_file: Option<String>,
    /// Names in `hash` that no file has.
    pub hash_not_found: Vec<String>,
}

/// The SHA-256 of object `handle`, from `hashes` when this connection read it already.
pub(crate) fn file_sha256(
    dev: &mut dyn Mtp,
    hashes: &mut HashCache,
    handle: u32,
    info: &ObjectInfo,
) -> Result<String, Error> {
    let key = (
        handle,
        info.filename.clone(),
        info.compressed_size,
        info.modification_date.clone(),
    );
    if let Some(hash) = hashes.get(&key) {
        return Ok(hash.clone());
    }
    let hash = sha256_hex(&dev.get_object(handle)?);
    hashes.insert(key, hash.clone());
    Ok(hash)
}

pub fn extdata_list(
    dev: &mut dyn Mtp,
    input: &ExtdataListInput,
    hashes: &mut HashCache,
) -> Result<ExtdataListOutput, Error> {
    let layout = extdata::discover(dev)?;
    let ext = layout.extdata.as_ref().ok_or_else(|| {
        Error::new(
            ErrorCode::ExtdataNotFound,
            "Could not find the watch's GARMIN/ExtData folder.",
        )
    })?;
    let mut objects: Vec<(u32, ObjectInfo, bool)> = ext
        .files
        .iter()
        .filter(|o| !o.info.is_folder())
        .map(|o| (o.handle, o.info.clone(), false))
        .collect();
    let (hidden_listing, note) = match scan_hidden(dev, layout.storage, ext) {
        HiddenScan::Unsupported => (HiddenListingStatus::Unsupported, None),
        HiddenScan::Failed(note) => (HiddenListingStatus::Error, Some(note)),
        HiddenScan::Listed {
            objects: hidden,
            note,
        } => {
            objects.extend(hidden.into_iter().map(|(h, info)| (h, info, true)));
            (HiddenListingStatus::Listed, note)
        }
    };
    objects.sort_by(|a, b| {
        (a.1.filename.to_lowercase(), a.0).cmp(&(b.1.filename.to_lowercase(), b.0))
    });

    let wanted: BTreeSet<String> = input
        .hash
        .iter()
        .flatten()
        .map(|n| n.to_lowercase())
        .collect();
    let hash_not_found = input
        .hash
        .iter()
        .flatten()
        .filter(|n| !objects.iter().any(|o| o.1.filename.eq_ignore_ascii_case(n)))
        .cloned()
        .collect();
    let total = objects.len();
    let offset = input.offset.unwrap_or(0);
    let page: Vec<&(u32, ObjectInfo, bool)> = if input.output_file.is_some() {
        objects.iter().collect()
    } else {
        objects
            .iter()
            .skip(offset)
            .take(input.limit.unwrap_or(DEFAULT_LIMIT))
            .collect()
    };
    let mut files = Vec::with_capacity(page.len());
    for (handle, info, hidden) in page {
        let sha256 = if wanted.contains(&info.filename.to_lowercase()) {
            Some(file_sha256(dev, hashes, *handle, info)?)
        } else {
            None
        };
        files.push(ExtdataFile {
            name: info.filename.clone(),
            handle: hex_handle(*handle),
            bytes: info.compressed_size.into(),
            hidden: *hidden,
            read_only: dev.read_only(*handle, info),
            sha256,
        });
    }
    let folder = ExtdataFolder {
        handle: hex_handle(ext.handle),
        storage_id: hex_handle(layout.storage),
        found_by: ext.found_by,
    };
    if let Some(path) = &input.output_file {
        let text = serde_json::to_string_pretty(&serde_json::json!({ "files": files }))
            .map_err(|e| Error::new(ErrorCode::Internal, e.to_string()))?;
        std::fs::write(path, text + "\n").map_err(|e| {
            Error::new(ErrorCode::InvalidInput, format!("cannot write {path}: {e}"))
        })?;
        return Ok(ExtdataListOutput {
            extdata: folder,
            hidden_listing,
            note,
            total,
            offset: 0,
            next_offset: None,
            files: Vec::new(),
            output_file: Some(path.clone()),
            hash_not_found,
        });
    }
    let end = offset + files.len();
    Ok(ExtdataListOutput {
        extdata: folder,
        hidden_listing,
        note,
        total,
        offset,
        next_offset: (end < total).then_some(end),
        files,
        output_file: None,
        hash_not_found,
    })
}
