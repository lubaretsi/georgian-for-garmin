"""The font registry (`fonts/registry.json`): host families, the Garmin packs that get Georgian,
other pinned stock fonts, the support list, and its checks.

A pack is `bitmap` (2-bit `.fnt` files, one per size) or `truetype` (masked TrueType `.bin`
files, one per weight) and belongs to a host family (`hosts`), which says what its Georgian is
sized by. `stock` pins other Garmin bitmap fonts for experiments and the codec's tests; they are
never built into the app's fonts.
"""

import hashlib
import json
from collections import Counter
from dataclasses import dataclass
from functools import cache
from pathlib import Path

from jsonschema import Draft202012Validator
from jsonschema.exceptions import best_match

from .codec import decode, identity
from .truetype import open_host, version_of

SCHEMA_NAME = "registry.schema.json"
# The repository's registry, beside the builder.
REGISTRY = Path(__file__).resolve().parents[2] / "fonts" / "registry.json"
# The stock file's extension, by pack kind: Garmin's download names and the build's file names.
SUFFIX = {"bitmap": ".fnt", "truetype": ".bin"}
# The host family each prediction family names: a predicted model is expected to get Georgian
# in that host's installable packs.
PREDICTION_HOSTS = {
    "noto_truetype": "noto-armenian-truetype",
    "bitmap_armenian": "armenian",
    "bitmap_korean": "korean",
}


class RegistryError(ValueError):
    """The registry is malformed, or a file it lists doesn't match its entry."""


def load(path, schema_path=None):
    """Read and validate the registry at `path`; the schema defaults to SCHEMA_NAME beside it."""
    path = Path(path)
    schema_path = Path(schema_path) if schema_path else path.with_name(SCHEMA_NAME)
    registry, schema = (_read_json(p) for p in (path, schema_path))
    validate(registry, schema, source=path)
    return registry


@cache
def default():
    """The repository's registry (REGISTRY), loaded and validated once."""
    return load(REGISTRY)


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
    watches = {watch["model_part"]: watch for watch in registry["watches"]}
    problems = [f"pack id {i} is listed more than once" for i in _repeated(p["id"] for p in packs)]
    hosts = {host["id"]: host for host in registry["hosts"]}
    problems += [
        f"host id {i} is listed more than once" for i in _repeated(h["id"] for h in registry["hosts"])
    ]
    all_parts = [part["part"] for pack in packs for part in pack["parts"]]
    all_parts += [part["part"] for part in registry["stock"]]
    problems += [f"part {part} is listed more than once" for part in _repeated(all_parts)]
    for entry, kind in [(p, p["kind"]) for p in packs] + [(s, "bitmap") for s in registry["stock"]]:
        name = f"pack {entry['id']}" if "id" in entry else f"stock part {entry['part']}"
        host = hosts.get(entry["host"])
        if host is None:
            problems.append(f"{name}: host {entry['host']} is not in hosts")
        elif host["kind"] != kind:
            problems.append(f"{name}: host {entry['host']} is a {host['kind']} host, not {kind}")
    # A font family reaches users only once a watch drew Georgian from it: a pack of that host
    # has a tested entry, or the host cites a lab observation (`confirmed_by`). Every other
    # pack is restore-only.
    confirmed = {p["host"] for p in packs if p["tested"]}
    confirmed |= {h["id"] for h in registry["hosts"] if h.get("confirmed_by")}
    for pack in packs:
        if pack.get("install", True) and pack["host"] not in confirmed:
            problems.append(
                f"pack {pack['id']}: host {pack['host']} has no tested pack and no confirmed_by; "
                "set install to false until a watch confirms it"
            )
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
            watch = watches.get(test["model_part"])
            if watch is None:
                problems.append(
                    f"{name}: tested on {test['model_part']}, which is not in watches"
                )
            elif "unsupported" in watch:
                problems.append(
                    f"{name}: tested on {watch['model']} ({test['model_part']}), which watches "
                    "lists as unsupported"
                )
            for part in test["parts"]:
                if part not in known:
                    problems.append(
                        f"{name}: tested on {test['model_part']} lists part "
                        f"{part}, which is not in this pack"
                    )
    for model_part in _repeated(watch["model_part"] for watch in registry["watches"]):
        problems.append(f"watches: {model_part} is listed more than once")
    for repeated in _repeated(entry["id"] for entry in registry["reasons"]):
        problems.append(f"reasons: {repeated} is listed more than once")
    reasons = {r["id"] for r in registry["reasons"]}
    used_reasons = set()
    tested = {test["model_part"] for pack in packs for test in pack["tested"]}
    tested_models = {w["model"] for w in registry["watches"] if w["model_part"] in tested}
    for watch in registry["watches"]:
        name = f"watches: {watch['model']} ({watch['model_part']})"
        if "unsupported" in watch:
            if not watch["issues"]:
                problems.append(f"{name} is unsupported but names no issue to follow")
            reason = watch["unsupported"]["reason"]
            used_reasons.add(reason)
            if reason not in reasons:
                problems.append(f"{name} names reason {reason}, which reasons lacks")
        if "prediction" in watch:
            if "unsupported" in watch:
                problems.append(f"{name} is predicted to work but marked unsupported")
            if watch["model_part"] in tested:
                problems.append(f"{name} is tested; drop its prediction")
            source = watch["prediction"]["source"]
            family = source.removeprefix("firmware family of ")
            if family != source and family not in tested_models:
                problems.append(f"{name}: prediction source names {family}, not a tested model")
            host = PREDICTION_HOSTS[watch["prediction"]["family"]]
            if not any(p["host"] == host and p.get("install", True) for p in packs):
                problems.append(
                    f"{name}: prediction family {watch['prediction']['family']} names host "
                    f"{host}, which has no installable pack"
                )
    for unused in sorted(reasons - used_reasons):
        problems.append(f"reasons: {unused} is not used")
    if problems:
        raise RegistryError("\n".join(f"{source}: {problem}" for problem in problems))


def resolved_watches(registry):
    """The support list with each unsupported reason as `{en, ka}`, as the asset manifest
    carries it."""
    reasons = {r["id"]: {"en": r["en"], "ka": r["ka"]} for r in registry["reasons"]}
    out = []
    for watch in registry["watches"]:
        entry = dict(watch)
        if "unsupported" in watch:
            entry["unsupported"] = dict(
                watch["unsupported"], reason=reasons[watch["unsupported"]["reason"]]
            )
        out.append(entry)
    return out


def _repeated(values):
    return [value for value, count in Counter(values).items() if count > 1]


@dataclass(frozen=True)
class Part:
    """A stock part the registry pins: from a pack (`pack` is its id) or from `stock` (None)."""

    part: str
    version: str
    url: str
    sha256: str
    kind: str
    host: str
    weight: int
    size_px: int | None
    name: str
    pack: str | None

    @property
    def file_name(self):
        """`<part>.fnt` or `<part>.bin`, as Garmin names the download."""
        return self.part + SUFFIX[self.kind]


def parts(registry):
    """{part number: Part} for every part the registry pins, packs first, in registry order."""
    out = {}
    for pack in registry["packs"]:
        for part in pack["parts"]:
            out[part["part"]] = Part(
                part["part"], part["version"], part["url"], part["sha256"], pack["kind"],
                pack["host"], weight(pack, part), part.get("size_px"),
                f"{pack['name']}{' %d px' % part['size_px'] if 'size_px' in part else ''}",
                pack["id"],
            )
    for part in registry["stock"]:
        out[part["part"]] = Part(
            part["part"], part["version"], part["url"], part["sha256"], "bitmap", part["host"],
            part["weight"], part["size_px"], part["name"], None,
        )
    return out


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
