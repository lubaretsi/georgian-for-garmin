import os
from pathlib import Path

import pytest

from garmin_fonts.build import PARTS, build

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
    return {key: Path(os.environ[var]) for key, var in INPUTS.items()}


@pytest.fixture(scope="session")
def stock(inputs):
    return {part: (inputs["stock_dir"] / f"{part}.fnt").read_bytes() for part, _ in PARTS}


@pytest.fixture(scope="session")
def assets(inputs, tmp_path_factory):
    out = tmp_path_factory.mktemp("assets") / "fonts"
    manifest = build(out=out, **inputs)
    return out, manifest
