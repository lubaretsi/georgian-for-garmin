"""Codec for Garmin's 2-bit ExtData fonts (`.fnt`, staged on the watch as `.tmp`).

Layout: a 36-byte little-endian header, a table of sorted Unicode ranges, one 32-bit index
entry per glyph (8-bit width above a 24-bit bitmap offset), the bitmaps, and a 21-byte footer
carrying the part number and release version. Glyphs are fixed-height rows of four 2-bit
pixels per byte, least-significant pixel first. Unknown header and footer fields are preserved.
"""

from math import ceil
from struct import Struct, iter_unpack, pack, unpack_from

HEADER = Struct("<HH4I6HI")
FOOTER_SIZE = 21
OFFSET_MASK = 0xFFFFFF
# Modern Mkhedruli and Mtavruli.
GEORGIAN = (*range(0x10D0, 0x10F1), *range(0x1C90, 0x1CB1))


def identity(footer):
    """Return the (part number, release version) a decoded footer names: ("006-D9035-03", "2.10")."""
    version = unpack_from("<H", footer, 6)[0]
    return footer[8:-1].decode("ascii", "replace"), "%d.%02d" % divmod(version, 100)


def decode(data):
    """Return (header, footer, cmap), with the missing-glyph tile at cmap[None].

    Each cmap value is a (width, bitmap) tile. Raises ValueError on malformed input.
    """
    if len(data) < HEADER.size:
        raise ValueError("truncated font header")
    header = HEADER.unpack_from(data)
    size, flags, end, ranges_start, index_start, pixels_start, _, height, *_ = header
    if (size, flags, ranges_start, header[-1]) != (36, 0x0403, 36, 0x12345678):
        raise ValueError("not a supported Garmin 2-bit font")
    if not (
        ranges_start + 16 <= index_start < pixels_start < end <= len(data) - FOOTER_SIZE
        and height
    ):
        raise ValueError("invalid section bounds")
    if (pixels_start - index_start) % 4:
        raise ValueError("unaligned glyph index")
    stride, span, reserved, count = unpack_from("<4I", data, ranges_start)
    if (stride, span, reserved) != (12, 16 + 12 * count, 0) or ranges_start + span != index_start:
        raise ValueError("invalid cmap header")
    footer = data[end:]
    if len(footer) != FOOTER_SIZE or footer[:6] != bytes.fromhex("e2ad4ef70100") or footer[-1]:
        raise ValueError("unexpected font footer")
    tiles = []
    next_pixel = pixels_start
    for (entry,) in iter_unpack("<I", data[index_start:pixels_start]):
        width, off = entry >> 24, entry & OFFSET_MASK
        pixel = pixels_start + off
        after = pixel + ((width + 3) // 4) * height
        if pixel != next_pixel or after > end:
            raise ValueError("noncontiguous or out-of-bounds bitmap")
        tiles.append((width, data[pixel:after]))
        next_pixel = after
    if not tiles or next_pixel != end:
        raise ValueError("bitmap length does not match the header")
    cmap = {None: tiles[0]}
    next_index, previous = 1, -1
    for lo, hi, first in iter_unpack("<III", data[ranges_start + 16:index_start]):
        if not previous < lo <= hi <= 0x10FFFF or first != next_index or first + hi - lo >= len(tiles):
            raise ValueError("invalid Unicode range")
        for cp in range(lo, hi + 1):
            cmap[cp] = tiles[next_index]
            next_index += 1
        previous = hi
    if next_index != len(tiles):
        raise ValueError("unmapped glyph table entries")
    return header, footer, cmap


def encode(header, footer, cmap):
    """Rebuild the range and index tables; preserve the template's other metadata."""
    keys = sorted(cp for cp in cmap if cp is not None)
    ranges = []
    offsets, pixels = bytearray(), bytearray()
    for i, cp in enumerate([None, *keys]):
        if cp is not None:
            if not 0 <= cp <= 0x10FFFF:
                raise ValueError(f"invalid code point: {cp}")
            if ranges and cp == ranges[-1][1] + 1:
                ranges[-1][1] = cp
            else:
                ranges.append([cp, cp, i])
        width, bitmap = cmap[cp]
        if not 0 <= width <= 255 or len(bitmap) != ((width + 3) // 4) * header[7]:
            raise ValueError(f"invalid glyph dimensions: {cp}")
        if len(pixels) > OFFSET_MASK:
            raise ValueError("bitmap offset exceeds 24 bits")
        offsets += pack("<I", (width << 24) | len(pixels))
        pixels += bitmap
    ranges_size = 16 + 12 * len(ranges)
    ranges_start = HEADER.size
    index_start = ranges_start + ranges_size
    pixels_start = index_start + len(offsets)
    end = pixels_start + len(pixels)
    return b"".join((
        HEADER.pack(*header[:2], end, ranges_start, index_start, pixels_start, *header[6:]),
        pack("<4I", 12, ranges_size, 0, len(ranges)),
        *(pack("<III", *r) for r in ranges),
        offsets,
        pixels,
        footer,
    ))


def with_georgian(data, font_path, pixel_size):
    """Return `data` with GEORGIAN glyphs rendered from `font_path`; existing glyphs are kept.

    Glyphs are drawn with Pillow at `pixel_size` on the template's own row height and
    baseline, using the font's default instance.
    """
    from fontTools.ttLib import TTFont
    from PIL import Image, ImageDraw, ImageFont

    header, footer, cmap = decode(data)
    with TTFont(font_path) as ttf:
        missing = set(GEORGIAN) - ttf.getBestCmap().keys()
    if missing:
        raise ValueError(f"font lacks Georgian code points: {sorted(missing)}")
    font = ImageFont.truetype(font_path, pixel_size)
    height, baseline = header[7:9]
    for cp in GEORGIAN:
        char = chr(cp)
        left, top, right, bottom = font.getbbox(char, anchor="ls")
        x = max(0, -left)
        width = max(1, ceil(font.getlength(char)) + x, right + x)
        if not (0 <= baseline + top < baseline + bottom <= height and width <= 255):
            raise ValueError(f"Georgian glyph U+{cp:04X} does not fit the template")
        image = Image.new("L", (width, height))
        ImageDraw.Draw(image).text((x, baseline), char, font=font, anchor="ls", fill=255)
        stride = (width + 3) // 4
        packed = bytearray(stride * height)
        for i, pixel in enumerate(image.tobytes()):
            y, col = divmod(i, width)
            packed[y * stride + col // 4] |= ((pixel + 42) // 85) << (2 * (col % 4))
        if not any(packed):
            raise ValueError(f"Georgian glyph U+{cp:04X} is empty")
        cmap[cp] = (width, bytes(packed))
    return encode(header, footer, cmap)
