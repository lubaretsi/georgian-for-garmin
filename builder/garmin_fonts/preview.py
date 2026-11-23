"""Render a part's Armenian and Georgian letters from their 2-bit bitmaps into a PNG, preview a
TrueType build (`truetype_preview`; the font build writes one per TrueType part), draw a part's
test mark picture (`bitmap_mark_picture`, `truetype_mark_picture`) and a contact sheet of a
build's mark pictures (`marks_sheet`).

The README image is `python -m garmin_fonts.preview --out ../docs/georgian-glyphs-27px.png`,
run from `builder/` in `nix develop`; the contact sheet of a build with test marks is
`python -m garmin_fonts.preview --marks <fonts dir> --out marks.png`.
"""

import argparse
import json
import os
import sys
from pathlib import Path

from . import __version__
from . import registry as font_registry
from .hosts import hosts
from .codec import decode
from .georgian import Outlines, choose_sizing, rows, with_georgian
from .marks import TEST_CAPS

PROG = "garmin-fonts-preview"
REGISTRY = font_registry.REGISTRY
ARMENIAN_SAMPLE = "Հայերեն ՀԱՅԵՐԵՆ"
GEORGIAN_SAMPLE = "მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ"
# Host text shown beside Georgian in a TrueType preview, each sample when the font maps all of it.
HOST_SAMPLES = ("Garmin xo", ARMENIAN_SAMPLE, "עברית")
# Pixel sizes of a TrueType preview's lines: about a watch's notification text, and large.
TRUETYPE_SIZES_PX = (20, 56)
# Grey for each 2-bit level, white on black like the watch.
GREYS = (0, 85, 170, 255)
LABEL_GREY = 150
# Pixel size of a TrueType part's mark picture, and the black border around every one.
MARK_PICTURE_PX = 24
MARK_PICTURE_PADDING = 2


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
        if part["part"] == part_number and pack["kind"] == "bitmap"
    )
    stock = font_registry.check_file(Path(stock_dir) / f"{part_number}.fnt", part["sha256"])
    font_registry.check_file(noto, entries["georgian_font"]["sha256"])
    outlines = Outlines(noto, font_registry.weight(pack, part))
    sizing = choose_sizing(stock, outlines, part["size_px"], hosts(entries)[pack["host"]])
    georgian, _ = with_georgian(stock, outlines, sizing)
    size = f"{part_number}, {part['size_px']} px"
    return sheet([
        (f"Garmin's Armenian letters ({size})", line(stock, ARMENIAN_SAMPLE)),
        (f"Georgian added to the same font ({size})", line(georgian, GEORGIAN_SAMPLE)),
    ])


def truetype_preview(font, label, margin=12):
    """A PNG image of a TrueType font `font` (plain bytes, not masked): `label`, then the host
    samples it maps beside GEORGIAN_SAMPLE, on one line at each of TRUETYPE_SIZES_PX."""
    import io

    from fontTools.ttLib import TTFont
    from PIL import Image, ImageDraw, ImageFont, features

    cmap = TTFont(io.BytesIO(font)).getBestCmap()
    samples = [s for s in HOST_SAMPLES if all(ord(c) in cmap for c in s)]
    text = "  ".join([*samples, GEORGIAN_SAMPLE])
    # Left to right, host text first, even when the line starts with Hebrew (needs libraqm).
    direction = "ltr" if features.check("raqm") else None
    label_font = ImageFont.load_default(size=14)
    fonts = [ImageFont.truetype(io.BytesIO(font), size) for size in TRUETYPE_SIZES_PX]
    boxes = [f.getbbox(text, anchor="ls", direction=direction) for f in fonts]
    heights = [bottom - top for _, top, _, bottom in boxes]
    width = max(right for _, _, right, _ in boxes) + 2 * margin
    image = Image.new("L", (max(width, 400), 22 + sum(heights) + margin * (len(fonts) + 2)))
    draw = ImageDraw.Draw(image)
    draw.text((margin, margin), label, font=label_font, fill=LABEL_GREY)
    y = margin + 22
    for f, (_, top, _, _), h in zip(fonts, boxes, heights):
        y += margin
        draw.text((margin, y - top), text, font=f, anchor="ls", fill=GREYS[3], direction=direction)
        y += h
    return image


def bitmap_mark_picture(font, padding=MARK_PICTURE_PADDING):
    """TEST_CAPS drawn from a bitmap font's glyph tiles at their native pixels, white on black."""
    from PIL import Image

    levels = line(font, TEST_CAPS)
    glyphs = Image.new("L", (len(levels[0]), len(levels)))
    glyphs.putdata([GREYS[level] for row in levels for level in row])
    image = Image.new("L", (glyphs.width + 2 * padding, glyphs.height + 2 * padding))
    image.paste(glyphs, (padding, padding))
    return image


def truetype_mark_picture(font, size=MARK_PICTURE_PX, padding=MARK_PICTURE_PADDING):
    """TEST_CAPS drawn from a TrueType font `font` (plain bytes) at `size` px, white on black."""
    import io

    from PIL import Image, ImageDraw, ImageFont

    pillow_font = ImageFont.truetype(io.BytesIO(font), size)
    left, top, right, bottom = pillow_font.getbbox(TEST_CAPS, anchor="ls")
    image = Image.new("L", (right - left + 2 * padding, bottom - top + 2 * padding))
    ImageDraw.Draw(image).text(
        (padding - left, padding - top), TEST_CAPS, font=pillow_font, anchor="ls", fill=GREYS[3]
    )
    return image


def marks_sheet(fonts_dir, scale=3, columns=2, margin=12):
    """A contact sheet of every mark picture in the asset directory `fonts_dir`, each scaled up
    `scale` times (pixelated) under a label: part, kind, variant, style and marked capital.
    Raises ValueError when the build has no test marks."""
    from PIL import Image, ImageDraw, ImageFont

    fonts_dir = Path(fonts_dir)
    manifest = json.loads((fonts_dir / "manifest.json").read_text())
    if not manifest.get("test_marks"):
        raise ValueError(f"{fonts_dir} is a build without test marks")
    label_font = ImageFont.load_default(size=14)
    label_height = 22
    cells = []
    for font in manifest["fonts"]:
        mark = font["mark"]
        kind = "bitmap" if "size_px" in font else "truetype"
        picture = Image.open(fonts_dir / mark["picture"]).convert("L")
        picture = picture.resize((picture.width * scale, picture.height * scale), Image.NEAREST)
        label = (f"{font['part']}  {kind}  {mark['variant']}  {mark['style']}  "
                 f"{mark['code_point']} (letter {mark['position'] + 1})")
        cells.append((label, picture))
    cell_width = max(max(p.width, label_font.getbbox(label)[2]) for label, p in cells) + margin
    cell_height = max(p.height for _, p in cells) + label_height + margin
    grid_rows = -(-len(cells) // columns)
    image = Image.new("L", (columns * cell_width + margin, grid_rows * cell_height + margin))
    draw = ImageDraw.Draw(image)
    for i, (label, picture) in enumerate(cells):
        row, column = divmod(i, columns)
        x, y = margin + column * cell_width, margin + row * cell_height
        draw.text((x, y), label, font=label_font, fill=LABEL_GREY)
        image.paste(picture, (x, y + label_height))
    return image


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
        "glyph bitmaps scaled up, into a PNG; or, with --marks, a contact sheet of a build's test "
        "mark pictures. Defaults come from `nix develop`.",
        epilog="examples:\n  python -m garmin_fonts.preview --out ../docs/georgian-glyphs-27px.png"
        "\n  python -m garmin_fonts.preview --marks ../result --out marks.png",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--version", action="version", version=f"{PROG} {__version__}")
    parser.add_argument("--registry", default=REGISTRY, help="registry.json (default: the repo's)")
    parser.add_argument("--stock-dir", default=os.environ.get("GFG_STOCK_DIR"),
                        help="directory with <part>.fnt (default: $GFG_STOCK_DIR)")
    parser.add_argument("--noto", default=os.environ.get("GFG_NOTO_SANS_GEORGIAN"),
                        help="Noto Sans Georgian TTF (default: $GFG_NOTO_SANS_GEORGIAN)")
    parser.add_argument("--part", default="006-D9035-03", help="part number (default: %(default)s)")
    parser.add_argument(
        "--marks", nargs="?", const=os.environ.get("GEORGIAN_FOR_GARMIN_FONTS", ""), metavar="DIR",
        help="draw the contact sheet of the test mark pictures in the font asset directory DIR, "
        "a build with test marks (default: $GEORGIAN_FOR_GARMIN_FONTS)",
    )
    parser.add_argument("--out", required=True, help="PNG to write")
    parser.add_argument("--json", action="store_true", help="print {\"out\": path} as JSON")
    args = parser.parse_args(argv)
    if args.marks is not None:
        if not args.marks:
            fail(args.json, "pass --marks DIR, or run inside `nix develop`")
        try:
            image = marks_sheet(args.marks)
        except (OSError, ValueError, KeyError) as error:
            fail(args.json, f"{args.marks}: {error}")
    else:
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
