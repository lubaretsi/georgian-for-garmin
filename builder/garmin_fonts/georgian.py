"""Draw Georgian into a Garmin font at the size of the font's own letters.

Garmin's nominal "N px" for an Armenian part is about the height of an Armenian capital, not the
font size, so Georgian drawn at N px comes out about a quarter smaller than the letters around
it. The size is measured instead: Noto Sans Georgian is drawn at the font size whose Mkhedruli
x-height comes closest to the stock Armenian lowercase x-height, both measured on the 2-bit
bitmaps, with its descenders shortened to fit the rows below the baseline (`choose_sizing`).
"""

import io
from dataclasses import dataclass
from math import ceil
from statistics import median

from .codec import GEORGIAN, decode, encode

# At most this many pixels of a Georgian glyph may fall outside the template's row, each at the
# faintest 2-bit level; they are dropped.
MAX_CLIPPED_PIXELS = 2
# Letters without ascenders or descenders, and capitals, measured to size Georgian.
ARMENIAN_X = "աոսռւ"
ARMENIAN_CAPITALS = "".join(map(chr, range(0x531, 0x557)))
MKHEDRULI_X = "ათიო"
MTAVRULI = "".join(map(chr, range(0x1C90, 0x1CB1)))
# Font units below the baseline that shortening leaves alone: round letters' overshoot.
OVERSHOOT = 10
# Shortest descender, in percent of the designed length, that `choose_sizing` tries.
MIN_DESCENDER = 50


@dataclass(frozen=True)
class Sizing:
    """How a part's Georgian is drawn: font size in pixels, and descender length in percent."""

    size: int
    descender: int = 100


def rows(tile, height):
    """The 2-bit levels of a (width, bitmap) tile, one list per row."""
    width, bitmap = tile
    stride = (width + 3) // 4
    return [
        [(bitmap[y * stride + x // 4] >> (2 * (x % 4))) & 3 for x in range(width)]
        for y in range(height)
    ]


def letter_height(levels):
    """A glyph's height in pixels: each row counts as its darkest pixel's level, 3 being 1 row.

    Antialiased top and bottom rows count by their coverage, so heights compare to a fraction
    of a pixel.
    """
    return sum(max(row, default=0) for row in levels) / 3


def measure(cmap, height, letters):
    """Median `letter_height` of `letters` in a decoded cmap with rows `height` pixels tall."""
    return median(letter_height(rows(cmap[ord(letter)], height)) for letter in letters)


def shorten(y, descender):
    """Font-unit `y` with the part of an outline below OVERSHOOT `descender`% as long."""
    return y if y >= -OVERSHOOT else -OVERSHOOT + round((y + OVERSHOOT) * descender / 100)


@dataclass(frozen=True)
class SourceGlyph:
    """A Noto Sans Georgian glyph as designed, in Noto's font units."""

    coordinates: tuple
    end_points: tuple
    flags: bytes
    advance: int


class Outlines:
    """Noto Sans Georgian at one weight, drawable at any size with shortened descenders."""

    def __init__(self, font_path, weight):
        from fontTools.misc.roundTools import otRound
        from fontTools.ttLib import TTFont
        from fontTools.varLib import instancer

        with TTFont(font_path) as ttf:
            cmap = ttf.getBestCmap()
            missing = set(GEORGIAN) - cmap.keys()
            axes = {a.axisTag: (a.minValue, a.maxValue) for a in ttf["fvar"].axes} \
                if "fvar" in ttf else {}
            if missing:
                raise ValueError(f"font lacks Georgian code points: {sorted(missing)}")
            low, high = axes.get("wght", (None, None))
            if low is None or not low <= weight <= high:
                raise ValueError(f"font has no weight {weight} on its wght axis")
            # A static instance at `weight`, every other axis at its default.
            location = {tag: None for tag in axes} | {"wght": weight}
            self._ttf = instancer.instantiateVariableFont(ttf, location)
        glyf = self._ttf["glyf"]
        self._names = {cp: cmap[cp] for cp in GEORGIAN}
        names = set(self._names.values())
        if any(glyf[name].isComposite() for name in names):
            raise ValueError("Georgian glyphs built from components are not supported")
        self._coordinates = {name: list(glyf[name].coordinates) for name in names}
        # Instancing can leave fractional coordinates; saving rounds them, as these do.
        self._glyphs = {
            cp: SourceGlyph(
                coordinates=tuple((otRound(x), otRound(y)) for x, y in self._coordinates[name]),
                end_points=tuple(glyf[name].endPtsOfContours),
                flags=bytes(glyf[name].flags),
                advance=otRound(self._ttf["hmtx"][name][0]),
            )
            for cp, name in self._names.items()
        }
        self._fonts = {}
        self.units_per_em = self._ttf["head"].unitsPerEm

    def glyph(self, cp):
        """The SourceGlyph for Georgian code point `cp`, as designed (no shortening)."""
        return self._glyphs[cp]

    def font(self, size, descender=100):
        """A Pillow font at `size` pixels, its outlines below the overshoot `descender`% long."""
        from PIL import ImageFont

        if descender not in self._fonts:
            glyf = self._ttf["glyf"]
            for name, points in self._coordinates.items():
                glyph = glyf[name]
                glyph.coordinates = type(glyph.coordinates)(
                    (x, shorten(y, descender)) for x, y in points
                )
                glyph.recalcBounds(glyf)
            buffer = io.BytesIO()
            self._ttf.save(buffer)
            self._fonts[descender] = buffer.getvalue()
        return ImageFont.truetype(io.BytesIO(self._fonts[descender]), size)


def draw(font, char, height, baseline):
    """Draw `char` on a row `height` pixels tall with its baseline `baseline` pixels down.

    Returns (width, levels inside the row, levels of the inked pixels outside it).
    """
    from PIL import Image, ImageDraw

    left, top, right, bottom = font.getbbox(char, anchor="ls")
    x = max(0, -left)
    width = max(1, ceil(font.getlength(char)) + x, right + x)
    # Rows the outline reaches beyond the template's, drawn so their ink can be checked.
    above, below = max(0, -(baseline + top)), max(0, baseline + bottom - height)
    image = Image.new("L", (width, above + height + below))
    ImageDraw.Draw(image).text((x, above + baseline), char, font=font, anchor="ls", fill=255)
    levels = image.point(lambda pixel: (pixel + 42) // 85).tobytes()
    inside = levels[above * width:(above + height) * width]
    outside = levels[:above * width] + levels[(above + height) * width:]
    return width, inside, [level for level in outside if level]


def fits(lost):
    """Whether a glyph that inks `lost` pixels outside its row may drop them."""
    return len(lost) <= MAX_CLIPPED_PIXELS and all(level == 1 for level in lost)


def _all_fit(font, height, baseline):
    return all(fits(draw(font, chr(cp), height, baseline)[2]) for cp in GEORGIAN)


def _glyph_heights(font, height, baseline, letters):
    return median(
        letter_height([inside[y * width:(y + 1) * width] for y in range(height)])
        for width, inside, _ in (draw(font, letter, height, baseline) for letter in letters)
    )


def choose_sizing(data, outlines, nominal):
    """The Sizing for Georgian in the stock font `data`, whose nominal size is `nominal` px.

    The size is the font size from 1 to twice `nominal` whose Mkhedruli x-height (MKHEDRULI_X)
    is closest to the stock Armenian x-height (ARMENIAN_X); ties go to the Mtavruli height
    closest to the Armenian capitals', then to the smaller size. Georgian descenders are longer
    than Armenian ones, so the outlines below OVERSHOOT are shortened, by whole percent, until
    every glyph fits the row. Raises ValueError when nothing down to MIN_DESCENDER fits.
    """
    header, _, cmap = decode(data)
    height, baseline = header[7:9]
    missing = "".join(c for c in ARMENIAN_X + ARMENIAN_CAPITALS if ord(c) not in cmap)
    if missing:
        raise ValueError(f"stock font lacks the Armenian letters Georgian is sized by: {missing}")
    x_height = measure(cmap, height, ARMENIAN_X)
    capitals = measure(cmap, height, ARMENIAN_CAPITALS)

    def distance(size):
        font = outlines.font(size)
        return (
            abs(_glyph_heights(font, height, baseline, MKHEDRULI_X) - x_height),
            abs(_glyph_heights(font, height, baseline, MTAVRULI) - capitals),
            size,
        )

    size = min(range(1, 2 * nominal + 1), key=distance)
    for descender in range(100, MIN_DESCENDER - 1, -1):
        if _all_fit(outlines.font(size, descender), height, baseline):
            return Sizing(size, descender)
    raise ValueError(f"Georgian does not fit the template at {size} px")


def with_georgian(data, outlines, sizing):
    """Return (`data` with GEORGIAN glyphs drawn from `outlines` at `sizing`, clipped pixels).

    Existing glyphs are kept. Glyphs are drawn on the template's own row height and baseline.
    Ink outside the row is dropped only when it is at most MAX_CLIPPED_PIXELS faintest (level 1)
    pixels; the second value maps each code point that lost pixels to how many. Any other
    overflow raises ValueError.
    """
    header, footer, cmap = decode(data)
    height, baseline = header[7:9]
    font = outlines.font(sizing.size, sizing.descender)
    clipped = {}
    for cp in GEORGIAN:
        width, inside, lost = draw(font, chr(cp), height, baseline)
        if width > 255 or not fits(lost):
            raise ValueError(f"Georgian glyph U+{cp:04X} does not fit the template")
        if lost:
            clipped[cp] = len(lost)
        stride = (width + 3) // 4
        packed = bytearray(stride * height)
        for i, level in enumerate(inside):
            y, col = divmod(i, width)
            packed[y * stride + col // 4] |= level << (2 * (col % 4))
        if not any(packed):
            raise ValueError(f"Georgian glyph U+{cp:04X} is empty")
        cmap[cp] = (width, bytes(packed))
    return encode(header, footer, cmap), clipped
