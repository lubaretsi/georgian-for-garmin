import copy
import hashlib
import json

import pytest
from jsonschema import Draft202012Validator
from registry_data import ENTRIES, REGISTRY, SCHEMA

from garmin_fonts.registry import RegistryError, check_pack, validate

SCHEMA_JSON = json.loads(SCHEMA.read_text())
PACK_ID = ENTRIES["packs"][0]["id"]


def test_registry_matches_its_schema():
    Draft202012Validator.check_schema(SCHEMA_JSON)
    Draft202012Validator(SCHEMA_JSON).validate(json.loads(REGISTRY.read_text()))
    validate(ENTRIES, SCHEMA_JSON)


def pack(entries):
    return entries["packs"][0]


def part(entries, index=0):
    return pack(entries)["parts"][index]


def append_part(entries, **changes):
    pack(entries)["parts"].append(part(entries) | changes)


def add_pack(entries, **changes):
    entries["packs"].append(copy.deepcopy(pack(entries)) | changes)


def set_part(field, value, index=0):
    return lambda entries: part(entries, index).__setitem__(field, value)


def set_tested(field, value):
    return lambda entries: pack(entries)["tested"][0].__setitem__(field, value)


MALFORMED = {
    "duplicate part in a pack": (
        lambda e: append_part(e, size_px=99),
        "part 006-D9035-01 is listed more than once",
    ),
    "duplicate part across packs": (
        lambda e: add_pack(e, id="other", parts=[part(e)], tested=[]),
        "part 006-D9035-01 is listed more than once",
    ),
    "duplicate pack id": (
        lambda e: add_pack(e, parts=[part(e) | {"part": "006-D9999-01"}], tested=[]),
        f"pack id {PACK_ID} is listed more than once",
    ),
    "duplicate size in a pack": (
        set_part("size_px", 21, index=1),
        f"pack {PACK_ID}: size 21 px is listed more than once",
    ),
    "uppercase hash": (
        set_part("sha256", part(ENTRIES)["sha256"].upper()),
        "packs/0/parts/0/sha256: .* does not match",
    ),
    "short hash": (set_part("sha256", "575b132a"), "packs/0/parts/0/sha256: .* does not match"),
    "malformed part number": (set_part("part", "6-D9035-01"), "packs/0/parts/0/part: "),
    "size as a string": (set_part("size_px", "21"), "packs/0/parts/0/size_px: '21' is not"),
    "zero size": (set_part("size_px", 0), "packs/0/parts/0/size_px: 0 is less than"),
    "malformed version": (set_part("version", "2.1"), "packs/0/parts/0/version: "),
    "URL off Garmin's server": (
        set_part("url", "https://example.com/006-D9035-01.fnt"),
        "packs/0/parts/0/url: ",
    ),
    "missing field": (
        lambda e: part(e).pop("version"),
        "packs/0/parts/0: 'version' is a required property",
    ),
    "unknown field": (set_part("sizepx", 21), r"packs/0/parts/0: Additional properties"),
    "malformed pack id": (lambda e: pack(e).__setitem__("id", "Armenian"), "packs/0/id: "),
    "no packs": (lambda e: e.__setitem__("packs", []), "packs: .* should be non-empty"),
    "tested part not in the pack": (
        set_tested("parts", ["006-D9035-09"]),
        "lists part 006-D9035-09, which is not in this pack",
    ),
    "duplicate tested entry": (
        lambda e: pack(e)["tested"].append(dict(pack(e)["tested"][0])),
        "006-B3703-00 with software 19.05 is listed more than once",
    ),
    "malformed software version": (set_tested("software", "19.5"), "tested/0/software: "),
    "bad Georgian font hash": (
        lambda e: e["georgian_font"].__setitem__("sha256", "x" * 64),
        "georgian_font/sha256: ",
    ),
}


@pytest.mark.parametrize("mutate, message", MALFORMED.values(), ids=MALFORMED.keys())
def test_malformed_registry_is_rejected(mutate, message):
    entries = copy.deepcopy(ENTRIES)
    mutate(entries)
    with pytest.raises(RegistryError, match=message):
        validate(entries, SCHEMA_JSON, source="registry.json")


def test_registry_error_names_the_file():
    entries = copy.deepcopy(ENTRIES)
    set_part("sha256", "nope")(entries)
    with pytest.raises(RegistryError, match="^registry.json: packs/0/parts/0/sha256: "):
        validate(entries, SCHEMA_JSON, source="registry.json")


def test_stock_files_match_the_registry(stock):
    for entry in ENTRIES["packs"]:
        check_pack(entry, stock)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def swap_sizes(entries):
    a, b = part(entries, 2), part(entries, 3)
    a["size_px"], b["size_px"] = b["size_px"], a["size_px"]


def mislabel_file(entries, stock):
    # The 27 px file listed (with its own hash) as the 24 px part.
    stock["006-D9035-02"] = stock["006-D9035-03"]
    part(entries, 1)["sha256"] = sha256(stock["006-D9035-03"])


def corrupt_file(entries, stock):
    stock["006-D9035-01"] = stock["006-D9035-01"][:-1]
    part(entries)["sha256"] = sha256(stock["006-D9035-01"])


STOCK_MISMATCH = {
    "size above the line height": (
        lambda e, s: set_part("size_px", 40, index=2)(e),
        "006-D9035-03: size 40 px does not match the stock font's 35 px line height",
    ),
    "size under half the line height": (
        lambda e, s: set_part("size_px", 17, index=2)(e),
        "006-D9035-03: size 17 px does not match the stock font's 35 px line height",
    ),
    "sizes swapped within the pack": (
        lambda e, s: swap_sizes(e),
        "006-D9035-04 at 27 px has line height 41 px, but 006-D9035-03 at 31 px has 35 px",
    ),
    "wrong version": (
        lambda e, s: set_part("version", "2.20")(e),
        "006-D9035-01: the stock file's footer has version 2.10, but the registry lists 2.20",
    ),
    "file of another part": (mislabel_file, "006-D9035-02: the stock file's footer names part 006-D9035-03"),
    "hash of another file": (
        lambda e, s: set_part("sha256", part(e, 1)["sha256"])(e),
        "006-D9035-01: stock file has SHA-256 575b",
    ),
    "unreadable file": (corrupt_file, "006-D9035-01: stock file: "),
}


@pytest.mark.parametrize("mutate, message", STOCK_MISMATCH.values(), ids=STOCK_MISMATCH.keys())
def test_registry_mismatching_stock_file_is_rejected(stock, mutate, message):
    entries, files = copy.deepcopy(ENTRIES), dict(stock)
    mutate(entries, files)
    with pytest.raises(RegistryError, match=f"^pack {PACK_ID}: .*{message}"):
        check_pack(pack(entries), files)
