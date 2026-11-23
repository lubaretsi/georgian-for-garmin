"""The repository's font registry, loaded once for test parametrization."""

from pathlib import Path

from garmin_fonts.hosts import hosts
from garmin_fonts.registry import load, parts

REGISTRY = Path(__file__).resolve().parents[2] / "fonts" / "registry.json"
SCHEMA = REGISTRY.with_name("registry.schema.json")
ENTRIES = load(REGISTRY)
# (pack id, part entry) for every bitmap part, in registry order.
PARTS = [
    (pack["id"], part)
    for pack in ENTRIES["packs"] if pack["kind"] == "bitmap"
    for part in pack["parts"]
]
PART_IDS = [part["part"] for _, part in PARTS]
# (pack, part entry) for every TrueType part, in registry order.
TRUETYPE_PARTS = [
    (pack, part)
    for pack in ENTRIES["packs"] if pack["kind"] == "truetype"
    for part in pack["parts"]
]
TRUETYPE_PART_IDS = [part["part"] for _, part in TRUETYPE_PARTS]
# Host families, by id.
HOSTS = hosts(ENTRIES)
# Every pinned bitmap part of a host other than Armenian: the registry's `stock` (outside the
# packs) and the Korean packs' parts, the Chinese, Japanese, Korean and Thai fonts the codec,
# host and catalog tests use.
STOCK_PART_IDS = [part["part"] for part in ENTRIES["stock"]] + [
    part["part"]
    for pack in ENTRIES["packs"] if pack["kind"] == "bitmap" and pack["host"] != "armenian"
    for part in pack["parts"]
]
# Every pinned part, packs and stock, by part number.
PINS = parts(ENTRIES)
