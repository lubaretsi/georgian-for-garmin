//! The support list and a connected watch's status in it.
//!
//! The registry's `watches` list every watch model the project knows of: its name, its GitHub
//! issues (the main one first) and, for a model known not to work, why and on which software
//! versions. Everything else about a watch's status is derived here, from the list, the packs'
//! `tested` entries and install settings, and the fonts the watch lists ([`support`]).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fonts::{FontSet, Hosts};
use crate::garmin_device::GarminDevice;
use crate::ops::TestStatus;

/// A watch model in the support list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    /// The name people know it by, e.g. "Venu 2".
    pub model: String,
    /// `Model/PartNumber`, e.g. "006-B3703-00"; unique in the list.
    pub model_part: String,
    /// GitHub issues about it, the main one (the one to follow) first.
    pub issues: Vec<u32>,
    /// Set for a model known not to work.
    pub unsupported: Option<Unsupported>,
}

/// Why a model doesn't work, and the software versions it was reported on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    /// Two decimals, e.g. "13.70".
    pub software: Vec<String>,
    pub reason: Reason,
}

/// A sentence in each language of the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reason {
    pub en: String,
    pub ka: String,
}

/// A watch's status in the support list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    /// This build installs Georgian, and every part it writes was confirmed on this model and
    /// software version (`tested` is `tested`).
    Tested,
    /// This build installs Georgian, but this model and software version aren't confirmed: a
    /// model not tested yet, or a tested one on another software version or with more parts.
    Expected,
    /// Only pre-release builds install Georgian on this watch (a pack with `install: "beta"`),
    /// and this is a stable build. In a pre-release build the same watch is `expected`.
    Beta,
    /// The support list says the model doesn't work, and why.
    Unsupported,
    /// Not in the list as unsupported, and this build has nothing to install on it.
    #[default]
    Unknown,
}

/// A connected watch's status, and whether a report from it adds information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Support {
    pub status: SupportStatus,
    /// The model's part number is in the support list.
    pub listed: bool,
    /// The GitHub issue to follow: the model's main issue, or for a `beta` watch whose model
    /// isn't listed, the issue its pack is tested in.
    pub issue: Option<u32>,
    /// `unsupported`: why, in each language of the app.
    pub reason: Option<Reason>,
    /// A report from this watch adds information: `expected` and `unknown` watches, and
    /// `unsupported` ones on a software version the list doesn't have. A `tested` watch's report
    /// is wanted only if Georgian didn't work after all; a `beta` one's only from a beta build.
    pub report_wanted: bool,
}

impl Default for Support {
    /// A watch with no status known (reports from before this field).
    fn default() -> Self {
        Support {
            status: SupportStatus::Unknown,
            listed: false,
            issue: None,
            reason: None,
            report_wanted: true,
        }
    }
}

/// The status of the watch `device` describes: `hosts` is what this build installs on it
/// ([`FontSet::hosts`]) and `tested` the overall tested status of those parts.
pub fn support(
    fonts: &FontSet,
    device: &GarminDevice,
    hosts: &Hosts<'_>,
    tested: TestStatus,
) -> Support {
    let watch = fonts
        .watches
        .iter()
        .find(|w| w.model_part.eq_ignore_ascii_case(&device.part_number));
    let listed = watch.is_some();
    let main_issue = watch.and_then(|w| w.issues.first().copied());
    if let Some(unsupported) = watch.and_then(|w| w.unsupported.as_ref()) {
        let known = unsupported.software.contains(&device.software_version);
        return Support {
            status: SupportStatus::Unsupported,
            listed,
            issue: main_issue,
            reason: Some(unsupported.reason.clone()),
            report_wanted: !known,
        };
    }
    if !hosts.listed.is_empty() {
        let status = if tested == TestStatus::Tested {
            SupportStatus::Tested
        } else {
            SupportStatus::Expected
        };
        return Support {
            status,
            listed,
            issue: main_issue,
            reason: None,
            report_wanted: status == SupportStatus::Expected,
        };
    }
    // Nothing installs here in this build: would a pre-release build install something?
    let beta = fonts.packs.iter().find(|p| {
        p.beta
            && !p.install
            && p.fonts
                .iter()
                .any(|f| device.declares_extdata_part(&f.part))
    });
    match beta {
        Some(pack) => Support {
            status: SupportStatus::Beta,
            listed,
            issue: main_issue.or(pack.issue),
            reason: None,
            report_wanted: false,
        },
        None => Support {
            status: SupportStatus::Unknown,
            listed,
            issue: main_issue,
            reason: None,
            report_wanted: true,
        },
    }
}
