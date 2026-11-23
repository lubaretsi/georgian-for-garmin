"""Rendering with fallback models: the three-state trace, and the render report over every watch
report fixture, with Garmin's fonts only and with ours installed.

The report fails only on what is knowable: a confirmed step that doesn't hold (a font named
that lacks the glyph, evidence that doesn't exist), or one of our installed fonts lacking a
Georgian letter. `coverage_only` and `unknown` are reported (printed, and in report.json beside
the PNGs in $GFG_RENDER_OUT), never treated as passing evidence.
"""

import copy
import json
import os
from collections import Counter
from pathlib import Path

import pytest
from registry_data import PINS

from garmin_fonts.build import identify
from garmin_fonts.codec import GEORGIAN
from garmin_fonts.render import (
    MODELS,
    REPORTS,
    STATES,
    TEST_TEXT,
    ModelError,
    find_model,
    installed_builds,
    load_models,
    render,
    render_report,
)

REPO = MODELS.parents[1]
FIXTURES = sorted(REPORTS.glob("*.json"))
VENU_2 = json.loads((REPORTS / "maintainer-venu2-stock-imported.json").read_text())
GEORGIAN_TEXT = [c for c in TEST_TEXT if ord(c) in GEORGIAN]


@pytest.fixture(scope="session")
def ours(assets):
    """Our Georgian builds, as the asset directory holds them."""
    out, manifest = assets
    return [(out / font["georgian"]).read_bytes() for font in manifest["fonts"]]


@pytest.fixture(scope="session")
def thai_build(catalog_stock, inputs):
    from garmin_fonts.build import add_georgian

    return add_georgian(catalog_stock["006-D8947-22"], offline=True)


def test_models_match_their_schema_and_cite_existing_sources():
    models = load_models()
    for evidence in models["evidence"]:
        for source in evidence["sources"]:
            if "repo" in source:
                assert (REPO / source["repo"]).exists(), source
    for model in models["models"]:
        for step in model["order"]:
            assert step["part"] in PINS, f"{model['id']}: {step['part']} is not pinned"


def test_models_reject_unknown_evidence(tmp_path):
    data = json.loads(MODELS.read_text())
    data["models"][0]["order"][0]["evidence"] = ["made-up"]
    (tmp_path / MODELS.name).write_text(json.dumps(data))
    (tmp_path / "fallback-models.schema.json").write_text(
        MODELS.with_name("fallback-models.schema.json").read_text()
    )
    with pytest.raises(ModelError, match="cites unknown evidence"):
        load_models(tmp_path / MODELS.name)


def by_char(rendering):
    return {item["char"]: item for item in rendering.trace}


def test_the_venu_2_stock_draws_boxes_confirmed(catalog_stock, stock):
    rendering = render(VENU_2["declared"], family="006-B3703-00", software="19.05")
    for char in GEORGIAN_TEXT:
        item = by_char(rendering)[char]
        assert (item["state"], item["font"]) == ("confirmed", "missing")
        assert "venu-2-stock-boxes" in item["evidence"]
    # Latin may come from the built-in font, whose bytes nobody has.
    assert by_char(rendering)["T"]["state"] == "unknown"


def test_our_armenian_font_draws_georgian_on_the_venu_2(catalog_stock, ours):
    rendering = render(VENU_2["declared"], replace=installed_builds(VENU_2["declared"], ours),
                       family="006-B3703-00", software="19.05")
    for char in GEORGIAN_TEXT:
        item = by_char(rendering)[char]
        assert (item["state"], item["font"]) == ("confirmed", "006-D9035-03"), item
        assert {"venu-2-armenian-27px", "venu-2-thai-before-armenian"} <= set(item["evidence"])


def test_a_thai_build_comes_before_the_armenian_one(catalog_stock, ours, thai_build):
    replace = installed_builds(VENU_2["declared"], ours) + [thai_build]
    rendering = render(VENU_2["declared"], replace=replace, family="006-B3703-00")
    item = by_char(rendering)["მ"]
    assert (item["state"], item["font"]) == ("confirmed", "006-D8947-22")
    assert {"006-D8947-22", "006-D9035-03"} <= set(item["candidates"])
    # The Thai host alone, Armenian fonts Garmin's (the lab's test (a)).
    alone = by_char(render(VENU_2["declared"], replace=[thai_build], family="006-B3703-00"))["მ"]
    assert (alone["state"], alone["font"]) == ("confirmed", "006-D8947-22")
    assert "venu-2-thai-host" in alone["evidence"]


def test_a_font_the_model_does_not_place_gives_coverage_only(catalog_stock, ours):
    from garmin_fonts.build import add_georgian

    japanese = add_georgian(catalog_stock["006-D8944-22"], offline=True)
    replace = installed_builds(VENU_2["declared"], ours) + [japanese]
    item = by_char(render(VENU_2["declared"], replace=replace, family="006-B3703-00"))["მ"]
    assert item["state"] == "coverage_only" and "006-D8944-22" in item["reason"]


def test_unavailable_bytes_and_missing_models_are_unknown(catalog_stock, ours):
    declared = [*VENU_2["declared"], {"part": "006-D8999-22", "version": "2.00"}]
    replace = installed_builds(VENU_2["declared"], ours)
    item = by_char(render(declared, replace=replace, family="006-B3703-00"))["მ"]
    assert item["state"] == "unknown" and "006-D8999-22" in item["unknown_fonts"]
    other = copy.deepcopy(VENU_2["declared"])
    next(e for e in other if e["part"] == "006-D9035-03")["version"] = "2.20"
    item = by_char(render(other, family="006-B3703-00"))["მ"]
    assert item["state"] == "unknown" and "2.20" in item["reason"]
    assert find_model("006-B3703-00", software="20.00") is None
    plus = render(VENU_2["declared"], replace=replace, family="006-B3851-00")
    assert {i["state"] for i in plus.trace} == {"unknown"}
    assert sorted(by_char(plus)["მ"]["candidates"]) == [f"006-D9035-0{i}" for i in range(1, 5)]


def georgian_holes(rendering, replaced):
    """Georgian characters none of our installed fonts has: a missing glyph in our builds."""
    return [item["char"] for item in rendering.trace
            if ord(item["char"]) in GEORGIAN and not set(item["candidates"]) & replaced]


def test_render_report_over_every_fixture(catalog_stock, ours):
    out = Path(os.environ["GFG_RENDER_OUT"]) if os.environ.get("GFG_RENDER_OUT") else None
    report, totals = [], Counter()
    for fixture in FIXTURES:
        for builds in ((), ours):
            rendering = render_report(fixture, builds=builds)
            installed = installed_builds(json.loads(fixture.read_text())["declared"], builds)
            replaced = {identify(data)[1] for data in installed}
            for item in rendering.trace:
                if item["state"] == "confirmed" and item["font"] != "missing":
                    assert item["font"] in item["candidates"], (fixture.name, item)
                if item["state"] == "confirmed":
                    assert item["evidence"], (fixture.name, item)
            if installed:
                assert not georgian_holes(rendering, replaced), fixture.name
            counts = rendering.counts()
            totals.update(counts)
            name = f"{fixture.stem}{'-ours' if builds else ''}"
            report.append({"fixture": fixture.name, "ours_installed": bool(installed),
                           "model": rendering.model, "counts": counts})
            if out is not None:
                rendering.save(out / f"{name}.png")
    summary = {"text": TEST_TEXT, "totals": {s: totals[s] for s in STATES}, "renders": report}
    if out is not None:
        (out / "report.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(summary["totals"]))
    assert len(report) == 2 * len(FIXTURES)


def test_a_png_is_drawn(catalog_stock, ours):
    rendering = render_report(REPORTS / "maintainer-venu2-stock-imported.json", builds=ours)
    assert rendering.png()[:8] == b"\x89PNG\r\n\x1a\n"
    assert rendering.image.width > 100


def our_build(ours, part):
    return next(data for data in ours if identify(data)[1] == part)


@pytest.fixture
def fetched(monkeypatch):
    """The parts render fetches from the catalog, recorded."""
    from garmin_fonts import render as render_module

    parts, fetch = [], render_module.catalog.fetch_stock

    def recording(part, **kwargs):
        parts.append(part)
        return fetch(part, **kwargs)

    monkeypatch.setattr(render_module.catalog, "fetch_stock", recording)
    return parts


def with_font(declared, font):
    """The declared list with `font` given in place of its part's entry."""
    part = identify(font if isinstance(font, bytes) else Path(font).read_bytes())[1]
    return [font if entry["part"] == part else entry for entry in declared]


@pytest.mark.parametrize("given", ["bytes", "file"])
def test_fonts_given_as_bytes_render_from_those_bytes(catalog_stock, ours, fetched, tmp_path,
                                                      given):
    data = our_build(ours, "006-D9035-03")
    font = data
    if given == "file":
        font = tmp_path / "ours.fnt"
        font.write_bytes(data)
    rendering = render(with_font(VENU_2["declared"], font), family="006-B3703-00",
                       software="19.05", offline=True)
    assert "006-D9035-03" not in fetched and "006-D8947-22" in fetched
    for char in GEORGIAN_TEXT:
        item = by_char(rendering)[char]
        assert (item["state"], item["font"]) == ("confirmed", "006-D9035-03"), item


def test_given_bytes_count_as_garmins_only_when_pinned(catalog_stock, fetched):
    stock_thai = catalog_stock["006-D8947-22"]
    rendering = render(with_font(VENU_2["declared"], stock_thai), family="006-B3703-00")
    assert (by_char(rendering)["მ"]["state"], by_char(rendering)["მ"]["font"]) == \
        ("confirmed", "missing")
    assert "006-D8947-22" not in fetched


def test_replace_must_name_a_declared_part(catalog_stock, thai_build):
    declared = [e for e in VENU_2["declared"] if e["part"] != "006-D8947-22"]
    with pytest.raises(ValueError, match="006-D8947-22, which the watch doesn't declare"):
        render(declared, replace=[thai_build], family="006-B3703-00")


@pytest.fixture(scope="session")
def korean_build(catalog_stock, inputs):
    from garmin_fonts.build import add_georgian

    return add_georgian(catalog_stock["006-D8945-22"], offline=True)


def test_the_korean_27_px_font_supplies_georgian_alone(catalog_stock, ours, korean_build):
    alone = by_char(render(VENU_2["declared"], replace=[korean_build], family="006-B3703-00"))
    item = alone["მ"]
    assert (item["state"], item["font"]) == ("confirmed", "006-D8945-22")
    assert {"venu-2-korean-host", "venu-2-compressed-korean"} <= set(item["evidence"])
    replace = [korean_build, *installed_builds(VENU_2["declared"], ours)]
    both = by_char(render(VENU_2["declared"], replace=replace, family="006-B3703-00"))["მ"]
    assert both["state"] == "coverage_only" and "006-D8945-22" in both["reason"]
