"""Draw Georgian into a Garmin font at the size of the font's own letters.

Garmin's nominal "N px" for an Armenian part is about the height of an Armenian capital, not the
font size, so Georgian drawn at N px comes out about a quarter smaller than the letters around
it. The size is measured instead: Noto Sans Georgian is drawn at the font size whose Mkhedruli
x-height comes closest to the x-height of the host's own letters (its host family's
`x_height`, the Armenian lowercase in Garmin's Armenian packs), both measured on the 2-bit
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
MKHEDRULI_X = "ათიო"
MTAVRULI = "".join(map(chr, range(0x1C90, 0x1CB1)))
# Font units below the baseline that shortening leaves alone: round letters' overshoot.
OVERSHOOT = 10
# Shortest descender, in percent of the designed length, that `choose_sizing` tries.
MIN_DESCENDER = 50


def rasterizer():
    """The libraries that draw Georgian here: {"pillow", "freetype2", "raqm", "fonttools"}
    versions, raqm None when Pillow lacks it. The same inputs give other bytes in another
    environment: Pillow without raqm (PyPI's wheels) measures a glyph's advance with its basic
    layout engine, which gives 22 of the 66 Georgian glyphs of the Korean 27 px build another
    width, with the same FreeType. The registry's `georgian_font.rasterizer` is the environment
    the pinned builds come from."""
    import fontTools
    import PIL
    from PIL import features

    return {"pillow": PIL.__version__, "freetype2": features.version("freetype2"),
            "raqm": features.version("raqm"), "fonttools": fontTools.version}


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


def choose_sizing(data, outlines, nominal, host):
    """The Sizing for Georgian in the stock font `data`, whose nominal size is `nominal` px,
    sized like the letters of its host family `host` (hosts.Host).

    The size is the font size from 1 to twice `nominal` whose Mkhedruli x-height (MKHEDRULI_X)
    is closest to the median height of the host's `x_height` letters; ties go to the Mtavruli
    height closest to the host's `capitals`, then to the smaller size. Georgian descenders are
    longer than Armenian ones, so the outlines below OVERSHOOT are shortened, by whole percent,
    until every glyph fits the row. Raises ValueError when nothing down to MIN_DESCENDER fits.
    """
    header, _, cmap = decode(data)
    height, baseline = header[7:9]
    missing = "".join(c for c in host.x_height + host.capitals if ord(c) not in cmap)
    if missing:
        raise ValueError(
            f"stock font lacks the {host.script} host's letters Georgian is sized by: {missing}"
        )
    x_height = measure(cmap, height, host.x_height)
    capitals = measure(cmap, height, host.capitals)

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


def bar_thickness(cap_height, room):
    """A test mark bar's thickness, also its gap to the letter: a tenth of `cap_height`, at
    least 1, at most half the `room` it is drawn in. Raises ValueError when `room` < 2."""
    thickness = min(max(1, round(cap_height / 10)), room // 2)
    if thickness < 1:
        raise ValueError(f"a test mark needs 2 free rows, but only {room} are left")
    return thickness


def add_bar(inside, width, height, baseline, sides):
    """The levels `inside` of a glyph drawn by `draw`, with a test mark's solid bar (level 3)
    spanning its inked columns on each of `sides`: "below" in the rows under the baseline and
    the glyph's lowest ink, "above" in the rows over its highest ink, with a gap as thick as
    the bar (`bar_thickness`). Raises ValueError when a bar doesn't fit the row."""
    levels = bytearray(inside)
    inked = [(y, x) for y in range(height) for x in range(width) if levels[y * width + x]]
    top, bottom = inked[0][0], inked[-1][0]
    left, right = min(x for _, x in inked), max(x for _, x in inked)
    cap_height = baseline - top
    for side in sides:
        if side == "below":
            start = max(baseline, bottom + 1)
            thickness = bar_thickness(cap_height, height - start)
            bar = range(start + thickness, start + 2 * thickness)
        else:
            thickness = bar_thickness(cap_height, top)
            bar = range(top - 2 * thickness, top - thickness)
        for y in bar:
            levels[y * width + left:y * width + right + 1] = b"\x03" * (right - left + 1)
    return bytes(levels)


def with_georgian(data, outlines, sizing, mark=None, markers=None):
    """Return (`data` with GEORGIAN glyphs drawn from `outlines` at `sizing`, clipped pixels).

    Existing glyphs are kept; a font that already maps a Georgian code point is rejected.
    Glyphs are drawn on the template's own row height and baseline.
    Ink outside the row is dropped only when it is at most MAX_CLIPPED_PIXELS faintest (level 1)
    pixels; the second value maps each code point that lost pixels to how many. Any other
    overflow raises ValueError. A test `mark` (marks.Mark) adds its bar to the marked capital
    (`add_bar`). `markers` ({glyph: drawn_as}, characters) then give each glyph the tile of
    another, Georgian ones included, as activation markers for test builds.
    """
    streams = {}
    header, footer, cmap = decode(data, streams)
    height, baseline = header[7:9]
    font = outlines.font(sizing.size, sizing.descender)
    clipped = {}
    present = [cp for cp in GEORGIAN if cp in cmap]
    if present:
        raise ValueError(f"font already maps Georgian: {', '.join(f'U+{cp:04X}' for cp in present)}")
    for cp in GEORGIAN:
        width, inside, lost = draw(font, chr(cp), height, baseline)
        if width > 255 or not fits(lost):
            raise ValueError(f"Georgian glyph U+{cp:04X} does not fit the template")
        if lost:
            clipped[cp] = len(lost)
        if not any(inside):
            raise ValueError(f"Georgian glyph U+{cp:04X} is empty")
        if mark is not None and cp == mark.code_point:
            inside = add_bar(inside, width, height, baseline, mark.sides)
        stride = (width + 3) // 4
        packed = bytearray(stride * height)
        for i, level in enumerate(inside):
            y, col = divmod(i, width)
            packed[y * stride + col // 4] |= level << (2 * (col % 4))
        cmap[cp] = (width, bytes(packed))
    for glyph, drawn_as in (markers or {}).items():
        if ord(glyph) not in cmap or ord(drawn_as) not in cmap:
            raise ValueError(f"marker {glyph} -> {drawn_as} needs both glyphs in the font")
        cmap[ord(glyph)] = cmap[ord(drawn_as)]
    return encode(header, footer, cmap, streams), clipped
