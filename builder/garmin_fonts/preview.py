"""Render a part's Armenian and Georgian letters from their 2-bit bitmaps into a PNG.

The README image is `python -m garmin_fonts.preview --out ../docs/georgian-glyphs-27px.png`,
run from `builder/` in `nix develop`.
"""

import argparse
import json
import os
import sys
from pathlib import Path

from . import __version__
from . import registry as font_registry
from .codec import decode
from .georgian import Outlines, choose_sizing, rows, with_georgian

PROG = "garmin-fonts-preview"
REGISTRY = Path(__file__).resolve().parents[2] / "fonts" / "registry.json"
ARMENIAN_SAMPLE = "Հայերեն ՀԱՅԵՐԵՆ"
GEORGIAN_SAMPLE = "მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ"
# Grey for each 2-bit level, white on black like the watch.
GREYS = (0, 85, 170, 255)
LABEL_GREY = 150


def line(font, text):
    """`text` drawn from a Garmin font's glyph bitmaps: a list of rows of 2-bit levels.

    The fonts have no space, so a space is a blank a third of the row height wide; a letter the
    font lacks is its missing-glyph box.
    """
    header, _, cmap = decode(font)
    height = header[7]
    out = [[] for _ in range(height)]
    for char in text:
        if char == " ":
            glyph = [[0] * (height // 3)] * height
        else:
            glyph = rows(cmap.get(ord(char), cmap[None]), height)
        for row, pixels in zip(out, glyph):
            row.extend(pixels)
    return out


def sheet(entries, scale=3, margin=12):
    """A PNG image of (label, line) entries, one below the other, at `scale` pixels per pixel."""
    from PIL import Image, ImageDraw, ImageFont

    label_font = ImageFont.load_default(size=14)
    label_height = 22
    width = max(len(levels[0]) for _, levels in entries) * scale + 2 * margin
    height = sum(label_height + len(levels) * scale + margin for _, levels in entries) + margin
    image = Image.new("L", (width, height))
    draw = ImageDraw.Draw(image)
    y = margin
    for label, levels in entries:
        draw.text((margin, y), label, font=label_font, fill=LABEL_GREY)
        y += label_height
        glyphs = Image.new("L", (len(levels[0]), len(levels)))
        glyphs.putdata([GREYS[level] for row in levels for level in row])
        image.paste(glyphs.resize((glyphs.width * scale, glyphs.height * scale), Image.NEAREST),
                    (margin, y))
        y += len(levels) * scale + margin
    return image


def readme_image(registry, stock_dir, noto, part_number):
    """The README image for `part_number`: its Armenian letters, then the Georgian ones."""
    entries = font_registry.load(registry)
    pack, part = next(
        (pack, part) for pack in entries["packs"] for part in pack["parts"]
        if part["part"] == part_number
    )
    stock = font_registry.check_file(Path(stock_dir) / f"{part_number}.fnt", part["sha256"])
    font_registry.check_file(noto, entries["georgian_font"]["sha256"])
    outlines = Outlines(noto, font_registry.weight(pack, part))
    sizing = choose_sizing(stock, outlines, part["size_px"])
    georgian, _ = with_georgian(stock, outlines, sizing)
    size = f"{part_number}, {part['size_px']} px"
    return sheet([
        (f"Garmin's Armenian letters ({size})", line(stock, ARMENIAN_SAMPLE)),
        (f"Georgian added to the same font ({size})", line(georgian, GEORGIAN_SAMPLE)),
    ])


def fail(as_json, message):
    if as_json:
        print(json.dumps({"error": {"code": "invalid_input", "message": message}}), file=sys.stderr)
    else:
        print(f"{PROG}: error: {message}", file=sys.stderr)
    sys.exit(2)


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    parser = argparse.ArgumentParser(
        prog=PROG,
        description="Draw a part's stock Armenian letters and its Georgian build, from the 2-bit "
        "glyph bitmaps scaled up, into a PNG. Defaults come from `nix develop`.",
        epilog="example:\n  python -m garmin_fonts.preview --out ../docs/georgian-glyphs-27px.png",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--version", action="version", version=f"{PROG} {__version__}")
    parser.add_argument("--registry", default=REGISTRY, help="registry.json (default: the repo's)")
    parser.add_argument("--stock-dir", default=os.environ.get("GFG_STOCK_DIR"),
                        help="directory with <part>.fnt (default: $GFG_STOCK_DIR)")
    parser.add_argument("--noto", default=os.environ.get("GFG_NOTO_SANS_GEORGIAN"),
                        help="Noto Sans Georgian TTF (default: $GFG_NOTO_SANS_GEORGIAN)")
    parser.add_argument("--part", default="006-D9035-03", help="part number (default: %(default)s)")
    parser.add_argument("--out", required=True, help="PNG to write")
    parser.add_argument("--json", action="store_true", help="print {\"out\": path} as JSON")
    args = parser.parse_args(argv)
    if not args.stock_dir or not args.noto:
        fail(args.json, "pass --stock-dir and --noto, or run inside `nix develop`")
    try:
        image = readme_image(args.registry, args.stock_dir, args.noto, args.part)
    except StopIteration:
        fail(args.json, f"part {args.part} is not in the registry")
    except (OSError, ValueError) as error:
        fail(args.json, str(error))
    image.save(args.out, optimize=True)
    print(json.dumps({"out": str(args.out)}) if args.json else args.out)


if __name__ == "__main__":
    main()
