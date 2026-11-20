import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pytest

from garmin_fonts.build import DEFAULT_PART, PARTS
from garmin_fonts.codec import GEORGIAN, decode

# The 27 px build that rendered Georgian notifications on a Venu 2 (firmware 19.05).
VERIFIED_27PX_SHA256 = "d836712663bda88e4d54bef003f511fdda2425f9c6bb886d50d513c7508fd894"
PART_IDS = [part for part, _ in PARTS]


def test_27px_matches_hardware_verified_build(assets):
    out, _ = assets
    data = (out / "georgian" / "006-D9035-03.fnt").read_bytes()
    assert hashlib.sha256(data).hexdigest() == VERIFIED_27PX_SHA256


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
    assert [(f["part"], f["size_px"]) for f in manifest["fonts"]] == list(PARTS)
    assert [f["part"] for f in manifest["fonts"] if f["default"]] == [DEFAULT_PART]
    for font in manifest["fonts"]:
        assert (out / font["stock"]).read_bytes() == stock[font["part"]]
        for kind in ("stock", "georgian"):
            data = (out / font[kind]).read_bytes()
            assert font[kind] == f"{kind}/{font['part']}.fnt"
            assert font[f"{kind}_sha256"] == hashlib.sha256(data).hexdigest()
    assert (out / "LICENSES" / "OFL.txt").read_bytes() == inputs["license_file"].read_bytes()


def run_cli(*args):
    builder = Path(__file__).resolve().parents[1]
    command = [sys.executable, "-m", "garmin_fonts", *args]
    return subprocess.run(command, capture_output=True, text=True, cwd=builder)


def test_cli_reports_invalid_input_as_json(inputs, tmp_path):
    args = ["--noto", str(inputs["noto"]), "--license", str(inputs["license_file"]), "--json"]
    result = run_cli("--stock-dir", str(tmp_path), "--out", str(tmp_path / "out"), *args)
    assert result.returncode == 2
    assert json.loads(result.stderr)["error"]["code"] == "invalid_input"
    assert not (tmp_path / "out").exists()

    result = run_cli("--stock-dir", str(inputs["stock_dir"]), "--out", str(tmp_path), *args)
    assert result.returncode == 2
    assert json.loads(result.stderr)["error"]["code"] == "invalid_input"
