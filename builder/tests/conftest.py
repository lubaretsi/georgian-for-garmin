import os
from pathlib import Path

import pytest
from registry_data import PART_IDS, REGISTRY, TRUETYPE_PART_IDS

from garmin_fonts.build import build
from garmin_fonts.georgian import Outlines

# Set by `nix develop` and the flake check to the pinned inputs.
INPUTS = {
    "stock_dir": "GFG_STOCK_DIR",
    "noto": "GFG_NOTO_SANS_GEORGIAN",
    "license_file": "GFG_NOTO_OFL",
}


@pytest.fixture(scope="session")
def inputs():
    missing = [var for var in INPUTS.values() if not os.environ.get(var)]
    if missing:
        pytest.fail(f"run inside `nix develop` (unset: {', '.join(missing)})")
    return {"registry": REGISTRY} | {key: Path(os.environ[var]) for key, var in INPUTS.items()}


@pytest.fixture(scope="session")
def stock(inputs):
    return {part: (inputs["stock_dir"] / f"{part}.fnt").read_bytes() for part in PART_IDS}


@pytest.fixture(scope="session")
def truetype_stock(inputs):
    """Garmin's masked TrueType files, by part."""
    return {part: (inputs["stock_dir"] / f"{part}.bin").read_bytes() for part in TRUETYPE_PART_IDS}


@pytest.fixture(scope="session")
def assets(inputs, tmp_path_factory):
    out = tmp_path_factory.mktemp("assets") / "fonts"
    manifest = build(out=out, **inputs)
    return out, manifest


@pytest.fixture(scope="session")
def outlines(inputs):
    """Noto Sans Georgian at a weight, loaded once per weight: outlines(400)."""
    loaded = {}

    def at(weight):
        if weight not in loaded:
            loaded[weight] = Outlines(inputs["noto"], weight)
        return loaded[weight]

    return at
