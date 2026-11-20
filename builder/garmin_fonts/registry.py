"""The font registry (`fonts/registry.json`): the Garmin packs that get Georgian, and its checks."""

import hashlib
import json
from collections import Counter
from pathlib import Path

from jsonschema import Draft202012Validator
from jsonschema.exceptions import best_match

from .codec import decode, identity

SCHEMA_NAME = "registry.schema.json"


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
        for size in _repeated(part["size_px"] for part in pack["parts"]):
            problems.append(f"{name}: size {size} px is listed more than once")
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


def check_file(path, expected_sha256):
    """Return the bytes at `path` after checking them against the registry's SHA-256."""
    data = Path(path).read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected_sha256:
        raise RegistryError(f"{path} has SHA-256 {actual}, but the registry lists {expected_sha256}")
    return data


def check_pack(pack, stock):
    """Check a pack's entries against its stock files (`stock[part number]` is the file's bytes).

    Each file must have the listed SHA-256 and carry the listed part number and version in its
    footer. Sizes are checked where the header allows: a part's line height must exceed its
    size and stay within twice it, and line heights must grow with size across the pack.
    """
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
        if height >= next_height:
            problems.append(
                f"{number} at {size} px has line height {height} px, but {next_number} at "
                f"{next_size} px has {next_height} px; larger sizes need taller lines"
            )
    if problems:
        raise RegistryError("\n".join(f"pack {pack['id']}: {problem}" for problem in problems))
