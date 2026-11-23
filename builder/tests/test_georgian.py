import pytest
from registry_data import HOSTS

from garmin_fonts import georgian
from garmin_fonts.codec import GEORGIAN, decode, encode
from garmin_fonts.georgian import OVERSHOOT, Outlines, Sizing, choose_sizing, rows, with_georgian


def test_font_without_georgian_is_rejected(inputs, tmp_path):
    from fontTools import subset
    from fontTools.ttLib import TTFont

    font = TTFont(inputs["noto"])
    subsetter = subset.Subsetter()
    subsetter.populate(unicodes=[cp for cp in GEORGIAN if cp != 0x10E0])
    subsetter.subset(font)
    path = tmp_path / "partial.ttf"
    font.save(path)
    with pytest.raises(ValueError, match="lacks Georgian code points: \\[4320\\]"):
        Outlines(path, 400)


def test_weight_off_the_axis_is_rejected(inputs):
    with pytest.raises(ValueError, match="no weight 950 on its wght axis"):
        Outlines(inputs["noto"], 950)


def test_template_too_short_is_rejected(stock, outlines):
    header, footer, _ = decode(stock["006-D9035-03"])
    short = list(header)
    short[7:9] = 4, 3  # row height, baseline
    template = encode(short, footer, {None: (4, bytes(4))})
    with pytest.raises(ValueError, match="does not fit the template"):
        with_georgian(template, outlines(400), Sizing(27))
    with pytest.raises(ValueError, match="lacks the Armenian host's letters Georgian is sized by: աոս"):
        choose_sizing(template, outlines(400), 27, HOSTS["armenian"])


def test_sizing_that_never_fits_is_rejected(stock, outlines, monkeypatch):
    monkeypatch.setattr(georgian, "MAX_CLIPPED_PIXELS", -1)  # nothing fits
    with pytest.raises(ValueError, match="Georgian does not fit the template at 34 px"):
        choose_sizing(stock["006-D9035-03"], outlines(400), 27, HOSTS["armenian"])


def template_like(data, height_change=0):
    """A template with `data`'s metrics, its row `height_change` pixels taller, and no glyphs."""
    header, footer, _ = decode(data)
    header = list(header)
    header[7] += height_change
    return encode(header, footer, {None: (4, bytes(header[7]))})


def test_faint_overflow_is_clipped_and_reported(stock, outlines):
    # 13 px, three rows below the baseline: ლ's tip leaves one level-1 pixel a row lower.
    data, clipped = with_georgian(template_like(stock["006-D9035-05"]), outlines(400), Sizing(13))
    assert clipped == {0x10DA: 1}
    assert any(decode(data)[2][0x10DA][1])


def test_overflow_beyond_the_limit_is_rejected(stock, outlines, monkeypatch):
    # One row fewer below the baseline: დ, ღ and ლ lose full-strength pixels.
    with pytest.raises(ValueError, match="does not fit the template"):
        with_georgian(template_like(stock["006-D9035-05"], -1), outlines(400), Sizing(13))
    monkeypatch.setattr(georgian, "MAX_CLIPPED_PIXELS", 0)
    with pytest.raises(ValueError, match="U\\+10DA does not fit the template"):
        with_georgian(template_like(stock["006-D9035-05"]), outlines(400), Sizing(13))


def depth(tile, height, baseline):
    """Rows of ink below the baseline."""
    inked = [y for y, row in enumerate(rows(tile, height)) if any(row)]
    return inked[-1] + 1 - baseline


def test_shortening_moves_only_descenders(stock, inputs, outlines):
    from fontTools.ttLib import TTFont

    template = template_like(stock["006-D9035-03"], 10)  # room for the full descenders
    header, _, _ = decode(template)
    height, baseline = header[7:9]
    full, short = (
        decode(with_georgian(template, outlines(400), Sizing(34, descender))[0])[2]
        for descender in (100, 70)
    )
    with TTFont(inputs["noto"]) as ttf:
        cmap, glyf = ttf.getBestCmap(), ttf["glyf"]
        descending = {cp for cp in GEORGIAN if glyf[cmap[cp]].yMin < -OVERSHOOT}
    assert descending
    for cp in GEORGIAN:
        if cp in descending:
            assert depth(short[cp], height, baseline) < depth(full[cp], height, baseline)
        else:
            assert short[cp] == full[cp], f"U+{cp:04X}"


def test_descenders_are_shortened_only_as_far_as_needed(stock, outlines):
    data = stock["006-D9035-03"]
    assert choose_sizing(data, outlines(400), 27, HOSTS["armenian"]) == Sizing(34, 85)
    with pytest.raises(ValueError, match="does not fit the template"):
        with_georgian(data, outlines(400), Sizing(34, 86))


def ink(tile):
    """Sum of a tile's 2-bit pixel levels."""
    _, bitmap = tile
    return sum((byte >> shift) & 3 for byte in bitmap for shift in (0, 2, 4, 6))


@pytest.mark.parametrize("part, size", [("006-D9035-17", 15), ("006-D9035-03", 27)])
def test_bold_draws_heavier_georgian_and_keeps_stock(stock, outlines, part, size):
    _, _, stock_cmap = decode(stock[part])
    regular, bold = (
        decode(with_georgian(stock[part], outlines(weight), Sizing(size))[0])[2]
        for weight in (400, 700)
    )
    for cmap in (regular, bold):
        assert all(cmap[cp] == tile for cp, tile in stock_cmap.items())
    heavier = [cp for cp in GEORGIAN if ink(bold[cp]) > ink(regular[cp])]
    assert len(heavier) == len(GEORGIAN), [f"U+{cp:04X}" for cp in GEORGIAN if cp not in heavier]
    assert sum(ink(bold[cp]) for cp in GEORGIAN) > 1.2 * sum(ink(regular[cp]) for cp in GEORGIAN)
