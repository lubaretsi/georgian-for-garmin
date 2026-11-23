"""Beta test marks: each part's Georgian marks one capital of the test text with a bar."""

import hashlib
import io
import json
import subprocess
import sys
from pathlib import Path

import pytest
from registry_data import ENTRIES, HOSTS, PART_IDS, PINS, TRUETYPE_PART_IDS
from test_build import cli_args, run_cli

from garmin_fonts import truetype
from garmin_fonts.codec import compressed, decode
from garmin_fonts.georgian import add_bar, rows
from garmin_fonts.marks import STYLES, TEST_CAPS, Mark, assign
from garmin_fonts.preview import bitmap_mark_picture, marks_sheet, truetype_mark_picture

PNG = b"\x89PNG\r\n\x1a\n"
# The contract's capitals: U+1C9B U+1C97 U+1C90 U+1C95 U+1CA0 U+1CA3 U+1C9A U+1C98.
CONTRACT_CAPS = (0x1C9B, 0x1C97, 0x1C90, 0x1C95, 0x1CA0, 0x1CA3, 0x1C9A, 0x1C98)


def expected_mark(parts, number):
    """The contract's mark for part `number`, the i-th of its kind's `parts`."""
    i = parts.index(number)
    return {
        "variant": chr(ord("A") + i),
        "code_point": f"U+{CONTRACT_CAPS[i % 8]:04X}",
        "position": i % 8,
        "style": ("below", "above", "both")[i // 8],
    }


def test_test_caps_match_contract():
    assert tuple(map(ord, TEST_CAPS)) == CONTRACT_CAPS
    assert STYLES == ("below", "above", "both")


def test_assignment_follows_registry_order_per_kind():
    marks = assign(ENTRIES["packs"])
    assert "".join(marks[p].variant for p in PART_IDS) == "ABCDEFGHIJKLMNOPQRSTUVWX"
    assert "".join(marks[p].variant for p in TRUETYPE_PART_IDS) == "ABCDEF"
    assert {marks[p].style for p in TRUETYPE_PART_IDS} == {"below"}
    for parts in (PART_IDS, TRUETYPE_PART_IDS):
        for number in parts:
            mark = marks[number]
            fields = {"variant": mark.variant, "code_point": f"U+{mark.code_point:04X}",
                      "position": mark.position, "style": mark.style}
            assert fields == expected_mark(parts, number)


def test_assignment_rejects_more_parts_than_variants():
    pack = {"kind": "bitmap", "parts": [{"part": str(i)} for i in range(25)]}
    with pytest.raises(ValueError, match="25 bitmap parts"):
        assign([pack])
    assert len(assign([pack | {"parts": pack["parts"][:24]}])) == 24


def test_stable_build_has_no_marks(assets):
    out, manifest = assets
    assert manifest["test_marks"] is False
    assert not any("mark" in font for font in manifest["fonts"])
    assert not list((out / "previews").glob("mark-*"))


def test_marked_manifest_matches_contract(marked_assets):
    out, manifest = marked_assets
    assert manifest["test_marks"] is True
    assert json.loads((out / "manifest.json").read_text()) == manifest
    for font in manifest["fonts"]:
        number, mark = font["part"], font["mark"]
        parts = PART_IDS if number in PART_IDS else TRUETYPE_PART_IDS
        picture = f"previews/mark-{number}.png"
        assert mark == expected_mark(parts, number) | {
            "picture": picture, "picture_sha256": mark["picture_sha256"]
        }
        data = (out / picture).read_bytes()
        assert data[:8] == PNG
        assert mark["picture_sha256"] == hashlib.sha256(data).hexdigest()


def test_marked_manifest_otherwise_matches_stable(assets, marked_assets):
    (_, stable), (_, marked) = assets, marked_assets
    changed = {"mark", "georgian_sha256"}
    assert [{k: v for k, v in f.items() if k not in changed} for f in marked["fonts"]] \
        == [{k: v for k, v in f.items() if k not in changed} for f in stable["fonts"]]
    # Packs and the support list are the same: pre-release builds install exactly what stable
    # builds do, so a beta never installs a font family no watch has confirmed.
    assert marked["packs"] == stable["packs"]
    assert marked["watches"] == stable["watches"]
    assert all(m["georgian_sha256"] != s["georgian_sha256"]
               for m, s in zip(marked["fonts"], stable["fonts"]))


def test_marked_builds_are_the_released_beta(marked_assets):
    """The marked builds are pinned: 0.3.0-beta.2 installed exactly these (TrueType ones then
    without Garmin's 16-byte trailer, which builds now keep), and the registry's
    `earlier_georgian` lists them so later builds recognize and replace them. The Korean packs
    came later and have no earlier build."""
    out, manifest = marked_assets
    released = {
        part["part"]: {(e["sha256"], e["bytes"]) for e in part.get("earlier_georgian", [])
                       if e["build"] == "0.3.0-beta.2"}
        for pack in ENTRIES["packs"]
        for part in pack["parts"]
    }
    korean = {p["part"] for pack in ENTRIES["packs"] if pack["host"] == "korean"
              for p in pack["parts"]}
    assert {part for part, builds in released.items() if not builds} == korean
    for font in manifest["fonts"]:
        if font["part"] in korean:
            continue
        data = (out / font["georgian"]).read_bytes()
        if font["part"] in TRUETYPE_PART_IDS:
            data = data[:-16]
        released_now = {(hashlib.sha256(data).hexdigest(), len(data))}
        assert released[font["part"]] == released_now, font["part"]


def pictures_equal(path, image):
    from PIL import Image

    with Image.open(path) as saved:
        return saved.mode == image.mode and saved.size == image.size \
            and saved.tobytes() == image.tobytes()


@pytest.mark.parametrize("part", PART_IDS)
def test_bitmap_mark_is_a_bar_in_the_row(assets, marked_assets, part):
    (stable_out, _), (out, manifest) = assets, marked_assets
    mark = next(f["mark"] for f in manifest["fonts"] if f["part"] == part)
    stable_data = (stable_out / "georgian" / f"{part}.fnt").read_bytes()
    data = (out / "georgian" / f"{part}.fnt").read_bytes()
    header, footer, cmap = decode(data)
    stable_header, stable_footer, stable_cmap = decode(stable_data)
    # A Compressed font's glyphs are run-length coded, so the bar can change its length and the
    # offsets after the range table (end, index, pixels); every other field stays.
    kept = range(len(header))
    if compressed(header):
        kept = [i for i in kept if i not in (2, 4, 5)]
    else:
        assert len(data) == len(stable_data)
    assert [header[i] for i in kept] == [stable_header[i] for i in kept]
    assert footer == stable_footer
    cp = int(mark["code_point"][2:], 16)
    assert cmap.keys() == stable_cmap.keys()
    assert [k for k in cmap if cmap[k] != stable_cmap[k]] == [cp]

    height, baseline = header[7:9]
    assert cmap[cp][0] == stable_cmap[cp][0]
    before, after = rows(stable_cmap[cp], height), rows(cmap[cp], height)
    ink = [(y, x) for y, row in enumerate(before) for x, level in enumerate(row) if level]
    top, bottom = ink[0][0], ink[-1][0]
    columns = list(range(min(x for _, x in ink), max(x for _, x in ink) + 1))
    bar_rows = [y for y in range(height) if after[y] != before[y]]
    for y in bar_rows:
        changed = [x for x in range(len(after[y])) if after[y][x] != before[y][x]]
        assert changed == columns, f"row {y}"
        assert {before[y][x] for x in changed} == {0}
        assert {after[y][x] for x in changed} == {3}
    below = [y for y in bar_rows if y > bottom]
    above = [y for y in bar_rows if y < top]
    assert below + above == bar_rows or above + below == bar_rows
    sides = {"below": (True, False), "above": (False, True), "both": (True, True)}[mark["style"]]
    assert (bool(below), bool(above)) == sides
    start = max(baseline, bottom + 1)
    if below:  # a solid bar, as far from the glyph as it is thick, inside the row
        assert below == list(range(below[0], below[-1] + 1))
        assert below[0] - start == len(below) and below[-1] < height
    if above:
        assert above == list(range(above[0], above[-1] + 1))
        assert top - 1 - above[-1] == len(above) and above[0] >= 0

    assert pictures_equal(out / mark["picture"], bitmap_mark_picture(data))


def solid_rows(levels, width):
    """The rows of `levels` that are solid level 3."""
    return [y for y in range(len(levels) // width)
            if levels[y * width:(y + 1) * width] == b"\x03" * width]


def glyph_tile(width, height, top, baseline):
    """Levels of a solid glyph inking rows top..baseline-1 of a `height`-row line."""
    return bytes(3 if top <= i // width < baseline else 0 for i in range(width * height))


def test_bitmap_bar_is_a_tenth_of_the_cap_height_and_shrinks_to_fit():
    # Cap height 20: a 2-row bar 2 rows from the glyph, on both sides.
    glyph = glyph_tile(3, 30, 4, 24)
    assert solid_rows(add_bar(glyph, 3, 30, 24, ("below", "above")), 3) \
        == [0, 1, *range(4, 24), 26, 27]
    # Three rows below and two above: a 1-row bar 1 row away.
    glyph = glyph_tile(3, 25, 2, 22)
    assert solid_rows(add_bar(glyph, 3, 25, 22, ("below", "above")), 3) \
        == [0, *range(2, 22), 23]
    # One row left: no room for a bar and its gap.
    with pytest.raises(ValueError, match="only 1"):
        add_bar(glyph_tile(3, 23, 2, 22), 3, 23, 22, ("below",))
    with pytest.raises(ValueError, match="only 1"):
        add_bar(glyph_tile(3, 25, 1, 22), 3, 25, 22, ("above",))


def ttfont(data):
    from fontTools.ttLib import TTFont

    return TTFont(io.BytesIO(truetype.unmask(data)))


# Tables the bar may change: the glyph itself, its offsets, and the recalculated extremes.
MARK_TABLES = {"glyf", "loca", "head", "maxp", "VDMX"}


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_truetype_mark_is_a_bar_within_the_limits(assets, marked_assets, part):
    (stable_out, _), (out, manifest) = assets, marked_assets
    mark = next(f["mark"] for f in manifest["fonts"] if f["part"] == part)
    data = (out / "georgian" / f"{part}.bin").read_bytes()
    marked, stable = ttfont(data), ttfont((stable_out / "georgian" / f"{part}.bin").read_bytes())
    assert sorted(marked.keys()) == sorted(stable.keys())
    for tag in set(marked.keys()) - MARK_TABLES - {"GlyphOrder"}:
        assert marked.reader[tag] == stable.reader[tag], tag
    glyf, stable_glyf = marked["glyf"], stable["glyf"]
    name = f"uni{mark['code_point'][2:]}"
    assert marked.getGlyphOrder() == stable.getGlyphOrder()
    assert [g for g in marked.getGlyphOrder()
            if glyf[g].compile(glyf) != stable_glyf[g].compile(stable_glyf)] == [name]

    glyph, before = glyf[name], stable_glyf[name]
    n = before.numberOfContours
    count = 2 if mark["style"] == "both" else 1
    assert glyph.numberOfContours == n + count
    points = list(glyph.coordinates)
    assert points[:len(before.coordinates)] == list(before.coordinates)
    assert glyph.endPtsOfContours[:n] == before.endPtsOfContours
    ascent, descent = truetype.limits(marked)
    upm = marked["head"].unitsPerEm
    bars = [points[i - 3:i + 1] for i in glyph.endPtsOfContours[n:]]
    sides = []
    for bar in bars:
        xs, ys = {x for x, _ in bar}, sorted({y for _, y in bar})
        assert xs == {before.xMin, before.xMax} and len(ys) == 2
        thickness = ys[1] - ys[0]
        assert 0 < thickness <= upm * truetype.BAR_EM + 1
        if ys[1] < 0:
            sides.append("below")
            assert min(0, before.yMin) - ys[1] == thickness and ys[0] >= -descent
        else:
            sides.append("above")
            assert ys[0] - before.yMax == thickness and ys[1] <= ascent
    assert sides == {"below": ["below"], "above": ["above"], "both": ["below", "above"]}[
        mark["style"]
    ]
    assert marked["hmtx"][name] == stable["hmtx"][name]

    assert pictures_equal(out / mark["picture"], truetype_mark_picture(truetype.unmask(data)))


def test_truetype_bar_fails_without_room(truetype_stock, outlines):
    data = truetype_stock[TRUETYPE_PART_IDS[0]]
    sizing = truetype.choose_sizing(data, outlines(400), HOSTS[PINS[TRUETYPE_PART_IDS[0]].host])
    font = ttfont(truetype.with_georgian(data, outlines(400), sizing))
    glyph = font["glyf"][f"uni{ord(TEST_CAPS[0]):04X}"]
    upm = font["head"].unitsPerEm
    with pytest.raises(ValueError, match="below"):
        truetype.add_bar(glyph, ("below",), upm, 2 * upm, -min(0, glyph.yMin) + 1)
    with pytest.raises(ValueError, match="above"):
        truetype.add_bar(glyph, ("above",), upm, glyph.yMax + 1, upm)


def test_marked_build_is_deterministic_through_the_cli(inputs, marked_assets, tmp_path):
    out, manifest = marked_assets
    result = run_cli(*cli_args(inputs, out=tmp_path / "fonts"), "--test-marks")
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == manifest
    files = sorted(p.relative_to(out) for p in out.rglob("*") if p.is_file())
    assert files == sorted(p.relative_to(tmp_path / "fonts")
                           for p in (tmp_path / "fonts").rglob("*") if p.is_file())
    for path in files:
        assert (out / path).read_bytes() == (tmp_path / "fonts" / path).read_bytes(), path


def test_marks_sheet_labels_every_picture(assets, marked_assets):
    image = marks_sheet(marked_assets[0])
    assert image.mode == "L" and image.width > 0
    with pytest.raises(ValueError, match="without test marks"):
        marks_sheet(assets[0])


def test_marks_sheet_cli_rejects_a_stable_build(assets, tmp_path):
    builder = Path(__file__).resolve().parents[1]
    result = subprocess.run(
        [sys.executable, "-m", "garmin_fonts.preview", "--marks", str(assets[0]),
         "--out", str(tmp_path / "sheet.png"), "--json"],
        capture_output=True, text=True, cwd=builder,
    )
    assert result.returncode == 2
    assert json.loads(result.stderr)["error"]["code"] == "invalid_input"
    assert not (tmp_path / "sheet.png").exists()


def test_mark_sides():
    assert Mark("A", 0, "both").sides == ("below", "above")
    assert Mark("A", 0, "above").sides == ("above",)
