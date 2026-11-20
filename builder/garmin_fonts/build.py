"""Build the font asset directory: Garmin stock fonts, Georgian builds and a manifest."""

import hashlib
import json
from pathlib import Path

from . import registry as font_registry
from .georgian import Outlines, choose_sizing, with_georgian

# The size the installer stages by default: verified on hardware (Venu 2, firmware 19.05).
DEFAULT_PART = "006-D9035-03"


def build(registry, stock_dir, noto, license_file, out):
    """Write the asset directory to `out`, which must not exist yet; return the manifest.

    `registry` is the font registry JSON (its schema beside it); `stock_dir` holds `<part>.fnt`
    for every part it lists; `noto` is the Noto Sans Georgian TTF and `license_file` its OFL
    text, both matching the registry's `georgian_font` hashes. Every file is checked before
    anything is written.
    """
    stock_dir, out = Path(stock_dir), Path(out)
    entries = font_registry.load(registry)
    packs, source = entries["packs"], entries["georgian_font"]
    font_registry.check_file(noto, source["sha256"])
    license_text = font_registry.check_file(license_file, source["license"]["sha256"])
    stock = {
        part["part"]: (stock_dir / f"{part['part']}.fnt").read_bytes()
        for pack in packs
        for part in pack["parts"]
    }
    for pack in packs:
        font_registry.check_pack(pack, stock)
    outlines = {}
    built = []
    for pack in packs:
        for part in pack["parts"]:
            weight = font_registry.weight(pack, part)
            if weight not in outlines:
                outlines[weight] = Outlines(noto, weight)
            data = stock[part["part"]]
            sizing = choose_sizing(data, outlines[weight], part["size_px"])
            built.append((pack["id"], part, sizing, *with_georgian(data, outlines[weight], sizing)))

    out.mkdir(parents=True, exist_ok=False)
    for sub in ("stock", "georgian", "LICENSES"):
        (out / sub).mkdir()
    (out / "LICENSES" / "OFL.txt").write_bytes(license_text)
    fonts = []
    for pack_id, part, sizing, georgian, clipped in built:
        number = part["part"]
        entry = {
            "part": number,
            "pack": pack_id,
            "size_px": part["size_px"],
            "default": number == DEFAULT_PART,
            # The font size Georgian is drawn at, and its descenders' length in percent.
            "georgian_size_px": sizing.size,
            "georgian_descender_percent": sizing.descender,
            # Faint Georgian pixels dropped outside the row: {"U+10DA": 1}.
            "clipped": {f"U+{cp:04X}": pixels for cp, pixels in clipped.items()},
        }
        for kind, data in (("stock", stock[number]), ("georgian", georgian)):
            path = f"{kind}/{number}.fnt"
            (out / path).write_bytes(data)
            entry[kind] = path
            entry[f"{kind}_sha256"] = hashlib.sha256(data).hexdigest()
        fonts.append(entry)
    manifest = {
        "fonts": fonts,
        "packs": [
            {
                "id": pack["id"],
                "name": pack["name"],
                "weight": pack["weight"],
                "parts": [part["part"] for part in pack["parts"]],
                "tested": pack["tested"],
            }
            for pack in packs
        ],
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest
