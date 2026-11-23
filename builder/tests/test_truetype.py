import hashlib
import io
import struct
from fractions import Fraction
from statistics import median

import pytest
from registry_data import ENTRIES, HOSTS, PINS, REGISTRY, TRUETYPE_PART_IDS, TRUETYPE_PARTS

from garmin_fonts import truetype
from garmin_fonts.codec import GEORGIAN
from garmin_fonts.georgian import MKHEDRULI_X
from garmin_fonts.registry import weight
from garmin_fonts.truetype import (
    LATIN_X_HEIGHT,
    UNICODE_RANGE_GEORGIAN,
    TRAILER_SIZE,
    UNITS_PER_EM,
    TrueTypeSizing,
    choose_sizing,
    limits,
    mask,
    open_host,
    trailer,
    version_of,
    unmask,
    with_georgian,
)

# Georgian added to Garmin's TrueType fonts (fonts/README.md, TrueType), with Garmin's version
# trailer. The Forerunner 165 (29.52) rendered the Noto Sans Armenian and Hebrew builds without
# the trailer, 0.3.0-beta.1's (see test_truetype_builds_keep_garmins_trailer); Swiss 721 Hebrew
# didn't render on the fenix 6S Pro without it. A change to these bytes needs a watch test
# (AGENTS.md).
TRUETYPE_SHA256 = {
    "006-D9971-09": "372a605beb46cebcd95b43e7a1cffeb478e8737ae2f277491fe48417eabb327a",
    "006-D9971-08": "7235705adc12f0da0c91ed76cd487a662fe2d939cfad8c199d40a9e50ee63c11",
    "006-D1053-98": "c81f5d9922dfd8f7f6db10c2c6ef89e9fec9f943c722a9bf762452cb646a7891",
    "006-D1053-99": "c00423c699333fe081691904aa43f5686404e9c748020751c58a0d744c73f211",
    "006-D0952-08": "8637fc049a2116bfe6f5e2c95c3f325ace2712351c591d1728d8a1b4d39a294b",
    "006-D0952-07": "e6274f30befbf026cc536205acc3071706309e0ae2898288e2c181f7e2d6d0c3",
}
# Tables the builder rewrites; every other table must stay byte for byte.
REWRITTEN = {"glyf", "loca", "hmtx", "hhea", "maxp", "head", "cmap", "post", "OS/2", "hdmx",
             "LTSH", "VDMX"}
# The fields of rewritten tables the build may change, as fonts/README.md lists them (TrueType
# packs, Size: "Tables the build updates"): the head bounding box, hhea and maxp totals, post
# glyph names, OS/2's Georgian range bit (checked alone), and the per-glyph entries of hdmx (and
# its record size), LTSH and VDMX. head's checkSumAdjustment covers the whole file, so any change
# moves it. glyf, loca, hmtx and cmap are compared glyph by glyph and code point by code point,
# and post's names through the glyph order: their encoding may change, as Swiss 721 Hebrew
# spells out "periodcentered" where fontTools writes its standard Macintosh index.
UPDATED_FIELDS = {
    "head": {"xMin", "yMin", "xMax", "yMax", "checkSumAdjustment"},
    "hhea": {"advanceWidthMax", "minLeftSideBearing", "minRightSideBearing", "xMaxExtent",
             "numberOfHMetrics"},
    "maxp": {"numGlyphs", "maxPoints", "maxContours"},
    "post": {"glyphOrder", "extraNames", "mapping"},
    "OS/2": {"ulUnicodeRange1"},
    "hdmx": {"recordSize", "hdmx"},
    "LTSH": {"yPels"},
    "VDMX": {"groups"},
}
# Garmin's Noto Sans Armenian files keep table data in tag order, and its Swiss 721 Hebrew files
# put glyf before loca and VDMX last; fontTools writes its own order (head, hhea, maxp, OS/2,
# hmtx, ...), which the Noto Sans Hebrew files already use. The table directory is kept
# (test_georgian_build_keeps_the_container_layout) and the pinned builds (TRUETYPE_SHA256) have
# the moved data, but fonts/README.md doesn't say the data moves.
REORDERED = {"006-D9971-09", "006-D9971-08", "006-D0952-08", "006-D0952-07"}


def font(data):
    from fontTools.ttLib import TTFont

    return TTFont(io.BytesIO(unmask(data)))


def built(assets, part):
    out, _ = assets
    return (out / "georgian" / f"{part}.bin").read_bytes()


def test_every_truetype_part_is_pinned():
    assert sorted(TRUETYPE_SHA256) == sorted(TRUETYPE_PART_IDS)


@pytest.mark.parametrize("part", TRUETYPE_SHA256)
def test_truetype_builds_match_pinned_bytes(assets, part):
    assert hashlib.sha256(built(assets, part)).hexdigest() == TRUETYPE_SHA256[part]


def earlier(number, build):
    part = next(p for pack in ENTRIES["packs"] for p in pack["parts"] if p["part"] == number)
    return [(e["sha256"], e["bytes"]) for e in part["earlier_georgian"] if e["build"] == build]


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_truetype_builds_keep_garmins_trailer(assets, truetype_stock, part):
    data, stock = built(assets, part), truetype_stock[part]
    assert trailer(data) == trailer(stock) == data[-16:]
    version = next(p for _, p in TRUETYPE_PARTS if p["part"] == part)["version"]
    assert version_of(data) == (version, part)
    body = data[:-16]
    assert len(body) % 4 == 0
    # The font still parses, and before the trailer it is exactly 0.3.0-beta.1's build, which
    # lacked it: only the trailer was added.
    assert font(body)["head"].unitsPerEm > 0
    assert earlier(part, "0.3.0-beta.1") == [(hashlib.sha256(body).hexdigest(), len(body))]


def test_a_bin_without_a_trailer_is_rejected(truetype_stock):
    stock = truetype_stock["006-D9971-09"]
    with pytest.raises(ValueError, match="trailer"):
        trailer(stock[:-16])
    assert version_of(stock) == ("2.00", "006-D9971-09")


@pytest.mark.parametrize("pack, part", TRUETYPE_PARTS, ids=TRUETYPE_PART_IDS)
def test_each_font_is_built_deterministically_at_its_weight(
    assets, truetype_stock, outlines, pack, part
):
    _, manifest = assets
    number = part["part"]
    entry = next(f for f in manifest["fonts"] if f["part"] == number)
    source = outlines(weight(pack, part))
    host = HOSTS[pack["host"]]
    sizing = choose_sizing(truetype_stock[number], source, host)
    assert (str(sizing.scale), sizing.basis, sizing.descender) == (
        entry["georgian_scale"], entry["georgian_scale_basis"], entry["georgian_descender_percent"]
    )
    first = with_georgian(truetype_stock[number], source, sizing)
    assert first == with_georgian(truetype_stock[number], source, sizing) == built(assets, number)
    other = outlines(400 if weight(pack, part) == 700 else 700)
    assert with_georgian(truetype_stock[number], other,
                         choose_sizing(truetype_stock[number], other, host)) \
        != first


def outline(glyf, name):
    """A glyph's outline, bounds and instructions, as the rasterizer sees them."""
    glyph = glyf[name]
    program = glyph.program.getBytecode() if hasattr(glyph, "program") else b""
    if glyph.numberOfContours == 0:
        return ("empty",)
    bounds = (glyph.xMin, glyph.yMin, glyph.xMax, glyph.yMax)
    if glyph.isComposite():
        components = [(c.glyphName, c.x, c.y, c.flags, getattr(c, "transform", None))
                      for c in glyph.components]
        return ("composite", components, bounds, program)
    return (list(glyph.coordinates), list(glyph.endPtsOfContours),
            bytes(f & 0x41 for f in glyph.flags), bounds, program)


def fields(table):
    """A decompiled table's fields, nested records (OS/2's panose) as dicts."""
    return {key: vars(value) if hasattr(value, "__dict__") else value
            for key, value in vars(table).items()}


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_georgian_build_keeps_the_host(each_build, truetype_stock, part):
    host, georgian = font(truetype_stock[part]), font(built(each_build, part))
    order = host.getGlyphOrder()
    added = [f"uni{cp:04X}" for cp in GEORGIAN]
    assert georgian.getGlyphOrder() == order + added

    assert set(georgian.keys()) == set(host.keys())
    for tag in sorted(set(host.keys()) - REWRITTEN - {"GlyphOrder"}):
        assert georgian.reader[tag] == host.reader[tag], f"{tag} changed"
    for tag in sorted(UPDATED_FIELDS.keys() & host.keys()):
        old, new = fields(host[tag]), fields(georgian[tag])
        changed = {key for key in old.keys() | new.keys() if old.get(key) != new.get(key)}
        unlisted = sorted(changed - UPDATED_FIELDS[tag])
        assert not unlisted, f"{tag} fields fonts/README.md doesn't list changed: {unlisted}"
    assert georgian["OS/2"].ulUnicodeRange1 == host["OS/2"].ulUnicodeRange1 | UNICODE_RANGE_GEORGIAN

    for name in order:
        assert outline(georgian["glyf"], name) == outline(host["glyf"], name), f"glyf {name}"
        assert georgian["hmtx"][name] == host["hmtx"][name], f"hmtx {name}"
    if "hdmx" in host:
        for ppem, widths in host["hdmx"].hdmx.items():
            assert all(georgian["hdmx"].hdmx[ppem][name] == widths[name] for name in order), \
                f"hdmx {ppem} ppem"
    if "LTSH" in host:
        assert {name: georgian["LTSH"].yPels[name] for name in order} == host["LTSH"].yPels, \
            "LTSH host entries"
    if "VDMX" in host:
        # Each size's extremes only grow, as far as the Georgian glyphs need.
        upm, glyf = host["head"].unitsPerEm, georgian["glyf"]
        top, bottom = max(glyf[name].yMax for name in added), min(glyf[name].yMin for name in added)
        for old, new in zip(host["VDMX"].groups, georgian["VDMX"].groups, strict=True):
            assert new == {
                ppem: (max(y_max, -(-top * ppem // upm)), min(y_min, bottom * ppem // upm))
                for ppem, (y_max, y_min) in old.items()
            }, "VDMX extremes"

    assert georgian["cmap"].tableVersion == host["cmap"].tableVersion, "cmap version"
    subtables = [(t.platformID, t.platEncID, t.format, t.language) for t in host["cmap"].tables]
    assert [(t.platformID, t.platEncID, t.format, t.language)
            for t in georgian["cmap"].tables] == subtables, "cmap subtables"
    for old in host["cmap"].tables:
        new = georgian["cmap"].getcmap(old.platformID, old.platEncID)
        expected = old.cmap
        if old.isUnicode() and old.format in (4, 12):
            expected = expected | {cp: f"uni{cp:04X}" for cp in GEORGIAN}
        wrong = sorted(cp for cp in expected.keys() | new.cmap.keys()
                       if new.cmap.get(cp) != expected.get(cp))
        assert not wrong, f"cmap ({old.platformID}, {old.platEncID}) maps differently: " + \
            ", ".join(f"U+{cp:04X}" for cp in wrong[:10])


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_updated_totals_are_the_hosts_extended_to_georgian(each_build, truetype_stock, part):
    host, georgian = font(truetype_stock[part]), font(built(each_build, part))
    glyf, hmtx = georgian["glyf"], georgian["hmtx"]
    names = [f"uni{cp:04X}" for cp in GEORGIAN]
    glyphs = [glyf[name] for name in names]
    metrics = [hmtx[name] for name in names]
    head, old_head = georgian["head"], host["head"]
    assert (head.xMin, head.yMin, head.xMax, head.yMax) == (
        min(old_head.xMin, *(g.xMin for g in glyphs)),
        min(old_head.yMin, *(g.yMin for g in glyphs)),
        max(old_head.xMax, *(g.xMax for g in glyphs)),
        max(old_head.yMax, *(g.yMax for g in glyphs)),
    ), "head bounding box"
    hhea, old_hhea = georgian["hhea"], host["hhea"]
    extents = [lsb + g.xMax - g.xMin for g, (_, lsb) in zip(glyphs, metrics)]
    assert hhea.advanceWidthMax == max(old_hhea.advanceWidthMax, *(a for a, _ in metrics))
    assert hhea.minLeftSideBearing == min(old_hhea.minLeftSideBearing, *(l for _, l in metrics))
    assert hhea.minRightSideBearing == min(
        old_hhea.minRightSideBearing, *(a - e for (a, _), e in zip(metrics, extents))
    )
    assert hhea.xMaxExtent == max(old_hhea.xMaxExtent, *extents)
    maxp, old_maxp = georgian["maxp"], host["maxp"]
    assert maxp.numGlyphs == old_maxp.numGlyphs + len(GEORGIAN) == len(georgian.getGlyphOrder())
    assert maxp.maxPoints == max(old_maxp.maxPoints, *(len(g.coordinates) for g in glyphs))
    assert maxp.maxContours == max(old_maxp.maxContours, *(g.numberOfContours for g in glyphs))


def layout(data):
    """A masked `.bin`'s plain font before the trailer, sfnt header (version, table count,
    binary search fields) and table directory [(tag, checksum, offset, length)]."""
    plain = unmask(data[:-TRAILER_SIZE])
    header = struct.unpack_from(">4sHHHH", plain)
    return plain, header, [struct.unpack_from(">4sIII", plain, 12 + 16 * i) for i in range(header[1])]


def checksum(data):
    data += b"\0" * (-len(data) % 4)
    return sum(struct.unpack(f">{len(data) // 4}I", data)) & 0xFFFFFFFF


def check_container(data):
    """Assert the sfnt conventions Garmin's files follow: table data right after the directory,
    each table 4-byte aligned and zero-padded, table and whole-file checksums correct."""
    plain, _, tables = layout(data)
    spans = sorted((offset, offset + length, tag.decode()) for tag, _, offset, length in tables)
    assert spans[0][0] == 12 + 16 * len(tables), "table data start"
    for (start, end, tag), following in zip(spans, [s for s, _, _ in spans[1:]] + [len(plain)]):
        assert start % 4 == 0, f"{tag} alignment"
        assert following - end < 4 and not any(plain[end:following]), f"{tag} padding"
    for tag, sum_, offset, length in tables:
        table = plain[offset:offset + length]
        if tag == b"head":
            table = table[:8] + b"\0\0\0\0" + table[12:]
        assert checksum(table) == sum_, f"{tag.decode()} checksum"
    assert checksum(plain) == 0xB1B0AFBA, "head checkSumAdjustment"


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_georgian_build_keeps_the_container_layout(each_build, truetype_stock, part):
    data, stock = built(each_build, part), truetype_stock[part]
    assert data[-TRAILER_SIZE:] == stock[-TRAILER_SIZE:], "trailer"
    (_, header, tables), (_, stock_header, stock_tables) = layout(data), layout(stock)
    assert header == stock_header, "sfnt header"
    assert [t[0] for t in tables] == [t[0] for t in stock_tables], "table directory order"
    check_container(stock)
    check_container(data)


@pytest.mark.parametrize("part", [
    pytest.param(part, marks=pytest.mark.xfail(
        strict=True, reason="fontTools moves the table data (REORDERED)"))
    if part in REORDERED else part
    for part in TRUETYPE_PART_IDS
])
def test_georgian_build_keeps_the_table_data_order(each_build, truetype_stock, part):
    def data_order(data):
        _, _, tables = layout(data)
        return [tag.decode() for tag, _, _, _ in sorted(tables, key=lambda t: t[2])]

    assert data_order(built(each_build, part)) == data_order(truetype_stock[part]), \
        "table data order"


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_georgian_glyphs_are_mapped_drawn_and_within_the_host_metrics(assets, part):
    georgian = font(built(assets, part))
    cmap, glyf, hmtx = georgian.getBestCmap(), georgian["glyf"], georgian["hmtx"]
    ascent, descent = limits(georgian)
    for cp in GEORGIAN:
        name = cmap[cp]
        glyph = glyf[name]
        assert glyph.numberOfContours > 0 and not glyph.isComposite(), name
        assert glyph.xMax > glyph.xMin and glyph.yMax > glyph.yMin, name
        assert glyph.program.getBytecode() == b"", name
        advance, lsb = hmtx[name]
        assert advance > 0 and lsb == glyph.xMin, name
        assert -descent <= glyph.yMin and glyph.yMax <= ascent, name


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_tables_sized_by_glyph_count_cover_georgian(assets, part):
    georgian = font(built(assets, part))
    upm, hmtx, glyf = georgian["head"].unitsPerEm, georgian["hmtx"], georgian["glyf"]
    added = [f"uni{cp:04X}" for cp in GEORGIAN]
    if "hdmx" in georgian:
        for ppem, widths in georgian["hdmx"].hdmx.items():
            for name in added:
                assert widths[name] == int(Fraction(hmtx[name][0] * ppem, upm) + Fraction(1, 2))
    if "LTSH" in georgian:
        assert all(georgian["LTSH"].yPels[name] == 1 for name in added)
    if "VDMX" in georgian:
        top = max(glyf[name].yMax for name in added)
        bottom = min(glyf[name].yMin for name in added)
        for group in georgian["VDMX"].groups:
            for ppem, (y_max, y_min) in group.items():
                assert y_max * upm >= top * ppem and y_min * upm <= bottom * ppem


def tops(font_, letters):
    cmap, glyf = font_.getBestCmap(), font_["glyf"]
    return median(glyf[cmap[ord(c)]].yMax for c in letters)


@pytest.mark.parametrize("part", TRUETYPE_PART_IDS)
def test_georgian_is_sized_by_latin_x_height_or_the_em(assets, part):
    _, manifest = assets
    entry = next(f for f in manifest["fonts"] if f["part"] == part)
    georgian = font(built(assets, part))
    letters = HOSTS[PINS[part].host].x_height
    has_latin = all(ord(c) in georgian.getBestCmap() for c in letters)
    assert entry["georgian_scale_basis"] == (LATIN_X_HEIGHT if has_latin else UNITS_PER_EM)
    if has_latin:
        assert abs(tops(georgian, MKHEDRULI_X) - tops(georgian, letters)) <= 1
    else:
        assert Fraction(entry["georgian_scale"]) == Fraction(georgian["head"].unitsPerEm, 1000)


def sizing_table(out, manifest):
    """The TrueType table in fonts/README.md, as the build measures it."""
    lines = [
        "| Part           | Font                       | Units/em | Matched to     | Scale  "
        "| Descenders | Host Latin x-height | Mkhedruli x-height | Georgian top, bottom "
        "| Host ascent, descent |",
        "| -------------- | -------------------------- | -------- | -------------- | ------ "
        "| ---------- | ------------------- | ------------------ | -------------------- "
        "| -------------------- |",
    ]
    for entry in (f for f in manifest["fonts"] if "georgian_scale" in f):
        georgian = font((out / entry["georgian"]).read_bytes())
        letters = HOSTS[PINS[entry["part"]].host].x_height
        has_latin = all(ord(c) in georgian.getBestCmap() for c in letters)
        latin = f"{tops(georgian, letters):g}" if has_latin else "none"
        glyphs = [georgian["glyf"][f"uni{cp:04X}"] for cp in GEORGIAN]
        extent = f"{max(g.yMax for g in glyphs)}, {min(g.yMin for g in glyphs)}"
        ascent, descent = limits(georgian)
        basis = {LATIN_X_HEIGHT: "Latin x-height", UNITS_PER_EM: "em"}[entry["georgian_scale_basis"]]
        lines.append(
            f"| `{entry['part']}` | {georgian['name'].getDebugName(4):<26} "
            f"| {georgian['head'].unitsPerEm:>8} | {basis:<14} "
            f"| {float(Fraction(entry['georgian_scale'])):.4f} "
            f"| {entry['georgian_descender_percent']:>9}% | {latin:>19} "
            f"| {tops(georgian, MKHEDRULI_X):>18g} | {extent:>20} | {f'{ascent}, -{descent}':>20} |"
        )
    return "\n".join(lines) + "\n"


def test_readme_lists_the_truetype_sizes(assets):
    out, manifest = assets
    table = sizing_table(out, manifest)
    readme = (REGISTRY.parent / "README.md").read_text()
    assert table in readme, f"update the TrueType table in fonts/README.md to:\n{table}"


def test_mask_is_garmins_xor_and_symmetric(truetype_stock):
    data = truetype_stock[TRUETYPE_PART_IDS[0]]
    assert unmask(data)[:4] == b"\x00\x01\x00\x00"
    assert unmask(data) == bytes(b ^ 0x76 for b in data)
    assert mask(unmask(data)) == data


def test_open_host_rejects_what_is_not_a_masked_font_without_georgian(assets, truetype_stock):
    data = truetype_stock[TRUETYPE_PART_IDS[0]]
    with pytest.raises(ValueError, match="not a masked TrueType font"):
        open_host(unmask(data))
    with pytest.raises(ValueError, match="unreadable TrueType font"):
        open_host(data[:200])
    with pytest.raises(ValueError, match="already maps Georgian: U\\+10D0"):
        open_host(built(assets, TRUETYPE_PART_IDS[0]))


def test_descenders_are_shortened_only_as_far_as_needed(truetype_stock, outlines):
    # Swiss 721 Hebrew: Latin-matched Georgian needs 95% descenders to stay within 483 units.
    data = truetype_stock["006-D0952-08"]
    sizing = choose_sizing(data, outlines(400), HOSTS["swiss-721-hebrew-truetype"])
    assert sizing.descender == 95
    loose = TrueTypeSizing(sizing.scale, sizing.descender + 1, sizing.basis)
    lowest = min(font(with_georgian(data, outlines(400), loose))["glyf"][f"uni{cp:04X}"].yMin
                 for cp in GEORGIAN)
    assert lowest < -limits(open_host(data))[1]


def test_sizing_that_never_fits_is_rejected(truetype_stock, outlines, monkeypatch):
    monkeypatch.setattr(truetype, "MIN_DESCENDER", 96)
    with pytest.raises(ValueError, match="descenders do not fit the host's 483-unit descent"):
        choose_sizing(truetype_stock["006-D0952-08"], outlines(400), HOSTS["swiss-721-hebrew-truetype"])


def test_a_low_ascent_shrinks_the_scale_to_fit(truetype_stock, outlines, monkeypatch):
    monkeypatch.setattr(truetype, "limits", lambda _font: (700, 1000))
    sizing = choose_sizing(truetype_stock["006-D9971-09"], outlines(400), HOSTS["noto-armenian-truetype"])
    assert sizing.scale == Fraction(700, 770) and sizing.descender == 100
