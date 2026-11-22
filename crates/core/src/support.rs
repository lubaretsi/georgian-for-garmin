//! The support list and a connected watch's status in it.
//!
//! The registry's `watches` list every watch model the project knows of: its name, its GitHub
//! issues (the main one first), for a model known not to work, why and on which software
//! versions, and for a model only predicted to work, the font host Garmin's data says it has.
//! Everything else about a watch's status is derived here, from the list, the packs' `tested`
//! entries and the fonts the watch lists ([`support`]); a prediction changes none of it.

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
    /// Set for a model predicted to work from Garmin's data, not tested; never with
    /// `unsupported` or tested entries.
    pub prediction: Option<PredictionFamily>,
}

/// The font host a model is predicted to have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PredictionFamily {
    /// Noto Sans Armenian TrueType (`006-D9971-08/-09`), as on the Forerunner 165: Garmin's
    /// Connect IQ device reference lists it.
    NotoTruetype,
    /// Bitmap Armenian (`006-D9035`), as on the fenix 7 Pro: same firmware family.
    BitmapArmenian,
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
    /// The GitHub issue to follow: the model's main issue.
    pub issue: Option<u32>,
    /// `unsupported`: why, in each language of the app.
    pub reason: Option<Reason>,
    /// The model's prediction in the support list, if it has one. Only for triage: the status
    /// and `report_wanted` come from the fonts the watch lists, so a wrong prediction shows as
    /// `unknown` with a report wanted.
    pub prediction: Option<PredictionFamily>,
    /// A report from this watch adds information: `expected` and `unknown` watches, and
    /// `unsupported` ones on a software version the list doesn't have. A `tested` watch's report
    /// is wanted only if Georgian didn't work after all. `inspect` also sets it for any watch
    /// where a target part's file is one this build doesn't recognise (`other`).
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
            prediction: None,
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
    let issue = watch.and_then(|w| w.issues.first().copied());
    let prediction = watch.and_then(|w| w.prediction);
    if let Some(unsupported) = watch.and_then(|w| w.unsupported.as_ref()) {
        let known = unsupported.software.contains(&device.software_version);
        return Support {
            status: SupportStatus::Unsupported,
            listed,
            issue,
            reason: Some(unsupported.reason.clone()),
            prediction,
            report_wanted: !known,
        };
    }
    let status = if hosts.listed.is_empty() {
        SupportStatus::Unknown
    } else if tested == TestStatus::Tested {
        SupportStatus::Tested
    } else {
        SupportStatus::Expected
    };
    Support {
        status,
        listed,
        issue,
        reason: None,
        prediction,
        report_wanted: status != SupportStatus::Tested,
    }
}
