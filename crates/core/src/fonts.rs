//! Font packs: Garmin's stock originals and the Georgian-extended builds, grouped by pack as the
//! asset manifest lists them (registry order).

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::Serialize;

use crate::garmin_device::GarminDevice;

/// How a pack's fonts reach the watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Garmin's 2-bit bitmap fonts, one part per pixel size, staged as
    /// `GARMIN/ExtData/<part>.tmp` and imported when the watch restarts.
    Bitmap,
    /// Garmin's TrueType fonts, one part per weight, which the watch keeps as
    /// `GARMIN/ExtData/<part>.bin`; installing replaces that file.
    Truetype,
}

struct BundledFont {
    part: &'static str,
    size_px: Option<u16>,
    version: &'static str,
    stock: &'static [u8],
    georgian: &'static [u8],
}

struct BundledTested {
    model: &'static str,
    model_part: &'static str,
    software: &'static str,
    parts: &'static [&'static str],
}

struct BundledPack {
    id: &'static str,
    name: &'static str,
    kind: Kind,
    weight: u16,
    fonts: &'static [BundledFont],
    tested: &'static [BundledTested],
}

static BUNDLED: &[BundledPack] = include!(concat!(env!("OUT_DIR"), "/bundled_fonts.rs"));

/// One size (bitmap) or weight (TrueType) of a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub part: String,
    /// The pack's kind.
    pub kind: Kind,
    /// Nominal pixel size of a bitmap part; `None` for TrueType parts, which scale.
    pub size_px: Option<u16>,
    /// Garmin's release version of the stock file, e.g. "2.00".
    pub version: String,
    pub stock: Cow<'static, [u8]>,
    pub georgian: Cow<'static, [u8]>,
}

impl Font {
    /// The file name in `GARMIN/ExtData`: `<part>.tmp`, which the watch imports, for a bitmap
    /// part; `<part>.bin`, which the watch loads, for a TrueType part.
    pub fn file_name(&self) -> String {
        match self.kind {
            Kind::Bitmap => format!("{}.tmp", self.part),
            Kind::Truetype => format!("{}.bin", self.part),
        }
    }
}

/// A watch on which a pack's Georgian build was seen rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tested {
    /// `Model/Description`, e.g. "Venu 2".
    pub model: String,
    /// `Model/PartNumber`, e.g. "006-B3703-00".
    pub model_part: String,
    /// `Model/SoftwareVersion` with two decimals, e.g. "19.05".
    pub software: String,
    /// The pack's parts seen rendering Georgian.
    pub parts: Vec<String>,
}

/// One Garmin font product in all its sizes or weights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// The weight its stock glyphs are drawn at, e.g. 400; Georgian is drawn at it too (a part
    /// may override it in the registry).
    pub weight: u16,
    /// Every part, in manifest order; each has the pack's kind.
    pub fonts: Vec<Font>,
    pub tested: Vec<Tested>,
}

/// A pack the watch lists and the parts of it the watch lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed<'a> {
    pub pack: &'a Pack,
    /// The pack's fonts the watch lists under `Garmin/ExtData`, in pack order; never empty.
    pub fonts: Vec<&'a Font>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSet {
    /// In registry order.
    pub packs: Vec<Pack>,
}

impl FontSet {
    /// The fonts embedded at build time from `GEORGIAN_FOR_GARMIN_FONTS`.
    pub fn bundled() -> FontSet {
        FontSet {
            packs: BUNDLED
                .iter()
                .map(|p| Pack {
                    id: p.id.to_owned(),
                    name: p.name.to_owned(),
                    kind: p.kind,
                    weight: p.weight,
                    fonts: p
                        .fonts
                        .iter()
                        .map(|f| Font {
                            part: f.part.to_owned(),
                            kind: p.kind,
                            size_px: f.size_px,
                            version: f.version.to_owned(),
                            stock: Cow::Borrowed(f.stock),
                            georgian: Cow::Borrowed(f.georgian),
                        })
                        .collect(),
                    tested: p
                        .tested
                        .iter()
                        .map(|t| Tested {
                            model: t.model.to_owned(),
                            model_part: t.model_part.to_owned(),
                            software: t.software.to_owned(),
                            parts: t.parts.iter().map(|&p| p.to_owned()).collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// Every font of every pack, with its pack.
    pub fn fonts(&self) -> impl Iterator<Item = (&Pack, &Font)> {
        self.packs
            .iter()
            .flat_map(|p| p.fonts.iter().map(move |f| (p, f)))
    }

    /// Whether any pack has part `part`.
    pub fn has_part(&self, part: &str) -> bool {
        self.fonts().any(|(_, f)| f.part == part)
    }

    /// Every pack, in registry order, with a part the watch lists under `Garmin/ExtData`, and
    /// the fonts of it the watch lists: what `install` and `restore_stock` write.
    pub fn listed(&self, device: &GarminDevice) -> Vec<Listed<'_>> {
        self.packs
            .iter()
            .filter_map(|pack| {
                let fonts: Vec<&Font> = pack
                    .fonts
                    .iter()
                    .filter(|f| device.declares_extdata_part(&f.part))
                    .collect();
                (!fonts.is_empty()).then_some(Listed { pack, fonts })
            })
            .collect()
    }
}
