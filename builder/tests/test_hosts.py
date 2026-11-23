"""Host families as registry data, and `add_georgian` over the hosts beyond Armenian."""

import copy
import os
from concurrent.futures import ProcessPoolExecutor

import pytest
from registry_data import ENTRIES, HOSTS, PINS, SCHEMA, STOCK_PART_IDS

from garmin_fonts import hosts
from garmin_fonts.build import add_georgian
from garmin_fonts.codec import GEORGIAN, decode, encode
from garmin_fonts.registry import RegistryError, validate

# The lab's Georgian builds in Garmin's Thai, Korean, Japanese and Chinese fonts (lab record
# 2026-11-23-cjk-bitmap-host), which add_georgian reproduces byte for byte: (markers, SHA-256).
# The Venu 2's Thai and Korean ones carry the activation markers they were tested with; the
# Thai 27 px build (006-D8947-22) drew Georgian on the watch.
LAB_BUILDS = {
    "006-D8947-20": ({"ჰ": "ა", "ก": "ข"}, "e7f4764509ae4eca2c79d5f4ea2293c9f885a6f879fcd21b39c9ae17f94ca403"),
    "006-D8947-21": ({"ჰ": "ბ", "ก": "ข"}, "d0e7fcb5aa24d252f286675b9371d9e5d3fc8fa46cda66b58e93bd58f8d858a3"),
    "006-D8947-22": ({"ჰ": "გ", "ก": "ข"}, "e9536a34b05f2e072563324ade79796f9fb5df1c4da8e55eccaf709c49f6bff7"),
    "006-D8947-23": ({"ჰ": "დ", "ก": "ข"}, "64e205e5da957c3baefde809d6fbefec93a713256a81115ab11a9e0be241c96a"),
    "006-D8945-20": ({"ჰ": "ე", "가": "나"}, "ac2cdf72457cf589014845f420ba50777d363823750af749310aa3afbfc129c6"),
    "006-D8945-21": ({"ჰ": "ვ", "가": "나"}, "a000ac1b9697125b55f232293c9be9b37339ade4a4dd505a8431190d0f11e4f6"),
    "006-D8945-22": ({"ჰ": "ზ", "가": "나"}, "bcc45e572a9b787cf217002bdbd1530bdbace3ecbc45f48cf35cfcefab585eb6"),
    "006-D8945-23": ({"ჰ": "თ", "가": "나"}, "c2f71980b2720ca96f2a53df4b6fc21036084980f4457ed24f9f64e148a73a00"),
    "006-D8945-67": ({}, "61a204a159b951cbb09e8a5a0706fd5d7b05af7135e6204e02da8ba45e94d2c6"),
    "006-D8945-68": ({}, "1f5a1f8e8873c56a1568ced1661b86a78d0dddb0e1fb70ee17ed8cae65d17176"),
    "006-D8944-20": ({}, "5559ddc20fb9700b77b7d79d12464079c742e5aa71eb0e907141ca657bccba71"),
    "006-D8944-21": ({}, "a295f26b2411a9027df0d0b85183f38bfd7adf8c5c1f7674de21a100950c4158"),
    "006-D8944-22": ({}, "20b8bef0f7d8150c6c2a1350100eaee6fe1b4f2684c77e195c9e90567a0adf10"),
    "006-D8944-67": ({}, "6c936fe17a779bd45033d88d418e5e662c40b44ff80b9d2d691411e6d2c4d0ef"),
    "006-D8448-70": ({}, "a1ee6538254d07b3e262a68ad2de3733ed7732fdc43abb8f5c908735f980ab52"),
    "006-D8448-71": ({}, "cce87d7c23872fa04179a6a7f62189884423f4bf6d316aa2e141745be939db84"),
}
# Parts whose rows Georgian doesn't fit at the matched size, as in the lab.
DO_NOT_FIT = {"006-D8944-23": 34, "006-D8944-66": 19}


def build_one(item):
    part, (markers, _) = item
    return part, add_georgian(part, markers=markers, offline=True)


@pytest.fixture(scope="session")
def lab_builds(catalog_stock, inputs):
    """add_georgian of every LAB_BUILDS part, in parallel."""
    with ProcessPoolExecutor(max_workers=os.cpu_count()) as pool:
        return dict(pool.map(build_one, LAB_BUILDS.items()))


def test_every_stock_part_has_a_bitmap_host():
    assert {PINS[p].host for p in STOCK_PART_IDS} == {"thai", "japanese", "korean", "chinese"}
    for host in HOSTS.values():
        if host.kind == "bitmap":
            assert host.capitals and len(host.marker) == 2


@pytest.mark.parametrize("part", LAB_BUILDS)
def test_add_georgian_reproduces_the_lab_builds(lab_builds, catalog_stock, part):
    built = lab_builds[part]
    markers, sha256 = LAB_BUILDS[part]
    assert built.sha256 == sha256
    assert (built.host, built.weight, built.markers) == (PINS[part].host, PINS[part].weight, markers)
    header, footer, cmap = decode(built.data)
    stock_header, stock_footer, stock_cmap = decode(catalog_stock[part])
    assert footer == stock_footer and header[1] == stock_header[1]
    assert cmap.keys() == stock_cmap.keys() | set(GEORGIAN)
    replaced = {ord(glyph) for glyph in markers}
    assert all(cmap[cp] == tile for cp, tile in stock_cmap.items() if cp not in replaced)
    for cp in GEORGIAN:
        if cp not in replaced:
            assert any(cmap[cp][1]) and cmap[cp] != stock_cmap[None], f"U+{cp:04X}"
    for glyph, drawn_as in markers.items():
        assert cmap[ord(glyph)] == cmap[ord(drawn_as)]


@pytest.mark.parametrize("part", DO_NOT_FIT)
def test_georgian_that_does_not_fit_is_refused(catalog_stock, inputs, part):
    with pytest.raises(ValueError, match=f"does not fit the template at {DO_NOT_FIT[part]} px"):
        add_georgian(catalog_stock[part], offline=True)


def test_size_like_another_host_and_explicit_sizing(catalog_stock, inputs):
    data = catalog_stock["006-D8947-22"]
    thai = add_georgian(data, offline=True)
    assert add_georgian(data, "thai", offline=True).data == thai.data
    assert add_georgian(data, hosts.get("thai"), offline=True).data == thai.data
    assert thai.stock_pinned and thai.stock_sha256 == PINS["006-D8947-22"].sha256
    header, footer, cmap = decode(data)
    cmap[ord("ก")] = cmap[ord("ข")]
    changed = add_georgian(encode(header, footer, cmap), offline=True)
    assert not changed.stock_pinned and changed.summary()["stock_pinned"] is False
    assert add_georgian(data, thai.sizing, offline=True).data == thai.data
    with pytest.raises(ValueError, match="host noto-hebrew-truetype is a truetype host"):
        add_georgian(data, "noto-hebrew-truetype", offline=True)
    with pytest.raises(ValueError, match="unknown host 'cyrillic'"):
        add_georgian(data, "cyrillic", offline=True)
    with pytest.raises(ValueError, match="needs both glyphs"):
        add_georgian(data, markers={"ჰ": "A"}, offline=True)
    with pytest.raises(ValueError, match="font already maps Georgian"):
        add_georgian(thai, offline=True)


def test_an_unpinned_parts_weight_comes_from_its_catalog_name(catalog_stock, inputs, tmp_path):
    import dataclasses

    from garmin_fonts.catalog import Stock

    # Garmin's Korean 27 px font under the footer of a part the registry doesn't pin.
    data = catalog_stock["006-D8945-22"][:-13] + b"006-D8945-13\x00"
    bold = Stock("006-D8945-13", "2.10", "bitmap", "APAC Font, Nanum Gothic Bold 27px, KOR",
                 "", False, "catalog", tmp_path / "x.fnt", data)
    assert add_georgian(bold, "korean", offline=True).weight == 700
    with pytest.raises(ValueError, match="weight isn't known"):
        add_georgian(data, "korean", offline=True)
    vague = dataclasses.replace(bold, name="APAC Font, Nanum Gothic 27px")
    with pytest.raises(ValueError, match="catalog name 'APAC Font, Nanum Gothic 27px'"):
        add_georgian(vague, "korean", offline=True)


def test_this_environment_is_the_pinned_rasterizer():
    from garmin_fonts.georgian import rasterizer

    assert rasterizer() == ENTRIES["georgian_font"]["rasterizer"], \
        "the nix environment changed: rebuild, test on a watch, then update the pins together"


def test_a_build_elsewhere_is_marked_and_warned(catalog_stock, inputs, monkeypatch):
    from garmin_fonts import build

    elsewhere = dict(ENTRIES["georgian_font"]["rasterizer"], raqm=None)
    monkeypatch.setattr(build, "rasterizer", lambda: elsewhere)
    with pytest.warns(build.UnpinnedRasterizerWarning, match="'raqm': None"):
        built = add_georgian(catalog_stock["006-D8947-22"], offline=True)
    assert not built.rasterizer_pinned and built.rasterizer == elsewhere
    assert built.summary()["rasterizer_pinned"] is False


def test_a_host_marker_marks_a_truetype_build(truetype_stock, inputs):
    data = truetype_stock["006-D1053-98"]
    marked = add_georgian(data, markers=HOSTS["noto-hebrew-truetype"].markers, noto=inputs["noto"])
    plain = add_georgian(data, noto=inputs["noto"])
    assert marked.data != plain.data and marked.sizing == plain.sizing


def broken(change):
    entries = copy.deepcopy(ENTRIES)
    change(entries)
    return entries


@pytest.mark.parametrize("change, message", [
    (lambda e: e["packs"][0].update(host="cyrillic"), "host cyrillic is not in hosts"),
    (lambda e: e["packs"][0].update(host="noto-hebrew-truetype"), "is a truetype host, not bitmap"),
    (lambda e: e["stock"].append(dict(e["stock"][0])), "part 006-D8448-25 is listed more than once"),
    (lambda e: e["stock"].append(dict(e["stock"][0], part="006-D9035-03")),
     "part 006-D9035-03 is listed more than once"),
    (lambda e: e["hosts"].append(dict(e["hosts"][0])), "host id armenian is listed more than once"),
    (lambda e: e["stock"][0].update(host="noto-armenian-truetype"), "is a truetype host"),
])
def test_registry_rejects_bad_hosts_and_stock(change, message):
    import json

    with pytest.raises(RegistryError, match=message):
        validate(broken(change), json.loads(SCHEMA.read_text()))


def test_bitmap_hosts_need_capitals():
    import json

    entries = broken(lambda e: e["hosts"][1]["size_like"].pop("capitals"))
    with pytest.raises(RegistryError, match="capitals"):
        validate(entries, json.loads(SCHEMA.read_text()))
