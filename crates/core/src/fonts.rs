//! Font packs: Garmin's stock originals and the Georgian-extended builds.

use std::borrow::Cow;

pub use crate::parts::PARTS;

struct BundledFont {
    part: &'static str,
    size_px: u16,
    default: bool,
    stock: &'static [u8],
    georgian: &'static [u8],
}

static BUNDLED: &[BundledFont] = include!(concat!(env!("OUT_DIR"), "/bundled_fonts.rs"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub part: String,
    pub size_px: u16,
    pub default: bool,
    pub stock: Cow<'static, [u8]>,
    pub georgian: Cow<'static, [u8]>,
}

impl Font {
    /// The file name the watch imports from `GARMIN/ExtData`.
    pub fn staged_name(&self) -> String {
        format!("{}.tmp", self.part)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSet {
    pub fonts: Vec<Font>,
}

impl FontSet {
    /// The fonts embedded at build time from `GEORGIAN_FOR_GARMIN_FONTS`.
    pub fn bundled() -> FontSet {
        FontSet {
            fonts: BUNDLED
                .iter()
                .map(|f| Font {
                    part: f.part.to_owned(),
                    size_px: f.size_px,
                    default: f.default,
                    stock: Cow::Borrowed(f.stock),
                    georgian: Cow::Borrowed(f.georgian),
                })
                .collect(),
        }
    }

    pub fn by_size(&self, size_px: u16) -> Option<&Font> {
        self.fonts.iter().find(|f| f.size_px == size_px)
    }
}
