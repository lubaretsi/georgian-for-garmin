"""The font registry (`fonts/registry.json`): the Garmin packs that get Georgian, and its checks.

A pack is `bitmap` (2-bit `.fnt` files, one per size) or `truetype` (masked TrueType `.bin`
files, one per weight).
"""

import hashlib
import json
from collections import Counter
from pathlib import Path

from jsonschema import Draft202012Validator
from jsonschema.exceptions import best_match

from .codec import decode, identity
from .truetype import open_host, version_of

SCHEMA_NAME = "registry.schema.json"
# The stock file's extension, by pack kind: Garmin's download names and the build's file names.
SUFFIX = {"bitmap": ".fnt", "truetype": ".bin"}


class RegistryError(ValueError):
    """The registry is malformed, or a file it lists doesn't match its entry."""


def load(path, schema_path=None):
    """Read and validate the registry at `path`; the schema defaults to SCHEMA_NAME beside it."""
    path = Path(path)
    schema_path = Path(schema_path) if schema_path else path.with_name(SCHEMA_NAME)
    registry, schema = (_read_json(p) for p in (path, schema_path))
    validate(registry, schema, source=path)
    return registry


def _read_json(path):
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as error:
        raise RegistryError(f"{path}: invalid JSON: {error}") from None


def validate(registry, schema, source="registry"):
    """Raise RegistryError unless `registry` matches `schema` and is internally consistent."""
    Draft202012Validator.check_schema(schema)
    error = best_match(Draft202012Validator(schema).iter_errors(registry))
    if error is not None:
        where = "/".join(str(key) for key in error.absolute_path) or "(root)"
        raise RegistryError(f"{source}: {where}: {error.message}")

    packs = registry["packs"]
    problems = [f"pack id {i} is listed more than once" for i in _repeated(p["id"] for p in packs)]
    all_parts = (part["part"] for pack in packs for part in pack["parts"])
    problems += [f"part {part} is listed more than once" for part in _repeated(all_parts)]
    for pack in packs:
        name = f"pack {pack['id']}"
        if pack["kind"] == "bitmap":
            for size in _repeated(part["size_px"] for part in pack["parts"]):
                problems.append(f"{name}: size {size} px is listed more than once")
        else:
            for repeated in _repeated(weight(pack, part) for part in pack["parts"]):
                problems.append(f"{name}: weight {repeated} is listed more than once")
        for model_part, software in _repeated((t["model_part"], t["software"]) for t in pack["tested"]):
            problems.append(f"{name}: {model_part} with software {software} is listed more than once")
        known = {part["part"] for part in pack["parts"]}
        for test in pack["tested"]:
            for part in test["parts"]:
                if part not in known:
                    problems.append(
                        f"{name}: tested on {test['model']} ({test['model_part']}) lists part "
                        f"{part}, which is not in this pack"
                    )
    if problems:
        raise RegistryError("\n".join(f"{source}: {problem}" for problem in problems))


def _repeated(values):
    return [value for value, count in Counter(values).items() if count > 1]


def weight(pack, part):
    """The weight Georgian is drawn at for `part` of `pack`: the part's own, else the pack's."""
    return part.get("weight", pack["weight"])


def file_name(pack, part):
    """The stock file's name: `<part>.fnt` or `<part>.bin`."""
    return part["part"] + SUFFIX[pack["kind"]]


def check_file(path, expected_sha256):
    """Return the bytes at `path` after checking them against the registry's SHA-256."""
    data = Path(path).read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected_sha256:
        raise RegistryError(f"{path} has SHA-256 {actual}, but the registry lists {expected_sha256}")
    return data


def check_pack(pack, stock):
    """Check a pack's entries against its stock files (`stock[part number]` is the file's bytes).

    A TrueType pack's files must have the listed SHA-256 and be masked TrueType fonts without
    Georgian (`check_truetype_pack`). A bitmap pack's files must have the listed SHA-256 and
    carry the listed part number and version in their footer. Sizes are checked where the header
    allows: a part's line height must exceed its size and stay within twice it, and line heights
    must not shrink as size grows across the pack (Garmin's 19 and 21 px NotoSans Regular parts
    share a 28 px line).
    """
    if pack["kind"] == "truetype":
        return check_truetype_pack(pack, stock)
    problems = []
    metrics = []
    for part in pack["parts"]:
        number, size = part["part"], part["size_px"]
        data = stock[number]
        actual = hashlib.sha256(data).hexdigest()
        if actual != part["sha256"]:
            problems.append(
                f"{number}: stock file has SHA-256 {actual}, but the registry lists {part['sha256']}"
            )
            continue
        try:
            header, footer, _ = decode(data)
        except ValueError as error:
            problems.append(f"{number}: stock file: {error}")
            continue
        file_part, file_version = identity(footer)
        if file_part != number:
            problems.append(f"{number}: the stock file's footer names part {file_part}")
        if file_version != part["version"]:
            problems.append(
                f"{number}: the stock file's footer has version {file_version}, but the registry "
                f"lists {part['version']}"
            )
        height = header[7]
        if not size < height <= 2 * size:
            problems.append(
                f"{number}: size {size} px does not match the stock font's {height} px line "
                f"height (a size from {(height + 1) // 2} to {height - 1} px would)"
            )
        metrics.append((size, height, number))
    metrics.sort()
    for (size, height, number), (next_size, next_height, next_number) in zip(metrics, metrics[1:]):
        if height > next_height:
            problems.append(
                f"{number} at {size} px has line height {height} px, but {next_number} at "
                f"{next_size} px has {next_height} px; larger sizes need lines at least as tall"
            )
    if problems:
        raise RegistryError("\n".join(f"pack {pack['id']}: {problem}" for problem in problems))


def check_truetype_pack(pack, stock):
    """Check a TrueType pack's entries against its stock files: each must have the listed
    SHA-256, end with Garmin's trailer naming the listed part and version, and unmask to a
    TrueType font with glyf outlines and no Georgian."""
    problems = []
    for part in pack["parts"]:
        number, data = part["part"], stock[part["part"]]
        actual = hashlib.sha256(data).hexdigest()
        if actual != part["sha256"]:
            problems.append(
                f"{number}: stock file has SHA-256 {actual}, but the registry lists {part['sha256']}"
            )
            continue
        try:
            open_host(data)
            file_version, file_part = version_of(data)
        except ValueError as error:
            problems.append(f"{number}: stock file: {error}")
            continue
        if (file_part, file_version) != (number, part["version"]):
            problems.append(
                f"{number}: stock file's trailer names {file_part} version {file_version}, but "
                f"the registry lists version {part['version']}"
            )
    if problems:
        raise RegistryError("\n".join(f"pack {pack['id']}: {problem}" for problem in problems))
