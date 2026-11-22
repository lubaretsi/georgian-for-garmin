import copy
import hashlib
import json

import pytest
from jsonschema import Draft202012Validator
from registry_data import ENTRIES, REGISTRY, SCHEMA

from garmin_fonts.registry import RegistryError, check_pack, validate
from garmin_fonts.truetype import unmask

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
        "part 006-D9035-00 is listed more than once",
    ),
    "duplicate part across packs": (
        lambda e: add_pack(e, id="other", parts=[part(e)], tested=[]),
        "part 006-D9035-00 is listed more than once",
    ),
    "duplicate pack id": (
        lambda e: add_pack(e, parts=[part(e) | {"part": "006-D9999-01"}], tested=[]),
        f"pack id {PACK_ID} is listed more than once",
    ),
    "duplicate size in a pack": (
        set_part("size_px", 19, index=1),
        f"pack {PACK_ID}: size 19 px is listed more than once",
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
    "missing pack weight": (
        lambda e: pack(e).pop("weight"),
        "packs/0: 'weight' is a required property",
    ),
    "pack weight under the axis": (
        lambda e: pack(e).__setitem__("weight", 50),
        "packs/0/weight: 50 is less than the minimum of 100",
    ),
    "part weight over the axis": (
        set_part("weight", 1000),
        "packs/0/parts/0/weight: 1000 is greater than the maximum of 900",
    ),
    "weight as a name": (set_part("weight", "Bold"), "packs/0/parts/0/weight: 'Bold' is not"),
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

# Support list: the first unsupported model, for the schema's error paths.
UNSUPPORTED = next(i for i, w in enumerate(ENTRIES["watches"]) if "unsupported" in w)


def watch(entries, index=0):
    return entries["watches"][index]


MALFORMED |= {
    "no support list": (lambda e: e.pop("watches"), "'watches' is a required property"),
    "duplicate model in the support list": (
        lambda e: e["watches"].append(dict(watch(e))),
        "watches: 006-B3703-00 is listed more than once",
    ),
    "tested model not in the support list": (
        set_tested("model_part", "006-B9999-00"),
        "tested on 006-B9999-00, which is not in watches",
    ),
    "tested model listed as unsupported": (
        set_tested("model_part", ENTRIES["watches"][UNSUPPORTED]["model_part"]),
        "which watches lists as unsupported",
    ),
    "unsupported model without an issue": (
        lambda e: watch(e, UNSUPPORTED).__setitem__("issues", []),
        "is unsupported but names no issue to follow",
    ),
    "unsupported model without software": (
        lambda e: watch(e, UNSUPPORTED)["unsupported"].__setitem__("software", []),
        f"watches/{UNSUPPORTED}/unsupported/software: .* should be non-empty",
    ),
    "reason without Georgian": (
        lambda e: e["reasons"][0].pop("ka"),
        "reasons/0: 'ka' is a required property",
    ),
    "unknown reason": (
        lambda e: watch(e, UNSUPPORTED)["unsupported"].__setitem__("reason", "nope"),
        "names reason nope, which reasons lacks",
    ),
    "unused reason": (
        lambda e: e["reasons"].append(dict(e["reasons"][0], id="spare")),
        "reasons: spare is not used",
    ),
    "duplicate reason": (
        lambda e: e["reasons"].append(dict(e["reasons"][0])),
        f"reasons: {ENTRIES['reasons'][0]['id']} is listed more than once",
    ),
    "issue as text": (
        lambda e: watch(e).__setitem__("issues", ["#4"]),
        "watches/0/issues/0: '#4' is not of type 'integer'",
    ),
    "malformed model part": (
        lambda e: watch(e).__setitem__("model_part", "B3703"),
        "watches/0/model_part: ",
    ),
    "install only in betas": (
        lambda e: e["packs"][0].__setitem__("install", "beta"),
        "packs/0/install: 'beta' is not of type 'boolean'",
    ),
}

# The first TrueType pack's index, for the schema's error paths.
TT = next(i for i, p in enumerate(ENTRIES["packs"]) if p["kind"] == "truetype")


def tt_part(entries, index=0):
    return entries["packs"][TT]["parts"][index]


MALFORMED |= {
    "missing kind": (
        lambda e: pack(e).pop("kind"),
        "packs/0: 'kind' is a required property",
    ),
    "unknown kind": (
        lambda e: pack(e).__setitem__("kind", "vector"),
        "packs/0/kind: 'vector' is not one of",
    ),
    "size on a TrueType part": (
        lambda e: tt_part(e).__setitem__("size_px", 20),
        f"packs/{TT}/parts/0: Additional properties are not allowed \\('size_px'",
    ),
    "bitmap URL on a TrueType part": (
        lambda e: tt_part(e).__setitem__("url", tt_part(e)["url"][:-4] + ".fnt"),
        f"packs/{TT}/parts/0/url: ",
    ),
    "TrueType part in a bitmap pack": (
        lambda e: pack(e)["parts"].append(dict(tt_part(e))),
        "packs/0/parts/5: 'size_px' is a required property",
    ),
    "duplicate weight in a TrueType pack": (
        lambda e: tt_part(e, 1).pop("weight"),
        f"pack {ENTRIES['packs'][TT]['id']}: weight 400 is listed more than once",
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


def test_stock_files_match_the_registry(stock, truetype_stock):
    for entry in ENTRIES["packs"]:
        check_pack(entry, stock | truetype_stock)


TRUETYPE_MISMATCH = {
    "hash of another file": (
        lambda e, s: tt_part(e).__setitem__("sha256", tt_part(e, 1)["sha256"]),
        "006-D9971-09: stock file has SHA-256 d0fc",
    ),
    "unmasked file": (
        lambda e, s: s.__setitem__("006-D9971-09", unmask(s["006-D9971-09"]))
        or tt_part(e).__setitem__("sha256", sha256(s["006-D9971-09"])),
        "006-D9971-09: stock file: not a masked TrueType font",
    ),
}


@pytest.mark.parametrize("mutate, message", TRUETYPE_MISMATCH.values(), ids=TRUETYPE_MISMATCH.keys())
def test_registry_mismatching_truetype_file_is_rejected(truetype_stock, mutate, message):
    entries, files = copy.deepcopy(ENTRIES), dict(truetype_stock)
    mutate(entries, files)
    pack_entry = entries["packs"][TT]
    with pytest.raises(RegistryError, match=f"^pack {pack_entry['id']}: .*{message}"):
        check_pack(pack_entry, files)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def swap_sizes(entries):
    a, b = part(entries, 3), part(entries, 4)
    a["size_px"], b["size_px"] = b["size_px"], a["size_px"]


def mislabel_file(entries, stock):
    # The 27 px file listed (with its own hash) as the 24 px part.
    stock["006-D9035-02"] = stock["006-D9035-03"]
    part(entries, 2)["sha256"] = sha256(stock["006-D9035-03"])


def corrupt_file(entries, stock):
    stock["006-D9035-01"] = stock["006-D9035-01"][:-1]
    part(entries, 1)["sha256"] = sha256(stock["006-D9035-01"])


STOCK_MISMATCH = {
    "size above the line height": (
        lambda e, s: set_part("size_px", 40, index=3)(e),
        "006-D9035-03: size 40 px does not match the stock font's 35 px line height",
    ),
    "size under half the line height": (
        lambda e, s: set_part("size_px", 17, index=3)(e),
        "006-D9035-03: size 17 px does not match the stock font's 35 px line height",
    ),
    "sizes swapped within the pack": (
        lambda e, s: swap_sizes(e),
        "006-D9035-04 at 27 px has line height 41 px, but 006-D9035-03 at 31 px has 35 px",
    ),
    "wrong version": (
        lambda e, s: set_part("version", "2.20", index=1)(e),
        "006-D9035-01: the stock file's footer has version 2.10, but the registry lists 2.20",
    ),
    "file of another part": (mislabel_file, "006-D9035-02: the stock file's footer names part 006-D9035-03"),
    "hash of another file": (
        lambda e, s: set_part("sha256", part(e, 2)["sha256"], index=1)(e),
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
