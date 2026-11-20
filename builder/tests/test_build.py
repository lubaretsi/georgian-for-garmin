import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest
from registry_data import ENTRIES, PART_IDS, PARTS, SCHEMA

from garmin_fonts.build import DEFAULT_PART, build
from garmin_fonts.codec import GEORGIAN, decode, with_georgian
from garmin_fonts.registry import RegistryError, weight

# The 27 px build rendered Georgian notifications on a Venu 2 (firmware 19.05); 21, 24 and 31 px
# are pinned to the build that shipped alongside it. The other parts are pinned to their first
# build and are not yet tested on a watch.
GEORGIAN_SHA256 = {
    "006-D9035-00": "72246049761da1fa7696c806fbc59c463a3aa5436eb4b17964a6c68ecfc19d04",
    "006-D9035-01": "33a9299cd4432f8995f5530b30e6538a5ce588fc2b46daa921af508a5737e298",
    "006-D9035-02": "3b5c9131e66193fc7878883e82070ad7bef9f75547991906394d6c3dd8248543",
    "006-D9035-03": "d836712663bda88e4d54bef003f511fdda2425f9c6bb886d50d513c7508fd894",
    "006-D9035-04": "5cbe821747efab4310f94a66c1f5018b81950265dd1c16a0d7aa8bdaef41f63a",
    "006-D9035-10": "ec1e706b1ad9c242a9ffa61593572042272f738d36036b81b85a2c2217f22877",
    "006-D9035-18": "672ca2dfb10674bec30c5fe84e84d6b8be8de69676eafe8e0439a2a6e9aa6bb1",
    "006-D9035-19": "672639ec6ccf504f18348e7d3ebbb1dd6d47d22dcc0b7354b6d9fabc81d49eea",
    "006-D9035-20": "5624b793b5ea0c2cf0aeba5a46ec9fb22e4acd958e5f3ae4f6448b464a155d30",
    "006-D9035-11": "176550847321f9a138fe36db3e77d3b5896265a8154480f5f3ce7e04ca474fe6",
    "006-D9035-12": "21e20af4c0caec27f2c9c3d87cb3113666f74b9f6add4dcc78580703012c972f",
    "006-D9035-13": "b6e7d89de0774dfddf40d252513e52f873e0e168a6d0c85d7dfb28428105112e",
    "006-D9035-14": "32f5c5da069d3a3c7c107a57373c6043868e9bb4e9e39a1ee325828a4fe6a80d",
    "006-D9035-05": "d360b8b94b13dfa2b8f7d1d8cb67b4b1a8d9bd221bd9d9fa264a10f17ba7f2c5",
    "006-D9035-17": "d64245b275ff0395c85d479a19cf7d61660ad7c971474be5f518d7bd6c273409",
    "006-D9035-06": "ef26e6aaa2030d9006f1864a9979b9e591d2fafb6af6d17b5fc28c01cf6b66b0",
    "006-D9035-07": "2acc6527e8ad14e9c3d5d826af14e9f035cca8e1b468c677c079bb399a7bb621",
    "006-D9035-21": "d15421a45cc99f8ae0d48e3261c41723ff90847773762fbe6826344f0c65f2ea",
    "006-D9035-08": "70cd24f0f750af1862b43d80f653fbca874f2db27255e92aa999dabae6ef20d1",
    "006-D9035-09": "02aa72e5ce026393008391e10e9d566521c41d9ef610f31503e941c07d2de648",
}
# Faint pixels dropped below the row, by part: the 12 and 13 px parts (identical glyphs, three
# rows below the baseline) lose the tip of ლ. A font or rasterizer change that clips more fails.
CLIPPED = {"006-D9035-05": {"U+10DA": 1}, "006-D9035-18": {"U+10DA": 1}}


def test_every_part_is_pinned():
    assert sorted(GEORGIAN_SHA256) == sorted(PART_IDS)


@pytest.mark.parametrize("part", GEORGIAN_SHA256)
def test_georgian_builds_match_pinned_bytes(assets, part):
    out, _ = assets
    data = (out / "georgian" / f"{part}.fnt").read_bytes()
    assert hashlib.sha256(data).hexdigest() == GEORGIAN_SHA256[part]


def test_clipped_pixels_match_pinned_list(assets):
    _, manifest = assets
    assert {f["part"]: f["clipped"] for f in manifest["fonts"] if f["clipped"]} == CLIPPED


@pytest.mark.parametrize("pack_id, part", PARTS, ids=PART_IDS)
def test_each_part_is_drawn_at_its_registry_weight(assets, stock, inputs, pack_id, part):
    out, _ = assets
    pack = next(p for p in ENTRIES["packs"] if p["id"] == pack_id)
    number, size = part["part"], part["size_px"]
    expected, _ = with_georgian(stock[number], inputs["noto"], size, weight(pack, part))
    assert (out / "georgian" / f"{number}.fnt").read_bytes() == expected
    if "weight" in part:  # the override changes the result (at the pack's weight it may not fit)
        try:
            at_pack_weight, _ = with_georgian(stock[number], inputs["noto"], size, pack["weight"])
        except ValueError:
            at_pack_weight = None
        assert at_pack_weight != expected


@pytest.mark.parametrize("part", PART_IDS)
def test_georgian_build_adds_glyphs_and_preserves_stock(assets, stock, part):
    out, _ = assets
    header, footer, cmap = decode((out / "georgian" / f"{part}.fnt").read_bytes())
    stock_header, stock_footer, stock_cmap = decode(stock[part])

    assert footer == stock_footer
    # Only the section offsets (end, ranges, index, pixels) may move.
    assert header[:2] + header[6:] == stock_header[:2] + stock_header[6:]
    assert cmap.keys() == stock_cmap.keys() | set(GEORGIAN)
    assert all(cmap[cp] == tile for cp, tile in stock_cmap.items())
    missing = stock_cmap[None]
    for cp in GEORGIAN:
        _, bitmap = cmap[cp]
        assert any(bitmap) and cmap[cp] != missing, f"U+{cp:04X}"


def test_asset_directory_matches_contract(assets, inputs, stock):
    out, manifest = assets
    assert json.loads((out / "manifest.json").read_text()) == manifest
    assert [f["part"] for f in manifest["fonts"] if f["default"]] == [DEFAULT_PART]
    for font in manifest["fonts"]:
        assert (out / font["stock"]).read_bytes() == stock[font["part"]]
        for kind in ("stock", "georgian"):
            data = (out / font[kind]).read_bytes()
            assert font[kind] == f"{kind}/{font['part']}.fnt"
            assert font[f"{kind}_sha256"] == hashlib.sha256(data).hexdigest()
    assert (out / "LICENSES" / "OFL.txt").read_bytes() == inputs["license_file"].read_bytes()


def manifest_view(entries):
    """What the manifest must say about the registry `entries`."""
    fonts = [
        {"part": part["part"], "pack": pack["id"], "size_px": part["size_px"],
         "stock_sha256": part["sha256"]}
        for pack in entries["packs"]
        for part in pack["parts"]
    ]
    packs = [
        {"id": pack["id"], "name": pack["name"], "weight": pack["weight"],
         "parts": [p["part"] for p in pack["parts"]], "tested": pack["tested"]}
        for pack in entries["packs"]
    ]
    return fonts, packs


def test_manifest_reflects_registry(assets):
    _, manifest = assets
    fonts, packs = manifest_view(ENTRIES)
    assert [{key: font[key] for key in fonts[0]} for font in manifest["fonts"]] == fonts
    assert manifest["packs"] == packs


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
    command = [sys.executable, "-m", "garmin_fonts", *args]
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
