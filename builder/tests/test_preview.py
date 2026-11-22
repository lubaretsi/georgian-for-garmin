import json
import subprocess
import sys
from pathlib import Path

from registry_data import REGISTRY

from garmin_fonts.codec import decode
from garmin_fonts.preview import ARMENIAN_SAMPLE, GREYS, line, readme_image, sheet, truetype_preview
from garmin_fonts.truetype import unmask


def test_line_is_drawn_from_the_glyph_bitmaps(stock):
    font = stock["006-D9035-03"]
    header, _, cmap = decode(font)
    levels = line(font, "ա ա")
    width = cmap[ord("ա")][0]
    assert len(levels) == header[7]
    assert all(len(row) == 2 * width + header[7] // 3 for row in levels)
    assert [row[:width] for row in levels] == [row[-width:] for row in levels]


def test_sheet_scales_each_pixel_to_a_square(stock):
    levels = line(stock["006-D9035-03"], ARMENIAN_SAMPLE)
    image = sheet([("label", levels)], scale=3, margin=0)
    assert image.width == 3 * len(levels[0])
    y, x = next((y, x) for y, row in enumerate(levels) for x, level in enumerate(row) if level == 3)
    top = image.height - 3 * len(levels)
    block = {image.getpixel((3 * x + dx, top + 3 * y + dy)) for dx in range(3) for dy in range(3)}
    assert block == {GREYS[3]}


def test_readme_image_shows_armenian_and_georgian(inputs):
    image = readme_image(REGISTRY, inputs["stock_dir"], inputs["noto"], "006-D9035-03")
    assert image.mode == "L"
    assert {color for _, color in image.getcolors()} >= set(GREYS)


def test_cli_rejects_unknown_part_as_json(inputs, tmp_path):
    builder = Path(__file__).resolve().parents[1]
    result = subprocess.run(
        [sys.executable, "-m", "garmin_fonts.preview", "--stock-dir", str(inputs["stock_dir"]),
         "--noto", str(inputs["noto"]), "--part", "006-D9035-99", "--out",
         str(tmp_path / "x.png"), "--json"],
        capture_output=True, text=True, cwd=builder,
    )
    assert result.returncode == 2
    assert json.loads(result.stderr)["error"]["code"] == "invalid_input"
    assert not (tmp_path / "x.png").exists()


def test_truetype_preview_draws_host_text_and_georgian(assets):
    out, _ = assets
    image = truetype_preview(unmask((out / "georgian" / "006-D0952-08.bin").read_bytes()), "label")
    assert image.mode == "L"
    # Three rows of ink: the label and the line at each size.
    inked = [y for y in range(image.height) if any(image.getpixel((x, y)) for x in range(image.width))]
    bands = sum(1 for a, b in zip([-2, *inked], inked) if b - a > 1)
    assert bands == 3
