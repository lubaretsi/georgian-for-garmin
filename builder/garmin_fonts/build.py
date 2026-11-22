"""Build the font asset directory: Garmin stock fonts, Georgian builds and a manifest."""

import hashlib
import io
import json
from pathlib import Path

from . import marks as test_mark
from . import registry as font_registry
from . import truetype
from .georgian import Outlines, choose_sizing, with_georgian
from .preview import bitmap_mark_picture, truetype_mark_picture, truetype_preview

# The size the installer stages by default: verified on hardware (Venu 2, firmware 19.05).
DEFAULT_PART = "006-D9035-03"


def build(registry, stock_dir, noto, license_file, out, test_marks=False):
    """Write the asset directory to `out`, which must not exist yet; return the manifest.

    `registry` is the font registry JSON (its schema beside it); `stock_dir` holds every part it
    lists, as `<part>.fnt` (bitmap packs) or `<part>.bin` (TrueType packs); `noto` is the Noto
    Sans Georgian TTF and `license_file` its OFL text, both matching the registry's
    `georgian_font` hashes. Every file is checked before anything is written. Each TrueType
    build also gets a preview, `previews/<part>.png`.

    With `test_marks` (pre-release builds), each part's Georgian marks one capital of the test
    text with a bar (`marks`), and gets a picture of the marked capitals,
    `previews/mark-<part>.png`, described by its manifest entry's `mark`. Without, the fonts
    are the ones tested on watches.
    """
    stock_dir, out = Path(stock_dir), Path(out)
    entries = font_registry.load(registry)
    packs, source = entries["packs"], entries["georgian_font"]
    marks = test_mark.assign(packs) if test_marks else {}
    font_registry.check_file(noto, source["sha256"])
    license_text = font_registry.check_file(license_file, source["license"]["sha256"])
    stock = {
        part["part"]: (stock_dir / font_registry.file_name(pack, part)).read_bytes()
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
            data, mark = stock[part["part"]], marks.get(part["part"])
            if pack["kind"] == "truetype":
                sizing = truetype.choose_sizing(data, outlines[weight])
                georgian = truetype.with_georgian(data, outlines[weight], sizing, mark)
                built.append((pack, part, georgian, truetype_entry(sizing)))
            else:
                sizing = choose_sizing(data, outlines[weight], part["size_px"])
                georgian, clipped = with_georgian(data, outlines[weight], sizing, mark)
                built.append((pack, part, georgian, bitmap_entry(part, sizing, clipped)))

    out.mkdir(parents=True, exist_ok=False)
    for sub in ("stock", "georgian", "previews", "LICENSES"):
        (out / sub).mkdir()
    (out / "LICENSES" / "OFL.txt").write_bytes(license_text)
    fonts = []
    for pack, part, georgian, details in built:
        number = part["part"]
        entry = {"part": number, "pack": pack["id"], "version": part["version"], **details}
        name = font_registry.file_name(pack, part)
        for kind, data in (("stock", stock[number]), ("georgian", georgian)):
            path = f"{kind}/{name}"
            (out / path).write_bytes(data)
            entry[kind] = path
            entry[f"{kind}_sha256"] = hashlib.sha256(data).hexdigest()
        if pack["kind"] == "truetype":
            entry["preview"] = f"previews/{number}.png"
            label = f"{number} {pack['name']}, weight {font_registry.weight(pack, part)}, with Georgian"
            image = truetype_preview(truetype.unmask(georgian), label)
            image.save(out / entry["preview"])
        if number in marks:
            if pack["kind"] == "truetype":
                picture = truetype_mark_picture(truetype.unmask(georgian))
            else:
                picture = bitmap_mark_picture(georgian)
            buffer = io.BytesIO()
            picture.save(buffer, format="PNG")
            path = f"previews/mark-{number}.png"
            (out / path).write_bytes(buffer.getvalue())
            entry["mark"] = test_mark.manifest_entry(
                marks[number], path, hashlib.sha256(buffer.getvalue()).hexdigest()
            )
        fonts.append(entry)
    manifest = {
        "test_marks": bool(test_marks),
        "fonts": fonts,
        "packs": [
            {
                "id": pack["id"],
                "name": pack["name"],
                "kind": pack["kind"],
                "weight": pack["weight"],
                "parts": [part["part"] for part in pack["parts"]],
                "tested": pack["tested"],
            }
            for pack in packs
        ],
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def bitmap_entry(part, sizing, clipped):
    """Manifest fields of a bitmap build."""
    return {
        "size_px": part["size_px"],
        "default": part["part"] == DEFAULT_PART,
        # The font size Georgian is drawn at, and its descenders' length in percent.
        "georgian_size_px": sizing.size,
        "georgian_descender_percent": sizing.descender,
        # Faint Georgian pixels dropped outside the row: {"U+10DA": 1}.
        "clipped": {f"U+{cp:04X}": pixels for cp, pixels in clipped.items()},
    }


def truetype_entry(sizing):
    """Manifest fields of a TrueType build."""
    return {
        "default": False,
        # Host font units per Noto unit, as an exact fraction ("79/39"), what it was matched to,
        # and the descenders' length in percent.
        "georgian_scale": str(sizing.scale),
        "georgian_scale_basis": sizing.basis,
        "georgian_descender_percent": sizing.descender,
    }
