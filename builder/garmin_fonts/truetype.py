"""Add Georgian to Garmin's TrueType ExtData fonts (`.bin`, kept on the watch in `ExtData`).

A Garmin `.bin` font is a plain TrueType file with every byte XOR-ed with 0x76 (`unmask`). The
host keeps its own glyphs, tables, names and vertical metrics; Noto Sans Georgian's Mkhedruli
and Mtavruli outlines are appended at the end of the glyph order, scaled to the host's units,
unhinted, and mapped in every Unicode cmap subtable that can hold them (`with_georgian`). The
size comes from `choose_sizing`: Mkhedruli x-height matches the host's Latin x-height when the
host has Latin letters, otherwise Noto's own proportions; descenders are shortened as in the
bitmap builds when they would reach below the host's descent.

The output is deterministic: no timestamps are touched, every coordinate is rounded from an
exact fraction, and table data fontTools doesn't need to change is copied as it was.
"""

import io
import logging
import struct
from dataclasses import dataclass
from fractions import Fraction
from math import ceil, floor

from .codec import GEORGIAN
from .georgian import MIN_DESCENDER, MKHEDRULI_X, shorten

MASK = 0x76
_MASK_TABLE = bytes(b ^ MASK for b in range(256))
SFNT_TRUETYPE = b"\x00\x01\x00\x00"
# Round-topped Latin lowercase, measured like the round-topped Mkhedruli in MKHEDRULI_X: both
# include their overshoot, so the comparison is like for like.
HOST_LATIN_X = "oces"
# OS/2 ulUnicodeRange1 bit 26: Georgian.
UNICODE_RANGE_GEORGIAN = 1 << 26
# What the scale is matched to.
LATIN_X_HEIGHT = "latin_x_height"
UNITS_PER_EM = "units_per_em"


def unmask(data):
    """A Garmin `.bin` font as plain TrueType, or TrueType as a `.bin` (the mask is symmetric)."""
    return bytes(data).translate(_MASK_TABLE)


mask = unmask


@dataclass(frozen=True)
class TrueTypeSizing:
    """How Georgian is scaled into a host: host units per Noto unit, descender length in
    percent, and what the scale was matched to (LATIN_X_HEIGHT or UNITS_PER_EM)."""

    scale: Fraction
    descender: int
    basis: str


def open_host(data):
    """The fontTools TTFont of a masked `.bin` font; raises ValueError unless it is a glyf-based
    TrueType font without Georgian."""
    from fontTools.ttLib import TTFont, TTLibError

    plain = unmask(data)
    if plain[:4] != SFNT_TRUETYPE:
        raise ValueError("not a masked TrueType font: unexpected header after unmasking")
    try:
        font = TTFont(io.BytesIO(plain), recalcTimestamp=False)
        cmap = font.getBestCmap()
        has_glyf = "glyf" in font
    # fontTools reports malformed input with several exception types.
    except (TTLibError, KeyError, IndexError, AssertionError, struct.error) as error:
        raise ValueError(f"unreadable TrueType font: {error}") from None
    if not has_glyf or cmap is None:
        raise ValueError("TrueType font without glyf outlines or a Unicode cmap")
    present = sorted(cp for cp in GEORGIAN if cp in cmap)
    if present:
        raise ValueError(f"font already maps Georgian: {', '.join(f'U+{cp:04X}' for cp in present)}")
    return font


def _median(values):
    values = sorted(Fraction(v) for v in values)
    middle = len(values) // 2
    return values[middle] if len(values) % 2 else (values[middle - 1] + values[middle]) / 2


def limits(font):
    """(ascent, descent) Georgian must stay within, in font units, descent positive: the
    tighter of `hhea` and the OS/2 Windows metrics, which renderers clip to. OS/2 typo metrics
    set line spacing, and the hosts' own glyphs reach beyond them, so they don't limit."""
    hhea, os2 = font["hhea"], font["OS/2"]
    return min(hhea.ascent, os2.usWinAscent), min(-hhea.descent, os2.usWinDescent)


def host_x_height(font):
    """Median top of HOST_LATIN_X in font units, or None when the host lacks one of them."""
    cmap, glyf = font.getBestCmap(), font["glyf"]
    if any(ord(c) not in cmap for c in HOST_LATIN_X):
        return None
    return _median(glyf[cmap[ord(c)]].yMax for c in HOST_LATIN_X)


def georgian_x_height(outlines):
    """Median top of MKHEDRULI_X in Noto units."""
    return _median(max(y for _, y in outlines.glyph(ord(c)).coordinates) for c in MKHEDRULI_X)


def choose_sizing(data, outlines):
    """The TrueTypeSizing for Georgian in the masked host font `data`.

    The scale makes Mkhedruli x-height (MKHEDRULI_X) equal the host's Latin x-height
    (HOST_LATIN_X) when the host has those letters; otherwise it keeps Noto's proportions to
    the em (host units per em over Noto's). When the tallest Georgian glyph would then rise
    above the host's ascent, the scale shrinks to fit it. Descenders below the overshoot are
    shortened, by whole percent, until the lowest point stays within the host's descent.
    Raises ValueError when nothing down to MIN_DESCENDER fits.
    """
    font = open_host(data)
    x_height = host_x_height(font)
    if x_height is None:
        scale, basis = Fraction(font["head"].unitsPerEm, outlines.units_per_em), UNITS_PER_EM
    else:
        scale, basis = x_height / georgian_x_height(outlines), LATIN_X_HEIGHT
    ascent, descent = limits(font)
    points = [p for cp in GEORGIAN for p in outlines.glyph(cp).coordinates]
    top = max(y for _, y in points)
    if round(top * scale) > ascent:
        scale = Fraction(ascent, top)
    for descender in range(100, MIN_DESCENDER - 1, -1):
        bottom = min(shorten(y, descender) for _, y in points)
        if round(bottom * scale) >= -descent:
            return TrueTypeSizing(scale, descender, basis)
    raise ValueError(f"Georgian descenders do not fit the host's {descent}-unit descent")


def _glyph(source, sizing):
    """A fontTools glyph: `source` scaled by `sizing`, descenders shortened, no instructions."""
    from fontTools.ttLib.tables import ttProgram
    from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphCoordinates

    glyph = Glyph()
    glyph.numberOfContours = len(source.end_points)
    glyph.endPtsOfContours = list(source.end_points)
    glyph.flags = bytearray(source.flags)
    glyph.coordinates = GlyphCoordinates(
        (round(x * sizing.scale), round(shorten(y, sizing.descender) * sizing.scale))
        for x, y in source.coordinates
    )
    glyph.program = ttProgram.Program()
    glyph.program.fromBytecode(b"")
    return glyph


def _round_half_up(value):
    return floor(value + Fraction(1, 2))


def with_georgian(data, outlines, sizing):
    """The masked host font `data` with GEORGIAN added from `outlines` at `sizing`, masked.

    New glyphs are named `uniXXXX` and appended after the host's; every existing glyph keeps
    its index, outline, instructions and metrics. They are mapped in each Unicode cmap subtable
    of format 4 or 12 (a legacy Mac Roman subtable can't hold them). Tables sized by glyph count
    get entries for them: `hdmx` (linear widths) and `LTSH` (linear at every size); `VDMX`
    extremes grow to cover them. OS/2 gains the Georgian Unicode range bit.
    """
    font = open_host(data)
    upm = font["head"].unitsPerEm
    glyf, hmtx = font["glyf"], font["hmtx"]
    # Load every table that lists glyphs while the glyph count still matches; the rest (GSUB,
    # GPOS, GDEF, hinting programs, names) are copied byte for byte.
    cmap, _post = font["cmap"], font["post"]
    hdmx, ltsh, vdmx = (font[tag] if tag in font else None for tag in ("hdmx", "LTSH", "VDMX"))
    # A copy: glyf shares the font's list and appends to it.
    order = list(font.getGlyphOrder())
    taken = set(order)
    if hdmx is not None:
        # fontTools' per-size lists are read-only views indexed by the original glyph order.
        hdmx.hdmx = {ppem: {name: widths[name] for name in order} for ppem, widths in hdmx.hdmx.items()}
    added = {}
    for cp in GEORGIAN:
        name = f"uni{cp:04X}"
        if name in taken:
            raise ValueError(f"host already has a glyph named {name}")
        source = outlines.glyph(cp)
        glyph = _glyph(source, sizing)
        glyf[name] = glyph
        glyph.recalcBounds(glyf)
        if glyph.numberOfContours <= 0 or glyph.xMax <= glyph.xMin or glyph.yMax <= glyph.yMin:
            raise ValueError(f"Georgian glyph U+{cp:04X} is empty")
        hmtx[name] = (round(source.advance * sizing.scale), glyph.xMin)
        added[cp] = name
    font.setGlyphOrder(order + list(added.values()))

    mapped = False
    for table in cmap.tables:
        if table.isUnicode() and table.format in (4, 12):
            table.cmap.update(added)
            mapped = True
    if not mapped:
        raise ValueError("host has no Unicode cmap subtable of format 4 or 12")
    font["OS/2"].ulUnicodeRange1 |= UNICODE_RANGE_GEORGIAN

    if hdmx is not None:
        for ppem, widths in hdmx.hdmx.items():
            for name in added.values():
                widths[name] = _round_half_up(Fraction(hmtx[name][0] * ppem, upm))
    if ltsh is not None:
        for name in added.values():
            ltsh.yPels[name] = 1
    if vdmx is not None:
        top = max(glyf[name].yMax for name in added.values())
        bottom = min(glyf[name].yMin for name in added.values())
        for group in vdmx.groups:
            for ppem, (y_max, y_min) in group.items():
                group[ppem] = (
                    max(y_max, ceil(Fraction(top * ppem, upm))),
                    min(y_min, floor(Fraction(bottom * ppem, upm))),
                )

    # fontTools warns about fsSelection bits the Bitstream hosts set in an OS/2 version 1
    # table; they are the host's own and are kept.
    os2_log = logging.getLogger("fontTools.ttLib.tables.O_S_2f_2")
    level = os2_log.level
    os2_log.setLevel(logging.ERROR)
    try:
        buffer = io.BytesIO()
        font.save(buffer)
    finally:
        os2_log.setLevel(level)
    return mask(buffer.getvalue())
