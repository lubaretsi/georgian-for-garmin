import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest
from registry_data import ENTRIES, PART_IDS, PARTS, SCHEMA

from garmin_fonts.build import DEFAULT_PART, build
from garmin_fonts.codec import GEORGIAN, decode
from garmin_fonts.registry import RegistryError

# The 27 px build rendered Georgian notifications on a Venu 2 (firmware 19.05); the others are
# pinned to the build that shipped alongside it.
GEORGIAN_SHA256 = {
    "006-D9035-01": "33a9299cd4432f8995f5530b30e6538a5ce588fc2b46daa921af508a5737e298",
    "006-D9035-02": "3b5c9131e66193fc7878883e82070ad7bef9f75547991906394d6c3dd8248543",
    "006-D9035-03": "d836712663bda88e4d54bef003f511fdda2425f9c6bb886d50d513c7508fd894",
    "006-D9035-04": "5cbe821747efab4310f94a66c1f5018b81950265dd1c16a0d7aa8bdaef41f63a",
}


@pytest.mark.parametrize("part", GEORGIAN_SHA256)
def test_georgian_builds_match_pinned_bytes(assets, part):
    out, _ = assets
    data = (out / "georgian" / f"{part}.fnt").read_bytes()
    assert hashlib.sha256(data).hexdigest() == GEORGIAN_SHA256[part]


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
        {"id": pack["id"], "name": pack["name"], "parts": [p["part"] for p in pack["parts"]],
         "tested": pack["tested"]}
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


def test_every_registry_pack_is_built(inputs, tmp_path):
    # Split the one real pack in two to show the builder isn't tied to it.
    pack = ENTRIES["packs"][0]
    first = pack | {"id": "small", "parts": pack["parts"][:2], "tested": []}
    second = pack | {"id": "large", "parts": pack["parts"][2:]}
    entries = ENTRIES | {"packs": [first, second]}
    registry = write_registry(tmp_path / "registry", entries)
    manifest = build(**(inputs | {"registry": registry}), out=tmp_path / "out")

    fonts, packs = manifest_view(entries)
    assert [{key: font[key] for key in fonts[0]} for font in manifest["fonts"]] == fonts
    assert manifest["packs"] == packs
    assert {f["part"]: f["georgian_sha256"] for f in manifest["fonts"]} == GEORGIAN_SHA256


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
