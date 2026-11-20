"""The repository's font registry, loaded once for test parametrization."""

from pathlib import Path

from garmin_fonts.registry import load

REGISTRY = Path(__file__).resolve().parents[2] / "fonts" / "registry.json"
SCHEMA = REGISTRY.with_name("registry.schema.json")
ENTRIES = load(REGISTRY)
# (pack id, part entry) for every part, in registry order.
PARTS = [(pack["id"], part) for pack in ENTRIES["packs"] for part in pack["parts"]]
PART_IDS = [part["part"] for _, part in PARTS]
