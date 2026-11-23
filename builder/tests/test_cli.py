"""The builder's command line: fetch, build, render, roundtrip, per the interfaces rule."""

import json
import subprocess
import sys
from pathlib import Path

from registry_data import PINS

from garmin_fonts.render import REPORTS

BUILDER = Path(__file__).resolve().parents[1]


def cli(*args, stdin=None):
    return subprocess.run([sys.executable, "-m", "garmin_fonts", *args], capture_output=True,
                          text=True, cwd=BUILDER, input=stdin)


def test_help_json_lists_the_commands_as_mcp_tools():
    result = cli("--help", "--json")
    assert result.returncode == 0
    tools = json.loads(result.stdout)["tools"]
    assert [t["name"] for t in tools] == ["fetch", "build", "render", "roundtrip"]
    for tool in tools:
        assert tool["inputSchema"]["type"] == "object" and tool["outputSchema"]["type"] == "object"
        assert set(tool["annotations"]) >= {"readOnlyHint", "destructiveHint", "idempotentHint",
                                            "openWorldHint"}
    assert [t["name"] for t in json.loads(cli("render", "--help", "--json").stdout)["tools"]] == \
        ["render"]
    for command in ("fetch", "build", "render", "roundtrip"):
        assert "example:" in cli(command, "--help").stdout


def test_fetch_and_roundtrip(catalog_stock, tmp_path):
    result = cli("fetch", "006-D8945-67", "--out", str(tmp_path), "--offline", "--json")
    assert result.returncode == 0, result.stderr
    (font,) = json.loads(result.stdout)["fonts"]
    assert font["sha256"] == PINS["006-D8945-67"].sha256 and font["pinned"]
    assert Path(font["path"]).read_bytes() == catalog_stock["006-D8945-67"]
    result = cli("roundtrip", font["path"], "006-D9035-03", "--json")
    assert result.returncode == 0, result.stderr
    output = json.loads(result.stdout)
    assert output["ok"] and [f["compressed"] for f in output["fonts"]] == [True, False]


def split_first_range(data):
    """`data` with its first multi-character Unicode range split in two: the same glyphs, laid
    out as the codec never writes them."""
    from struct import iter_unpack, pack, unpack_from

    end, ranges_start, index_start, pixels_start = unpack_from("<4I", data, 4)
    rows = list(iter_unpack("<III", data[ranges_start + 16:index_start]))
    i = next(i for i, (lo, hi, _) in enumerate(rows) if hi > lo)
    lo, hi, first = rows[i]
    rows[i:i + 1] = [(lo, lo, first), (lo + 1, hi, first + 1)]
    table = pack("<4I", 12, 16 + 12 * len(rows), 0, len(rows)) + b"".join(pack("<III", *r) for r in rows)
    header = bytearray(data[:ranges_start])
    header[4:20] = pack("<4I", end + 12, ranges_start, index_start + 12, pixels_start + 12)
    return bytes(header) + table + data[index_start:]


def test_a_failed_round_trip_exits_1(catalog_stock, tmp_path):
    from garmin_fonts.codec import decode

    odd = tmp_path / "odd.fnt"
    odd.write_bytes(split_first_range(catalog_stock["006-D8947-20"]))
    assert decode(odd.read_bytes())[2] == decode(catalog_stock["006-D8947-20"])[2]
    result = cli("roundtrip", str(odd), "--json")
    assert result.returncode == 1
    assert json.loads(result.stdout)["fonts"][0]["roundtrip"] is False


def test_build_one_font_with_markers(catalog_stock, tmp_path, inputs):
    result = cli("build", "006-D8947-22", "--markers", "ჰ=გ", "ก=ข", "--out", str(tmp_path),
                 "--offline", "--json")
    assert result.returncode == 0, result.stderr
    (font,) = json.loads(result.stdout)["fonts"]
    assert font["sha256"] == "e9536a34b05f2e072563324ade79796f9fb5df1c4da8e55eccaf709c49f6bff7"
    assert font["markers"] == {"ჰ": "გ", "ก": "ข"} and font["host"] == "thai"
    assert (tmp_path / "006-D8947-22.fnt").is_file()
    again = cli("build", "006-D8947-22", "--out", str(tmp_path), "--offline", "--json")
    assert again.returncode == 2 and "exists" in json.loads(again.stderr)["error"]["message"]


def test_render_a_report(catalog_stock, tmp_path, inputs):
    png = tmp_path / "venu2.png"
    report = REPORTS / "maintainer-venu2-stock-imported.json"
    result = cli("render", "--report", str(report), "--with-our-fonts", "--out", str(png),
                 "--json")
    assert result.returncode == 0, result.stderr
    output = json.loads(result.stdout)
    assert output["model"] == "venu-2-notifications" and output["png"] == str(png)
    assert output["counts"]["confirmed"] == 16
    assert png.read_bytes()[:4] == b"\x89PNG"
    via_input = cli("render", "--input", "-", "--json",
                    stdin=json.dumps({"report": str(report), "text": "ა"}))
    assert json.loads(via_input.stdout)["trace"][0]["font"] == "missing"


def test_invalid_input_exits_2_with_a_json_error(tmp_path):
    for args in (["fetch", "--json"], ["fetch", "D8947", "--json"],
                 ["build", "006-D8947-22", "--markers", "ჰ", "--out", str(tmp_path), "--json"],
                 ["render", "--json"], ["frobnicate", "--json"],
                 ["render", "--input", "-", "--json"]):
        result = cli(*args, stdin="{not json")
        assert result.returncode == 2, (args, result.stderr)
        assert json.loads(result.stderr)["error"]["code"] == "invalid_input", args


def test_an_unavailable_font_exits_1(tmp_path):
    result = cli("fetch", "006-D8947-99", "--offline", "--json")
    assert result.returncode == 1
    assert json.loads(result.stderr)["error"]["code"] == "font_unavailable"
