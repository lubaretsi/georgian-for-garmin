"""Build the font asset directory: Garmin stock fonts, Georgian builds and a manifest."""

import hashlib
import json
from pathlib import Path

from .codec import with_georgian

# Garmin's Armenian Noto Sans Regular 2.10 packs: (part number, nominal pixel size).
PARTS = (
    ("006-D9035-01", 21),
    ("006-D9035-02", 24),
    ("006-D9035-03", 27),
    ("006-D9035-04", 31),
)
# The size verified on hardware (Venu 2, firmware 19.05).
DEFAULT_PART = "006-D9035-03"


def build(stock_dir, noto, license_file, out):
    """Write the asset directory to `out`, which must not exist yet; return the manifest.

    `stock_dir` holds `<part>.fnt` for every part in PARTS; `noto` is the Noto Sans
    Georgian TTF and `license_file` its OFL text.
    """
    stock_dir, out = Path(stock_dir), Path(out)
    license_text = Path(license_file).read_bytes()
    built = []
    for part, size_px in PARTS:
        stock = (stock_dir / f"{part}.fnt").read_bytes()
        built.append((part, size_px, stock, with_georgian(stock, noto, size_px)))

    out.mkdir(parents=True, exist_ok=False)
    for sub in ("stock", "georgian", "LICENSES"):
        (out / sub).mkdir()
    (out / "LICENSES" / "OFL.txt").write_bytes(license_text)
    fonts = []
    for part, size_px, stock, georgian in built:
        entry = {"part": part, "size_px": size_px, "default": part == DEFAULT_PART}
        for kind, data in (("stock", stock), ("georgian", georgian)):
            path = f"{kind}/{part}.fnt"
            (out / path).write_bytes(data)
            entry[kind] = path
            entry[f"{kind}_sha256"] = hashlib.sha256(data).hexdigest()
        fonts.append(entry)
    manifest = {"fonts": fonts}
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest
