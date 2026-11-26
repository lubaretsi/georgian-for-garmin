//! A watch connected in Garmin mode: Garmin's own USB protocol (`091e:0003`) instead of MTP, as
//! a watch whose USB Mode setting is "Garmin" connects when nobody answers its "Use MTP?"
//! question. Connecting fails with `garmin_mode` for it; `switch_to_mtp` asks it to come back as
//! MTP, as Garmin Express does ([`crate::garmin_usb`] on macOS and Linux).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// USB product ID of a Garmin device in Garmin mode.
pub const GARMIN_MODE_PRODUCT_ID: u16 = 0x0003;

/// What a `garmin_mode` error says about switching the watch here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GarminMode {
    /// Whether `switch_to_mtp` can try to switch the watch on this computer.
    pub switchable: bool,
    /// Why it can't, when it can't.
    pub reason: Option<NotSwitchable>,
}

/// Why `switch_to_mtp` can't switch a watch in Garmin mode on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotSwitchable {
    /// Windows: this program can't talk to a device in Garmin mode (that goes through Garmin's
    /// own USB driver).
    NoDriver,
}

impl GarminMode {
    /// What this build can do for a watch in Garmin mode on this system.
    pub fn here() -> Self {
        if cfg!(windows) {
            GarminMode {
                switchable: false,
                reason: Some(NotSwitchable::NoDriver),
            }
        } else {
            GarminMode {
                switchable: true,
                reason: None,
            }
        }
    }

    /// The error's message.
    pub(crate) fn message(&self) -> String {
        let manual = "On the watch, choose MTP when it asks, or unplug it and connect it again. To \
                      stop the question for good: Settings › System › USB Mode › MTP.";
        if self.switchable {
            format!(
                "The watch is connected in Garmin mode, not in file-transfer (MTP) mode. \
                 switch_to_mtp (switch-to-mtp) switches it. {manual}"
            )
        } else {
            format!(
                "The watch is connected in Garmin mode, not in file-transfer (MTP) mode. {manual}"
            )
        }
    }
}

/// `switch_to_mtp`'s input: none.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SwitchInput {}

/// How the switch ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwitchStatus {
    /// A Garmin MTP device appeared after the request.
    Switched,
    /// The request was sent, but no Garmin MTP device appeared within the wait.
    TimedOut,
}

/// The product data the watch sends in Garmin mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Product {
    /// Garmin's product number (3703 for the Venu 2, as in its part number 006-B3703-00), not
    /// the USB product ID.
    pub product_id: u16,
    /// The software version, e.g. "19.05".
    pub software_version: String,
    /// The watch's own description, e.g. "VENU 2 V19.05".
    pub description: String,
}

/// `switch_to_mtp`'s output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SwitchOutput {
    pub status: SwitchStatus,
    /// The watch's product data, when it sent it before its protocol list.
    pub product: Option<Product>,
    /// The protocols the watch lists, e.g. ["P000", "L001", "A010", "A903", "A1050"]; A1050 is
    /// the switch to MTP.
    pub protocols: Vec<String>,
    /// Seconds from the request until a Garmin MTP device appeared, or until the wait ended.
    pub waited_seconds: f64,
}

/// Whether Windows lists a Garmin device in Garmin mode, from its device IDs only: nothing is
/// opened, and nothing of the IDs is kept.
#[cfg(windows)]
pub(crate) fn listed_on_windows() -> bool {
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_GETIDLIST_FILTER_ENUMERATOR, CM_GETIDLIST_FILTER_PRESENT, CM_Get_Device_ID_List_SizeW,
        CM_Get_Device_ID_ListW, CR_SUCCESS,
    };
    use windows::core::w;

    let flags = CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT;
    let mut len = 0u32;
    if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, w!("USB"), flags) } != CR_SUCCESS {
        return false;
    }
    let mut buffer = vec![0u16; len as usize];
    if unsafe { CM_Get_Device_ID_ListW(w!("USB"), &mut buffer, flags) } != CR_SUCCESS {
        return false;
    }
    let prefix = format!(
        "USB\\VID_{:04X}&PID_{GARMIN_MODE_PRODUCT_ID:04X}\\",
        crate::GARMIN_VENDOR_ID
    );
    buffer
        .split(|&c| c == 0)
        .map(String::from_utf16_lossy)
        .any(|id| id.to_ascii_uppercase().starts_with(&prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_names_the_watch_s_own_way_out() {
        let here = GarminMode::here();
        assert_eq!(here.switchable, !cfg!(windows));
        for mode in [
            GarminMode {
                switchable: true,
                reason: None,
            },
            GarminMode {
                switchable: false,
                reason: Some(NotSwitchable::NoDriver),
            },
        ] {
            let message = mode.message();
            assert!(message.contains("USB Mode › MTP"), "{message}");
            assert_eq!(
                message.contains("switch_to_mtp"),
                mode.switchable,
                "{message}"
            );
        }
    }
}
