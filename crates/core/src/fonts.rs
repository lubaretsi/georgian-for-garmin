//! Font packs: Garmin's stock originals and the Georgian-extended builds, grouped by pack as the
//! asset manifest lists them (registry order).

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::garmin_device::GarminDevice;
use crate::names::{self, Role};
use crate::support::{PredictionFamily, Reason, Unsupported, Watch};

/// How a pack's fonts reach the watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    mark: Option<BundledMark>,
    earlier: &'static [BundledEarlier],
}

struct BundledEarlier {
    build: &'static str,
    sha256: &'static str,
    bytes: u64,
}

struct BundledMark {
    variant: &'static str,
    code_point: &'static str,
    position: u8,
    style: &'static str,
    picture: &'static [u8],
}

struct BundledTested {
    model_part: &'static str,
    software: &'static str,
    parts: &'static [&'static str],
}

struct BundledPack {
    id: &'static str,
    name: &'static str,
    kind: Kind,
    host: &'static str,
    weight: u16,
    install: bool,
    fonts: &'static [BundledFont],
    tested: &'static [BundledTested],
}

struct BundledUnsupported {
    software: &'static [&'static str],
    reason_en: &'static str,
    reason_ka: &'static str,
}

struct BundledWatch {
    model: &'static str,
    model_part: &'static str,
    issues: &'static [u32],
    unsupported: Option<BundledUnsupported>,
    prediction: Option<PredictionFamily>,
}

static BUNDLED: &[BundledPack] = include!(concat!(env!("OUT_DIR"), "/bundled_fonts.rs"));
static BUNDLED_WATCHES: &[BundledWatch] = include!(concat!(env!("OUT_DIR"), "/bundled_watches.rs"));

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
    /// The beta test mark drawn into `georgian`; `None` in stable builds, whose Georgian fonts
    /// carry no marks.
    pub mark: Option<Mark>,
    /// Georgian builds of this part that earlier releases installed (beta test marks), which
    /// this build recognizes as its own and replaces.
    pub earlier: Vec<Earlier>,
}

/// A Georgian build an earlier release installed, known by size and SHA-256.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Earlier {
    /// The release, e.g. "0.3.0-beta.2".
    pub build: String,
    /// Lowercase hex.
    pub sha256: String,
    pub bytes: u64,
}

/// What a file named for a part holds, by its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    /// This build's Georgian font.
    Georgian,
    /// A Georgian build an earlier release installed.
    EarlierGeorgian,
    /// Garmin's original.
    Stock,
    /// Anything else.
    Other,
}

/// Capitals of the test text a beta build's marks go on, in order: `Mark::position` indexes it.
pub const MARKED_CAPITALS: &str = "ᲛᲗᲐᲕᲠᲣᲚᲘ";

/// A beta build's test mark: one capital of [`MARKED_CAPITALS`] drawn with a bar, different in
/// every part of a kind, so a notification shows which part rendered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    /// "A", "B", …, in registry order among the parts of one kind.
    pub variant: String,
    /// The marked capital, e.g. "U+1C90".
    pub code_point: String,
    /// Its index in the test word ᲛᲗᲐᲕᲠᲣᲚᲘ, from 0.
    pub position: u8,
    /// Where the bar is: "below", "above" or "both".
    pub style: String,
    /// PNG of the test word in this part's Georgian build, white on black like the watch.
    pub picture: Cow<'static, [u8]>,
}

impl Font {
    /// What this part's own file in `GARMIN/ExtData` is: a bitmap part's `.tmp`, which the watch
    /// imports at the next restart; a TrueType part's `.bin`, which the watch loads.
    pub fn target_role(&self) -> Role {
        match self.kind {
            Kind::Bitmap => Role::Waiting,
            Kind::Truetype => Role::Bin,
        }
    }

    /// The name this build writes the part under in `GARMIN/ExtData`: the file name `device`
    /// declares for it in `GarminDevice.xml`, in the watch's own name form (the Lily 2 Active
    /// declares `D894567.tmp`, the Venu 2 `006-D9035-03.tmp`), so the watch imports or loads
    /// it; [`Font::long_file_name`] when the watch declares no file name that [`names`] resolves
    /// to this part's own file.
    pub fn file_name(&self, device: &GarminDevice) -> String {
        device
            .extdata_files()
            .find(|d| {
                d.part_number == self.part
                    && names::is(&d.file_name, &self.part, self.target_role())
            })
            .map_or_else(|| self.long_file_name(), |d| d.file_name.clone())
    }

    /// The part's file name in the part-number form: `<part>.tmp` for a bitmap part,
    /// `<part>.bin` for a TrueType part.
    pub fn long_file_name(&self) -> String {
        match self.kind {
            Kind::Bitmap => format!("{}.tmp", self.part),
            Kind::Truetype => format!("{}.bin", self.part),
        }
    }

    /// Whether a file of `size` bytes may be one this build knows (stock, Georgian or an
    /// earlier Georgian build), so it is worth reading.
    pub fn known_size(&self, size: usize) -> bool {
        size == self.stock.len()
            || size == self.georgian.len()
            || self.earlier.iter().any(|e| e.bytes == size as u64)
    }

    /// What `bytes`, a file named for this part, holds.
    pub fn identify(&self, bytes: &[u8]) -> Content {
        if *bytes == *self.georgian {
            Content::Georgian
        } else if *bytes == *self.stock {
            Content::Stock
        } else if self.earlier.iter().any(|e| e.bytes == bytes.len() as u64) && {
            let hash = sha256_hex(bytes);
            self.earlier
                .iter()
                .any(|e| e.bytes == bytes.len() as u64 && e.sha256 == hash)
        } {
            Content::EarlierGeorgian
        } else {
            Content::Other
        }
    }
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A watch on which a pack's Georgian build was seen rendering. Its model is in the support
/// list ([`FontSet::watches`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tested {
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
    /// The registry's host family of its fonts, e.g. "armenian", "korean",
    /// "noto-armenian-truetype".
    pub host: String,
    /// The weight its stock glyphs are drawn at, e.g. 400; Georgian is drawn at it too (a part
    /// may override it in the registry).
    pub weight: u16,
    /// Whether `install` writes this pack's Georgian build. A pack whose Georgian didn't render
    /// on a watch is restore-only: `restore_stock` still writes Garmin's original, and its
    /// Georgian builds are still recognized.
    pub install: bool,
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
    /// The support list: every watch model the project knows of, in registry order.
    pub watches: Vec<Watch>,
}

impl FontSet {
    /// The fonts and support list embedded at build time from `GEORGIAN_FOR_GARMIN_FONTS`.
    pub fn bundled() -> FontSet {
        FontSet {
            watches: BUNDLED_WATCHES
                .iter()
                .map(|w| Watch {
                    model: w.model.to_owned(),
                    model_part: w.model_part.to_owned(),
                    issues: w.issues.to_vec(),
                    unsupported: w.unsupported.as_ref().map(|u| Unsupported {
                        software: u.software.iter().map(|&s| s.to_owned()).collect(),
                        reason: Reason {
                            en: u.reason_en.to_owned(),
                            ka: u.reason_ka.to_owned(),
                        },
                    }),
                    prediction: w.prediction,
                })
                .collect(),
            packs: BUNDLED
                .iter()
                .map(|p| Pack {
                    id: p.id.to_owned(),
                    name: p.name.to_owned(),
                    kind: p.kind,
                    host: p.host.to_owned(),
                    weight: p.weight,
                    install: p.install,
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
                            mark: f.mark.as_ref().map(|m| Mark {
                                variant: m.variant.to_owned(),
                                code_point: m.code_point.to_owned(),
                                position: m.position,
                                style: m.style.to_owned(),
                                picture: Cow::Borrowed(m.picture),
                            }),
                            earlier: f
                                .earlier
                                .iter()
                                .map(|e| Earlier {
                                    build: e.build.to_owned(),
                                    sha256: e.sha256.to_owned(),
                                    bytes: e.bytes,
                                })
                                .collect(),
                        })
                        .collect(),
                    tested: p
                        .tested
                        .iter()
                        .map(|t| Tested {
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

    /// Whether this is a beta build whose Georgian fonts carry test marks.
    pub fn test_marks(&self) -> bool {
        self.fonts().any(|(_, f)| f.mark.is_some())
    }

    /// What `install` writes on the watch `device` describes: the installable packs of one host
    /// with a part the watch lists under `Garmin/ExtData`, the other installable parts it lists,
    /// which are left alone, and the restore-only parts it lists.
    ///
    /// Registry order decides the host: the first installable pack the watch lists chooses it.
    /// A bitmap pack chooses its own host family alone (a watch that lists bitmap Armenian gets
    /// Georgian there, and its Korean fonts stay Garmin's); a TrueType pack chooses every
    /// installable TrueType pack. The bitmap Armenian packs come first, then the TrueType packs,
    /// then the Korean ones, so a watch gets Georgian in Korean fonts only when it lists none of
    /// the others.
    pub fn hosts(&self, device: &GarminDevice) -> Hosts<'_> {
        self.hosts_of(device, false)
    }

    /// What `restore_stock` writes: as [`FontSet::hosts`], with every restore-only pack the
    /// watch lists too, whatever host install chooses, so Garmin's originals can go back where
    /// an earlier release installed Georgian.
    pub fn restore_hosts(&self, device: &GarminDevice) -> Hosts<'_> {
        self.hosts_of(device, true)
    }

    fn hosts_of(&self, device: &GarminDevice, restore: bool) -> Hosts<'_> {
        let all: Vec<Listed<'_>> = self
            .packs
            .iter()
            .filter_map(|pack| {
                let fonts: Vec<&Font> = pack
                    .fonts
                    .iter()
                    .filter(|f| device.declares_extdata_part(&f.part))
                    .collect();
                (!fonts.is_empty()).then_some(Listed { pack, fonts })
            })
            .collect();
        // The first installable pack the watch lists; for restore without one, the first
        // restore-only pack, whose kind `kind` then reports.
        let first = all
            .iter()
            .find(|l| l.pack.install)
            .or_else(|| all.first().filter(|_| restore))
            .map(|l| l.pack);
        let chosen = |p: &Pack| {
            first
                .is_some_and(|f| p.kind == f.kind && (p.kind == Kind::Truetype || p.host == f.host))
        };
        let mut hosts = Hosts {
            kind: first.map(|p| p.kind),
            listed: Vec::new(),
            skipped: Vec::new(),
            restore_only: Vec::new(),
        };
        for listed in all {
            if !listed.pack.install {
                if restore {
                    hosts.listed.push(listed);
                } else {
                    hosts.restore_only.extend(listed.fonts);
                }
            } else if chosen(listed.pack) {
                hosts.listed.push(listed);
            } else {
                hosts.skipped.extend(listed.fonts);
            }
        }
        hosts
    }
}

/// The fonts a watch gets Georgian in ([`FontSet::hosts`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosts<'a> {
    /// The kind of the chosen host's packs; `None` when the watch lists no part this build
    /// installs (for restore: no part of this build at all).
    pub kind: Option<Kind>,
    /// Every pack of the chosen host, in registry order, with the parts of it the watch lists;
    /// for [`FontSet::restore_hosts`], every restore-only pack it lists too.
    pub listed: Vec<Listed<'a>>,
    /// Other installable parts the watch lists (of another host), in registry order; left
    /// alone.
    pub skipped: Vec<&'a Font>,
    /// Parts of restore-only packs the watch lists (empty for [`FontSet::restore_hosts`]):
    /// install leaves them alone; restore writes Garmin's originals.
    pub restore_only: Vec<&'a Font>,
}
