//! The support list the app's examples show: the UI fixtures (`tests/ui_fixtures.rs`), the demo
//! build and their tests, in place of the registry's. Feature `example-watches`, debug builds only.
//!
//! The registry changes whenever someone confirms a watch, so examples that read a real model's
//! status from it would change with every confirmation. Here they read it from this list instead.
//! A model confirmed on a software version stays confirmed there, so the tested examples are
//! real models with the registry's own entries ([`TESTED`]). Every other status (not confirmed
//! yet, predicted, unsupported, confirmed on other software) can change, so it is shown on a
//! role watch ([`Role`]): a made-up model name and part number (`006-B99xx-00`, which no Garmin
//! watch has) on a real watch's file listing, the issues of the report that listing comes from,
//! and the registry's own reasons.

use std::sync::LazyLock;

use crate::fonts::{FontSet, Tested};
use crate::support::{PredictionFamily, Reason, Unsupported, Watch};

/// A made-up watch model that shows one support status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Role {
    pub model: &'static str,
    pub model_part: &'static str,
}

/// Expected and predicted to work through Korean bitmap fonts, no issue: the first confirmation.
/// Shown on the Lily 2 Active's listing (short names).
pub const UNCONFIRMED: Role = Role {
    model: "Example Unconfirmed",
    model_part: "006-B9901-00",
};

/// Expected and predicted to work through Korean bitmap fonts, with an issue (#12, whose
/// report's listing it is shown on).
pub const KOREAN: Role = Role {
    model: "Example Korean",
    model_part: "006-B9902-00",
};

/// Expected, not predicted, with an issue (#5).
pub const EXPECTED: Role = Role {
    model: "Example Expected",
    model_part: "006-B9903-00",
};

/// Unsupported on 28.02: the watch doesn't use the font for Georgian (#6, #20).
pub const FONT_UNUSED: Role = Role {
    model: "Example Font Unused",
    model_part: "006-B9904-00",
};

/// Unsupported on 16.12: Georgian in its Korean font didn't show (#57).
pub const KOREAN_UNUSED: Role = Role {
    model: "Example Korean Unused",
    model_part: "006-B9905-00",
};

/// Unsupported on 13.70: it connects only as a USB drive (#16).
pub const DRIVE_ONLY: Role = Role {
    model: "Example Drive Only",
    model_part: "006-B9906-00",
};

/// Predicted to work through Noto Sans Armenian TrueType, no issue.
pub const PREDICTED: Role = Role {
    model: "Example Predicted",
    model_part: "006-B9907-00",
};

/// Confirmed on 29.52 in the Forerunner 255's bitmap Armenian parts (#13), so on any other
/// software confirmed on other software.
pub const OTHER_SOFTWARE: Role = Role {
    model: "Example Other Software",
    model_part: "006-B9908-00",
};

/// The tested entries: (pack id, model part, software, parts). The real ones are the registry's.
pub const TESTED: &[(&str, &str, &str, &[&str])] = &[
    (
        "armenian-noto-sans-regular",
        "006-B3703-00",
        "19.05",
        &[
            "006-D9035-01",
            "006-D9035-02",
            "006-D9035-03",
            "006-D9035-04",
        ],
    ),
    (
        "armenian-noto-sans-regular",
        "006-B4135-00",
        "27.18",
        &["006-D9035-00"],
    ),
    (
        "armenian-noto-sans-regular",
        OTHER_SOFTWARE.model_part,
        "29.52",
        &["006-D9035-00"],
    ),
    (
        "armenian-noto-sans-armenian-regular",
        OTHER_SOFTWARE.model_part,
        "29.52",
        &["006-D9035-18"],
    ),
    (
        "armenian-noto-sans-armenian-bold",
        "006-B4135-00",
        "27.18",
        &[
            "006-D9035-05",
            "006-D9035-06",
            "006-D9035-07",
            "006-D9035-09",
        ],
    ),
    (
        "armenian-noto-sans-armenian-bold",
        OTHER_SOFTWARE.model_part,
        "29.52",
        &[
            "006-D9035-05",
            "006-D9035-06",
            "006-D9035-07",
            "006-D9035-08",
            "006-D9035-09",
            "006-D9035-17",
        ],
    ),
    (
        "armenian-noto-sans-armenian-truetype",
        "006-B4432-00",
        "29.52",
        &["006-D9971-09", "006-D9971-08"],
    ),
    (
        "hebrew-noto-sans-hebrew-truetype",
        "006-B4432-00",
        "29.52",
        &["006-D1053-98", "006-D1053-99"],
    ),
];

/// The registry's reasons, by id: the words a role watch's unsupported status shows.
static REASONS: LazyLock<serde_json::Value> = LazyLock::new(|| {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../../../fonts/registry.json"))
            .expect("fonts/registry.json is JSON");
    registry["reasons"].clone()
});

fn reason(id: &str) -> Reason {
    let entry = REASONS
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("fonts/registry.json has no reason {id}"));
    serde_json::from_value(serde_json::json!({"en": entry["en"], "ka": entry["ka"]}))
        .expect("a reason has en and ka")
}

fn watch(model: &str, model_part: &str, issues: &[u32]) -> Watch {
    Watch {
        model: model.into(),
        model_part: model_part.into(),
        issues: issues.to_vec(),
        unsupported: None,
        prediction: None,
    }
}

fn role(role: Role, issues: &[u32]) -> Watch {
    watch(role.model, role.model_part, issues)
}

fn unsupported(software: &str, reason_id: &str) -> Option<Unsupported> {
    Some(Unsupported {
        software: vec![software.into()],
        reason: reason(reason_id),
    })
}

/// The support list: the tested real models, with the registry's issues, then the role watches.
pub fn watches() -> Vec<Watch> {
    vec![
        watch("Venu 2", "006-B3703-00", &[]),
        watch("tactix 7", "006-B4135-00", &[]),
        watch("Forerunner 165", "006-B4432-00", &[5, 18, 19, 22]),
        Watch {
            prediction: Some(PredictionFamily::BitmapKorean),
            ..role(UNCONFIRMED, &[])
        },
        Watch {
            prediction: Some(PredictionFamily::BitmapKorean),
            ..role(KOREAN, &[12])
        },
        role(EXPECTED, &[5]),
        Watch {
            unsupported: unsupported("28.02", "font_unused"),
            ..role(FONT_UNUSED, &[6, 20])
        },
        Watch {
            unsupported: unsupported("16.12", "korean_font_unused"),
            ..role(KOREAN_UNUSED, &[57])
        },
        Watch {
            unsupported: unsupported("13.70", "usb_drive_only"),
            ..role(DRIVE_ONLY, &[16])
        },
        Watch {
            prediction: Some(PredictionFamily::NotoTruetype),
            ..role(PREDICTED, &[])
        },
        role(OTHER_SOFTWARE, &[13]),
    ]
}

/// This build's fonts with this list's support list and tested entries in place of the
/// registry's. Panics where [`TESTED`] names a pack or part this build lacks.
pub fn fonts() -> FontSet {
    let mut fonts = FontSet::bundled();
    for pack in &mut fonts.packs {
        pack.tested.clear();
    }
    for &(id, model_part, software, parts) in TESTED {
        let pack = fonts
            .packs
            .iter_mut()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("example tested entry: no pack {id}"));
        for part in parts {
            assert!(
                pack.fonts.iter().any(|f| f.part == *part),
                "example tested entry: pack {id} has no part {part}"
            );
        }
        pack.tested.push(Tested {
            model_part: model_part.into(),
            software: software.into(),
            parts: parts.iter().map(|&p| p.to_owned()).collect(),
        });
    }
    fonts.watches = watches();
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tested_model_is_listed_and_every_role_is_made_up() {
        let fonts = fonts();
        for pack in &fonts.packs {
            for t in &pack.tested {
                let watch = fonts.watches.iter().find(|w| w.model_part == t.model_part);
                assert!(
                    watch.is_some_and(|w| w.unsupported.is_none() && w.prediction.is_none()),
                    "{}: {}",
                    pack.id,
                    t.model_part
                );
            }
        }
        // A role's part number is no real watch's: the registry lists none of them.
        let real = FontSet::bundled();
        for w in fonts
            .watches
            .iter()
            .filter(|w| w.model.starts_with("Example "))
        {
            assert!(w.model_part.starts_with("006-B99"), "{}", w.model);
            assert!(
                !real.watches.iter().any(|r| r.model_part == w.model_part),
                "{} is a registry model's part number",
                w.model
            );
        }
        // The real models are tested in the registry, with these very entries.
        for w in fonts
            .watches
            .iter()
            .filter(|w| !w.model.starts_with("Example "))
        {
            let mut entries = 0;
            for pack in &fonts.packs {
                let registry = real.packs.iter().find(|p| p.id == pack.id).unwrap();
                for t in pack.tested.iter().filter(|t| t.model_part == w.model_part) {
                    entries += 1;
                    assert!(
                        registry.tested.contains(t),
                        "{}: {} on {} isn't the registry's entry",
                        pack.id,
                        w.model,
                        t.software
                    );
                }
            }
            assert!(entries > 0, "{} has no tested entry", w.model);
        }
    }
}
