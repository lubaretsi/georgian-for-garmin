import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest
from registry_data import ENTRIES, HOSTS, PART_IDS, PARTS, REGISTRY, SCHEMA

from garmin_fonts.build import DEFAULT_PART, build
from garmin_fonts.codec import GEORGIAN, decode
from garmin_fonts.georgian import (
    MKHEDRULI_X,
    MTAVRULI,
    Sizing,
    choose_sizing,
    measure,
    with_georgian,
)
from garmin_fonts.registry import RegistryError, weight

# Mkhedruli x-height matched to the Armenian, descenders shortened to fit (fonts/README.md, Size).
# The tactix 7 (software 27.18) rendered these builds of -00, -05, -06, -07 and -09; the others
# aren't tested on a watch yet. The Venu 2 (firmware 19.05) rendered the earlier builds of -01 to
# -04, drawn at the nominal size.
GEORGIAN_SHA256 = {
    "006-D9035-00": "87f3fca5f37b93dca75d6b78f593b16aa35949b67efe77a326d12545065b35f9",
    "006-D9035-01": "c90dc5c0d3d6eb39185a8fafaa99e81cf2e573e0ce8ed9e8e36c282d008c21cc",
    "006-D9035-02": "f7735b25dac42b24a4b8a0a1ab592ababf66a4ab4e0f4bb02f1abe8894effe6a",
    "006-D9035-03": "09e303cb78771212fb00c19608b9c177f51a172366583109003dec7cd2ef052a",
    "006-D9035-04": "de05a25b6a77e8167bd783fb785ac8a837874c9e33fce72a7b5ffc752ce427d5",
    "006-D9035-10": "155a7dff7d8608685b14f20cc8575b19b7ccc8640908d4dda6131e8253552a9d",
    "006-D9035-18": "73681017b6fe9cf123d398311a099111f58397f614f13405e87b92e912e43a9a",
    "006-D9035-19": "a1015d27360af8cecefe6f86bdfbda6423151ed2a7ca15cc57d3fb6c112e1e0f",
    "006-D9035-20": "d8e3b085b776cdb37132042a296dd875d53ec6ccc5dcd8aa2d3cc67b9cfc18c6",
    "006-D9035-11": "39e6c83bc094da85f2c7ca21ccad87926780460d2287ef242af70e4abf74a4fc",
    "006-D9035-12": "bac2dd0cfd7d1ecd193fc9d17fd593ae32dfd22e7d59c7eb174b5682d29f3884",
    "006-D9035-13": "31f43fede1ca229f48a4ed8486356a8ae2e4b6ebaf0839cd4dbc21376a77596a",
    "006-D9035-14": "8f593c49b3e7d5f08d7316e2476a05c1ac061ef1bd95faa99d971e4e58674bac",
    "006-D9035-05": "8f8b16f3823a57a4e585ffc1e01f3bd445fb42673e5d4d78a56ece7d82e83695",
    "006-D9035-17": "d1adb13a43b23d999f310210e55846a806e4b4769e53f16d65dee7a4af609252",
    "006-D9035-06": "57a6a6da6b8953e513b54e3556eca4c911976f93b6a34c27352d7f89e3917366",
    "006-D9035-07": "0c91ccd24c7cb2073b4043868815280f3fb33de5d239272b8c498e503762e160",
    "006-D9035-21": "f3d155b6b3a9f852858fb291fa9017636fb11503c030127f5d9b6c8023032981",
    "006-D9035-08": "2c321a9917f003d36678848305fa9b37b43b71e7aa83da94b01f1061f5cb32bd",
    "006-D9035-09": "bd048e8b593871fdf87ecedbfc82b359c6cd6572887a1d3535f4753654a9e809",
}
# Faint pixels dropped below the row, by part: the tips of დ, ლ and ღ. A font or rasterizer change
# that clips more fails.
CLIPPED = {
    "006-D9035-00": {"U+10D3": 1, "U+10DA": 1, "U+10E6": 2},
    "006-D9035-01": {"U+10D3": 1, "U+10DA": 1, "U+10E6": 1},
    "006-D9035-02": {"U+10D3": 1},
    "006-D9035-03": {"U+10D3": 2, "U+10DA": 2, "U+10E6": 2},
    "006-D9035-04": {"U+10D3": 2, "U+10DA": 2, "U+10E6": 1},
    "006-D9035-10": {"U+10DA": 1},
    "006-D9035-18": {"U+10D3": 1, "U+10DA": 1, "U+10E6": 1},
    "006-D9035-20": {"U+10D3": 1, "U+10DA": 1, "U+10E6": 2},
    "006-D9035-11": {"U+10D3": 1, "U+10DA": 1},
    "006-D9035-12": {"U+10D3": 2, "U+10E6": 2},
    "006-D9035-13": {"U+10D3": 2, "U+10DA": 2, "U+10E6": 1},
    "006-D9035-14": {"U+10DA": 1},
    "006-D9035-05": {"U+10D3": 1, "U+10DA": 1, "U+10E6": 1},
    "006-D9035-17": {"U+10D3": 1, "U+10DA": 1, "U+10E6": 1},
    "006-D9035-06": {"U+10D3": 1, "U+10DA": 1, "U+10E6": 2},
    "006-D9035-07": {"U+10D3": 1, "U+10DA": 1},
    "006-D9035-21": {"U+10DA": 1, "U+10E6": 1},
    "006-D9035-08": {"U+10D3": 1},
    "006-D9035-09": {"U+10D3": 1, "U+10DA": 1},
}


def test_every_part_is_pinned():
    assert sorted(GEORGIAN_SHA256) == sorted(PART_IDS)


@pytest.mark.parametrize("part", GEORGIAN_SHA256)
def test_georgian_builds_match_pinned_bytes(assets, part):
    out, _ = assets
    data = (out / "georgian" / f"{part}.fnt").read_bytes()
    assert hashlib.sha256(data).hexdigest() == GEORGIAN_SHA256[part]


def test_clipped_pixels_match_pinned_list(assets):
    _, manifest = assets
    assert {f["part"]: f["clipped"] for f in manifest["fonts"] if f.get("clipped")} == CLIPPED


def sizing_of(manifest, number):
    font = next(f for f in manifest["fonts"] if f["part"] == number)
    return Sizing(font["georgian_size_px"], font["georgian_descender_percent"])


@pytest.mark.parametrize("pack_id, part", PARTS, ids=PART_IDS)
def test_each_part_is_drawn_at_its_registry_weight(assets, stock, outlines, pack_id, part):
    out, manifest = assets
    pack = next(p for p in ENTRIES["packs"] if p["id"] == pack_id)
    number, sizing = part["part"], sizing_of(manifest, part["part"])
    host = HOSTS[pack["host"]]
    assert sizing == choose_sizing(stock[number], outlines(weight(pack, part)), part["size_px"], host)
    expected, _ = with_georgian(stock[number], outlines(weight(pack, part)), sizing)
    assert (out / "georgian" / f"{number}.fnt").read_bytes() == expected
    if "weight" in part:  # the override changes the result (at the pack's weight it may not fit)
        try:
            at_pack_weight, _ = with_georgian(stock[number], outlines(pack["weight"]), sizing)
        except ValueError:
            at_pack_weight = None
        assert at_pack_weight != expected


def heights(data):
    """(Armenian x-height, capitals, Mkhedruli x-height, Mtavruli height) of a built font."""
    header, _, cmap = decode(data)
    return tuple(
        measure(cmap, header[7], letters)
        for letters in (HOSTS["armenian"].x_height, HOSTS["armenian"].capitals, MKHEDRULI_X, MTAVRULI)
    )


@pytest.mark.parametrize("pack_id, part", PARTS, ids=PART_IDS)
def test_georgian_matches_the_armenian_x_height(assets, pack_id, part):
    out, _ = assets
    armenian, _, georgian, _ = heights((out / "georgian" / f"{part['part']}.fnt").read_bytes())
    assert abs(georgian - armenian) <= 1


def sizing_table(out, manifest):
    """The per-part table in fonts/README.md, as the build measures it."""
    lines = [
        "| Part           | Nominal | Armenian x-height | Armenian capitals | Georgian font size "
        "| Descenders | Mkhedruli x-height | Mtavruli height |",
        "| -------------- | ------- | ----------------- | ----------------- | ------------------ "
        "| ---------- | ------------------ | --------------- |",
    ]
    for font in (f for f in manifest["fonts"] if "size_px" in f):
        cells = heights((out / font["georgian"]).read_bytes())
        x, caps, mkhedruli, mtavruli = (f"{value:.1f}" for value in cells)
        lines.append(
            f"| `{font['part']}` | {font['size_px']:>4} px | {x:>17} | {caps:>17} "
            f"| {font['georgian_size_px']:>15} px | {font['georgian_descender_percent']:>9}% "
            f"| {mkhedruli:>18} | {mtavruli:>15} |"
        )
    return "\n".join(lines) + "\n"


def test_readme_lists_the_measured_sizes(assets):
    out, manifest = assets
    table = sizing_table(out, manifest)
    readme = (REGISTRY.parent / "README.md").read_text()
    assert table in readme, f"update the table in fonts/README.md to:\n{table}"


# codec.HEADER's fields; the build may move only the section offsets after the range table.
HEADER_FIELDS = ("header size", "flags", "end", "ranges offset", "index offset", "pixels offset",
                 "field 6", "height", *(f"field {i}" for i in range(8, 12)), "magic")
MOVED = {"end", "index offset", "pixels offset"}


@pytest.mark.parametrize("part", PART_IDS)
def test_georgian_build_adds_glyphs_and_preserves_stock(each_build, stock, part):
    out, _ = each_build
    header, footer, cmap = decode((out / "georgian" / f"{part}.fnt").read_bytes())
    stock_header, stock_footer, stock_cmap = decode(stock[part])

    assert footer == stock_footer, "footer (part, version)"
    changed = {name for name, new, old in zip(HEADER_FIELDS, header, stock_header, strict=True)
               if new != old}
    assert changed <= MOVED, f"header fields changed: {sorted(changed - MOVED)}"
    assert cmap.keys() == stock_cmap.keys() | set(GEORGIAN)
    for cp, tile in stock_cmap.items():
        assert cmap[cp] == tile, "missing glyph" if cp is None else f"U+{cp:04X}"
    missing = stock_cmap[None]
    for cp in GEORGIAN:
        _, bitmap = cmap[cp]
        assert any(bitmap) and cmap[cp] != missing, f"U+{cp:04X}"


def test_asset_directory_matches_contract(assets, inputs, stock, truetype_stock):
    out, manifest = assets
    assert json.loads((out / "manifest.json").read_text()) == manifest
    assert [f["part"] for f in manifest["fonts"] if f["default"]] == [DEFAULT_PART]
    stock_files = stock | truetype_stock
    assert sorted(f["part"] for f in manifest["fonts"]) == sorted(stock_files)
    for font in manifest["fonts"]:
        suffix = ".fnt" if font["part"] in stock else ".bin"
        assert (out / font["stock"]).read_bytes() == stock_files[font["part"]]
        for kind in ("stock", "georgian"):
            data = (out / font[kind]).read_bytes()
            assert font[kind] == f"{kind}/{font['part']}{suffix}"
            assert font[f"{kind}_sha256"] == hashlib.sha256(data).hexdigest()
        if suffix == ".bin":
            assert font["preview"] == f"previews/{font['part']}.png"
            assert (out / font["preview"]).read_bytes()[:8] == b"\x89PNG\r\n\x1a\n"
        else:
            assert "preview" not in font
    assert (out / "LICENSES" / "OFL.txt").read_bytes() == inputs["license_file"].read_bytes()


def manifest_view(entries):
    """What the manifest must say about the registry `entries`: per font, the fields it must
    carry with their values (bitmap parts have a size, TrueType parts don't); per pack, all."""
    fonts = [
        {"part": part["part"], "pack": pack["id"], "version": part["version"],
         "stock_sha256": part["sha256"], "earlier_georgian": part.get("earlier_georgian", [])}
        | ({"size_px": part["size_px"]} if pack["kind"] == "bitmap" else {})
        for pack in entries["packs"]
        for part in pack["parts"]
    ]
    packs = [
        {"id": pack["id"], "name": pack["name"], "kind": pack["kind"], "weight": pack["weight"],
         "install": pack.get("install", True),
         "parts": [p["part"] for p in pack["parts"]], "tested": pack["tested"]}
        for pack in entries["packs"]
    ]
    return fonts, packs


def test_manifest_reflects_registry(assets):
    _, manifest = assets
    fonts, packs = manifest_view(ENTRIES)
    assert [{key: font.get(key) for key in view} for font, view in zip(manifest["fonts"], fonts)] \
        == fonts
    assert len(manifest["fonts"]) == len(fonts)
    assert not any("size_px" in font for font in manifest["fonts"][len(PART_IDS):])
    assert manifest["packs"] == packs
    # The support list, each reason written out where a model names it.
    watches = {w["model_part"]: w for w in manifest["watches"]}
    assert [w["model_part"] for w in manifest["watches"]] == \
        [w["model_part"] for w in ENTRIES["watches"]]
    reasons = {r["id"]: r for r in ENTRIES["reasons"]}
    for entry in ENTRIES["watches"]:
        written = watches[entry["model_part"]]
        if "unsupported" in entry:
            reason = reasons[entry["unsupported"]["reason"]]
            assert written["unsupported"] == {
                "software": entry["unsupported"]["software"],
                "reason": {"en": reason["en"], "ka": reason["ka"]},
            }
        assert {k: v for k, v in written.items() if k != "unsupported"} == \
            {k: v for k, v in entry.items() if k != "unsupported"}


def write_registry(directory, entries):
    directory.mkdir(exist_ok=True)
    shutil.copy(SCHEMA, directory / SCHEMA.name)
    path = directory / "registry.json"
    path.write_text(json.dumps(entries))
    return path


def test_georgian_font_hash_is_checked(inputs, tmp_path):
    source = ENTRIES["georgian_font"] | {"sha256": "0" * 64}
    registry = write_registry(tmp_path / "registry", ENTRIES | {"georgian_font": source})
    with pytest.raises(RegistryError, match="but the registry lists 0{64}"):
        build(**(inputs | {"registry": registry}), out=tmp_path / "out")
    assert not (tmp_path / "out").exists()


def run_cli(*args):
    builder = Path(__file__).resolve().parents[1]
    command = [sys.executable, "-m", "garmin_fonts", "build", *args]
    return subprocess.run(command, capture_output=True, text=True, cwd=builder)


def cli_args(inputs, **overrides):
    values = {"registry": inputs["registry"], "stock_dir": inputs["stock_dir"],
              "noto": inputs["noto"], "license": inputs["license_file"]} | overrides
    args = [arg for key, value in values.items() for arg in (f"--{key.replace('_', '-')}", str(value))]
    return [*args, "--json"]


def test_cli_reports_invalid_input_as_json(inputs, tmp_path):
    result = run_cli(*cli_args(inputs, stock_dir=tmp_path, out=tmp_path / "out"))
    assert result.returncode == 2
    assert json.loads(result.stderr)["error"]["code"] == "invalid_input"
    assert not (tmp_path / "out").exists()

    result = run_cli(*cli_args(inputs, out=tmp_path))
    assert result.returncode == 2
    assert json.loads(result.stderr)["error"]["code"] == "invalid_input"


def test_cli_rejects_malformed_registry(inputs, tmp_path):
    pack = ENTRIES["packs"][0]
    duplicated = pack | {"parts": [*pack["parts"], pack["parts"][0]]}
    registry = write_registry(tmp_path / "registry", ENTRIES | {"packs": [duplicated]})
    result = run_cli(*cli_args(inputs, registry=registry, out=tmp_path / "out"))
    assert result.returncode == 2
    error = json.loads(result.stderr)["error"]
    assert error["code"] == "invalid_input"
    assert f"part {PARTS[0][1]['part']} is listed more than once" in error["message"]
    assert not (tmp_path / "out").exists()
