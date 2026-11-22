//! Embeds the font asset directory named by `GEORGIAN_FOR_GARMIN_FONTS`, after checking every
//! listed file against the SHA-256 in its `manifest.json` and every font against its pack, and
//! the manifest's support list (the registry's `watches`).

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::exit;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const FONTS_ENV: &str = "GEORGIAN_FOR_GARMIN_FONTS";

#[derive(Deserialize)]
struct Manifest {
    fonts: Vec<Entry>,
    packs: Vec<PackEntry>,
    /// Beta builds: every Georgian font carries a test mark. Absent in older manifests.
    #[serde(default)]
    test_marks: bool,
    /// The support list. Absent in older manifests: every watch's status is derived from its
    /// fonts alone.
    #[serde(default)]
    watches: Vec<WatchEntry>,
}

#[derive(Deserialize)]
struct WatchEntry {
    model: String,
    model_part: String,
    issues: Vec<u32>,
    #[serde(default)]
    unsupported: Option<UnsupportedEntry>,
}

#[derive(Deserialize)]
struct UnsupportedEntry {
    software: Vec<String>,
    reason: ReasonEntry,
}

#[derive(Deserialize)]
struct ReasonEntry {
    en: String,
    ka: String,
}

#[derive(Deserialize)]
struct Entry {
    pack: String,
    part: String,
    version: String,
    /// Bitmap parts only.
    size_px: Option<u16>,
    stock: String,
    stock_sha256: String,
    georgian: String,
    georgian_sha256: String,
    /// Present exactly when the manifest's `test_marks` is.
    mark: Option<MarkEntry>,
    /// Georgian builds earlier releases installed. Absent in older manifests.
    #[serde(default)]
    earlier_georgian: Vec<EarlierEntry>,
}

#[derive(Deserialize)]
struct EarlierEntry {
    build: String,
    sha256: String,
    bytes: u64,
}

#[derive(Deserialize)]
struct MarkEntry {
    variant: String,
    code_point: String,
    position: u8,
    style: String,
    picture: String,
    picture_sha256: String,
}

#[derive(Deserialize)]
struct PackEntry {
    id: String,
    name: String,
    kind: String,
    weight: u16,
    parts: Vec<String>,
    tested: Vec<TestedEntry>,
    /// false: restore only. Absent in older manifests: true.
    #[serde(default = "yes")]
    install: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct TestedEntry {
    model_part: String,
    software: String,
    parts: Vec<String>,
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("error: {}", message.as_ref());
    exit(1);
}

fn checked_file(dir: &Path, relative: &str, expected_sha256: &str) -> String {
    let rel = Path::new(relative);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        fail(format!(
            "manifest path {relative:?} must stay inside {}",
            dir.display()
        ));
    }
    let path = dir.join(rel);
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes =
        fs::read(&path).unwrap_or_else(|e| fail(format!("cannot read {}: {e}", path.display())));
    let mut actual = String::with_capacity(64);
    for byte in Sha256::digest(&bytes) {
        write!(actual, "{byte:02x}").unwrap();
    }
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        fail(format!(
            "{} has SHA-256 {actual}, but the manifest lists {expected_sha256}",
            path.display()
        ));
    }
    path.into_os_string()
        .into_string()
        .unwrap_or_else(|_| fail("font paths must be valid UTF-8"))
}

/// Checks that the packs and fonts describe each other exactly, then writes the pack table.
fn pack_table(dir: &Path, manifest: &Manifest, manifest_path: &Path) -> String {
    let where_ = manifest_path.display();
    if manifest.packs.is_empty() {
        fail(format!("{where_} lists no packs"));
    }
    let mut ids = BTreeSet::new();
    let mut parts = BTreeSet::new();
    for font in &manifest.fonts {
        if !parts.insert(font.part.as_str()) {
            fail(format!("{where_}: font part {} is listed twice", font.part));
        }
        if !manifest.packs.iter().any(|p| p.id == font.pack) {
            fail(format!(
                "{where_}: font part {} names unknown pack {:?}",
                font.part, font.pack
            ));
        }
        if font.mark.is_some() != manifest.test_marks {
            fail(format!(
                "{where_}: font part {} {} a test mark, but test_marks is {}",
                font.part,
                if font.mark.is_some() { "has" } else { "lacks" },
                manifest.test_marks
            ));
        }
    }
    let mut code = String::from("&[\n");
    for pack in &manifest.packs {
        if !ids.insert(pack.id.as_str()) {
            fail(format!("{where_}: pack {:?} is listed twice", pack.id));
        }
        if pack.parts.is_empty() {
            fail(format!("{where_}: pack {:?} has no parts", pack.id));
        }
        let fonts: Vec<&Entry> = manifest
            .fonts
            .iter()
            .filter(|f| f.pack == pack.id)
            .collect();
        let listed: BTreeSet<&str> = pack.parts.iter().map(String::as_str).collect();
        let carried: BTreeSet<&str> = fonts.iter().map(|f| f.part.as_str()).collect();
        if listed.len() != pack.parts.len() || listed != carried {
            fail(format!(
                "{where_}: pack {:?} lists parts {:?} but its fonts are {:?}",
                pack.id, pack.parts, carried
            ));
        }
        let kind = match pack.kind.as_str() {
            "bitmap" => "Bitmap",
            "truetype" => "Truetype",
            other => fail(format!(
                "{where_}: pack {:?} has unknown kind {other:?}",
                pack.id
            )),
        };
        let mut sizes = BTreeSet::new();
        writeln!(
            code,
            "    BundledPack {{ id: {:?}, name: {:?}, kind: Kind::{kind}, weight: {}, \
             install: {}, fonts: &[",
            pack.id, pack.name, pack.weight, pack.install
        )
        .unwrap();
        for part in &pack.parts {
            let font = fonts.iter().find(|f| &f.part == part).expect("checked");
            match (kind, font.size_px) {
                ("Bitmap", Some(size)) => {
                    if !sizes.insert(size) {
                        fail(format!(
                            "{where_}: pack {:?} has two {size} px fonts",
                            pack.id
                        ));
                    }
                }
                ("Bitmap", None) => fail(format!(
                    "{where_}: bitmap font part {} has no size_px",
                    font.part
                )),
                (_, Some(_)) => fail(format!(
                    "{where_}: TrueType font part {} has a size_px",
                    font.part
                )),
                (_, None) => {}
            }
            let stock = checked_file(dir, &font.stock, &font.stock_sha256);
            let georgian = checked_file(dir, &font.georgian, &font.georgian_sha256);
            let mark = match &font.mark {
                None => "None".to_owned(),
                Some(m) => {
                    if !matches!(m.style.as_str(), "below" | "above" | "both")
                        || m.position >= 8
                        || m.variant.len() != 1
                    {
                        fail(format!(
                            "{where_}: font part {} has an invalid mark",
                            font.part
                        ));
                    }
                    let picture = checked_file(dir, &m.picture, &m.picture_sha256);
                    format!(
                        "Some(BundledMark {{ variant: {:?}, code_point: {:?}, position: {}, \
                         style: {:?}, picture: include_bytes!({picture:?}) }})",
                        m.variant, m.code_point, m.position, m.style
                    )
                }
            };
            let mut earlier = String::from("&[");
            for e in &font.earlier_georgian {
                let valid = e.sha256.len() == 64
                    && e.sha256
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
                if !valid || e.bytes == 0 {
                    fail(format!(
                        "{where_}: font part {} has an invalid earlier_georgian entry",
                        font.part
                    ));
                }
                if e.sha256 == font.stock_sha256 {
                    fail(format!(
                        "{where_}: font part {}'s earlier Georgian build from {} is its stock font",
                        font.part, e.build
                    ));
                }
                write!(
                    earlier,
                    "BundledEarlier {{ build: {:?}, sha256: {:?}, bytes: {} }}, ",
                    e.build, e.sha256, e.bytes
                )
                .unwrap();
            }
            earlier.push(']');
            writeln!(
                code,
                "        BundledFont {{ part: {:?}, size_px: {:?}, version: {:?}, \
                 stock: include_bytes!({stock:?}), georgian: include_bytes!({georgian:?}), \
                 mark: {mark}, earlier: {earlier} }},",
                font.part, font.size_px, font.version,
            )
            .unwrap();
        }
        code.push_str("    ], tested: &[\n");
        for tested in &pack.tested {
            if let Some(part) = tested.parts.iter().find(|p| !listed.contains(p.as_str())) {
                fail(format!(
                    "{where_}: pack {:?} lists tested part {part}, which it doesn't have",
                    pack.id
                ));
            }
            writeln!(
                code,
                "        BundledTested {{ model_part: {:?}, software: {:?}, parts: &{:?} }},",
                tested.model_part, tested.software, tested.parts
            )
            .unwrap();
        }
        code.push_str("    ] },\n");
    }
    code.push_str("]\n");
    code
}

/// Checks the support list against the packs, then writes it.
fn watch_table(manifest: &Manifest, manifest_path: &Path) -> String {
    let where_ = manifest_path.display();
    let mut parts = BTreeSet::new();
    for watch in &manifest.watches {
        if !parts.insert(watch.model_part.as_str()) {
            fail(format!(
                "{where_}: watch {} is listed twice",
                watch.model_part
            ));
        }
        if watch.unsupported.is_some() && watch.issues.is_empty() {
            fail(format!(
                "{where_}: unsupported watch {} names no issue",
                watch.model_part
            ));
        }
    }
    // Older manifests have no support list.
    if !manifest.watches.is_empty() {
        for pack in &manifest.packs {
            if let Some(t) = pack
                .tested
                .iter()
                .find(|t| !parts.contains(t.model_part.as_str()))
            {
                fail(format!(
                    "{where_}: pack {:?} was tested on {}, which the support list lacks",
                    pack.id, t.model_part
                ));
            }
        }
    }
    let mut code = String::from("&[\n");
    for watch in &manifest.watches {
        let unsupported = match &watch.unsupported {
            None => "None".to_owned(),
            Some(u) => format!(
                "Some(BundledUnsupported {{ software: &{:?}, reason_en: {:?}, reason_ka: {:?} }})",
                u.software, u.reason.en, u.reason.ka
            ),
        };
        writeln!(
            code,
            "    BundledWatch {{ model: {:?}, model_part: {:?}, issues: &{:?}, unsupported: \
             {unsupported} }},",
            watch.model, watch.model_part, watch.issues
        )
        .unwrap();
    }
    code.push_str("]\n");
    code
}

fn main() {
    println!("cargo:rerun-if-env-changed={FONTS_ENV}");
    let Some(dir) = env::var_os(FONTS_ENV) else {
        fail(format!(
            "{FONTS_ENV} is not set; point it at a font asset directory made by the font builder \
             (manifest.json, stock/, georgian/)"
        ));
    };
    let dir = fs::canonicalize(&dir)
        .unwrap_or_else(|e| fail(format!("{FONTS_ENV}={}: {e}", Path::new(&dir).display())));
    let manifest_path = dir.join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(&manifest_path)
            .unwrap_or_else(|e| fail(format!("cannot read {}: {e}", manifest_path.display()))),
    )
    .unwrap_or_else(|e| fail(format!("invalid {}: {e}", manifest_path.display())));

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    for (name, code) in [
        (
            "bundled_fonts.rs",
            pack_table(&dir, &manifest, &manifest_path),
        ),
        ("bundled_watches.rs", watch_table(&manifest, &manifest_path)),
    ] {
        let out = out_dir.join(name);
        fs::write(&out, code)
            .unwrap_or_else(|e| fail(format!("cannot write {}: {e}", out.display())));
    }
}
