//! Embeds the font asset directory named by `GEORGIAN_FOR_GARMIN_FONTS`, after checking every
//! listed file against the SHA-256 in its `manifest.json`.

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::exit;

use serde::Deserialize;
use sha2::{Digest, Sha256};

mod parts {
    include!("src/parts.rs");
}

const FONTS_ENV: &str = "GEORGIAN_FOR_GARMIN_FONTS";

#[derive(Deserialize)]
struct Manifest {
    fonts: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    part: String,
    size_px: u16,
    stock: String,
    stock_sha256: String,
    georgian: String,
    georgian_sha256: String,
    #[serde(default)]
    default: bool,
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("error: {}", message.as_ref());
    exit(1);
}

fn checked_file(dir: &Path, relative: &str, expected_sha256: &str) -> PathBuf {
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
    path
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

    if manifest.fonts.is_empty() {
        fail(format!("{} lists no fonts", manifest_path.display()));
    }
    if manifest.fonts.iter().filter(|f| f.default).count() > 1 {
        fail(format!(
            "{} marks more than one font as default",
            manifest_path.display()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut code = String::from("&[\n");
    for font in &manifest.fonts {
        if !parts::PARTS.contains(&(font.size_px, font.part.as_str())) {
            fail(format!(
                "unknown font part {} at {} px",
                font.part, font.size_px
            ));
        }
        if !seen.insert(font.part.as_str()) {
            fail(format!("font part {} is listed twice", font.part));
        }
        let stock = checked_file(&dir, &font.stock, &font.stock_sha256);
        let georgian = checked_file(&dir, &font.georgian, &font.georgian_sha256);
        writeln!(
            code,
            "    BundledFont {{ part: {:?}, size_px: {}, default: {}, stock: include_bytes!({:?}), \
             georgian: include_bytes!({:?}) }},",
            font.part,
            font.size_px,
            font.default,
            stock
                .to_str()
                .unwrap_or_else(|| fail("font paths must be valid UTF-8")),
            georgian
                .to_str()
                .unwrap_or_else(|| fail("font paths must be valid UTF-8")),
        )
        .unwrap();
    }
    code.push_str("]\n");
    let out =
        PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("bundled_fonts.rs");
    fs::write(&out, code).unwrap_or_else(|e| fail(format!("cannot write {}: {e}", out.display())));
}
