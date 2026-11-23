"""Garmin's stock fonts, from Garmin's part-driven update catalog, pinned by the registry.

`fetch_stock(part)` returns a stock font's bytes. A part the registry pins (a pack's part or a
`stock` entry) is looked for, in order, in the directories `nix develop` and the flake checks
provide (`GFG_STOCK_DIR`, `GFG_CATALOG_DIR`: fixed-output fetches of every pinned URL), then in
the cache (`~/.cache/georgian-garmin/catalog/<part>/<version>/`), then downloaded from its pinned
URL; whatever its source, its SHA-256 must match the pin. A part the registry doesn't pin is
asked of Garmin's catalog (what Garmin Express asks for a watch's files): the catalog names its
product, release version, download URL, size and MD5, and the download must match those; the
result says `pinned: false`.

Offline (`offline=True`, or `GFG_OFFLINE=1` as in CI) nothing is downloaded: a part that isn't in
those directories or the cache fails with CatalogError. Garmin's catalog only serves each part's
latest release, so pinned parts are fetched by their pinned URL, never through the catalog; the
fetch fails clearly when Garmin removes that URL.
"""

import hashlib
import json
import os
import re
import tempfile
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

from . import registry as font_registry
from .codec import decode
from .truetype import version_of

ENDPOINT = "https://omt.garmin.com/api/device-software/v1/updates"
# The catalog request: a made-up device asking for one ExtData file (the form recorded in the
# lab record 2026-11-20-watch-font-survey, catalog-probes.json).
DEVICE = (
    '<Device xmlns="http://www.garmin.com/xmlschemas/GarminDevice/v2"><Model>'
    "<PartNumber>006-B0000-00</PartNumber><SoftwareVersion>0</SoftwareVersion>"
    "<Description>Research lookup</Description></Model><Id>1</Id>"
    "<MassStorageMode>{files}</MassStorageMode></Device>"
)
UPDATE_FILE = (
    "<UpdateFile><PartNumber>{part}</PartNumber><Version><Major>0</Major><Minor>0</Minor>"
    "</Version><Path>Garmin/ExtData</Path><FileName>{part}{suffix}</FileName></UpdateFile>"
)
PART_NUMBER = re.compile(r"^[0-9]{3}-[A-Z][0-9]{4}-[0-9]{2}$")
# Directories of pinned files that `nix develop` and the flake checks set.
PINNED_DIRS = ("GFG_STOCK_DIR", "GFG_CATALOG_DIR")
TIMEOUT = 120


class CatalogError(Exception):
    """A stock font can't be had: offline and not at hand, not in Garmin's catalog, or a download
    that doesn't match its pin or the catalog. `code` is `invalid_input` for a malformed part
    number, else `font_unavailable`."""

    def __init__(self, message, code="font_unavailable"):
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class Stock:
    """A stock font: its bytes, what it is, and where they came from (`nix`, `cache`,
    `download` or `catalog`). `pinned` says whether the registry pins it."""

    part: str
    version: str
    kind: str
    name: str
    sha256: str
    pinned: bool
    source: str
    path: Path
    data: bytes = field(repr=False)

    def summary(self):
        """The fields `fetch` reports, without the bytes."""
        return {
            "part": self.part, "version": self.version, "kind": self.kind, "name": self.name,
            "sha256": self.sha256, "bytes": len(self.data), "pinned": self.pinned,
            "source": self.source, "path": str(self.path),
        }


def cache_dir():
    """The download cache: `$GFG_CATALOG_CACHE`, else `$XDG_CACHE_HOME/georgian-garmin/catalog`,
    else `~/.cache/georgian-garmin/catalog`."""
    if os.environ.get("GFG_CATALOG_CACHE"):
        return Path(os.environ["GFG_CATALOG_CACHE"])
    base = os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache"
    return Path(base) / "georgian-garmin" / "catalog"


def is_offline(offline=None):
    """`offline` when given, else whether `GFG_OFFLINE` is set to 1."""
    return os.environ.get("GFG_OFFLINE") == "1" if offline is None else offline


def _open(request, timeout=TIMEOUT):
    """The network, in one place (tests replace it)."""
    return urllib.request.urlopen(request, timeout=timeout)


def _sha256(data):
    return hashlib.sha256(data).hexdigest()


def _store(path, data):
    """Write `data` to `path` atomically, so a concurrent or interrupted fetch never leaves half
    a file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
        handle.write(data)
    os.replace(handle.name, path)


def catalog_entry(part, kind="bitmap"):
    """Garmin's catalog entry for `part` (its `updates` item: productName, version,
    deliverable.secureDownloadUrl, sizeInBytes, md5). Raises CatalogError when the catalog
    doesn't know it."""
    suffix = font_registry.SUFFIX[kind]
    body = json.dumps({
        "IsUserInteractive": False,
        "GarminDeviceXml": DEVICE.format(files=UPDATE_FILE.format(part=part, suffix=suffix)),
    }).encode()
    request = urllib.request.Request(
        ENDPOINT, body, {"Content-Type": "application/json", "Accept": "application/json"}
    )
    try:
        with _open(request, timeout=60) as response:
            updates = json.load(response)["updates"]
    except (OSError, ValueError, KeyError) as error:
        raise CatalogError(f"{part}: Garmin's catalog didn't answer: {error}") from None
    matches = [u for u in updates if u.get("partNumber") == part]
    if len(matches) != 1:
        raise CatalogError(f"{part}: Garmin's catalog returned {len(matches)} entries for it")
    return matches[0]


def _download(url):
    try:
        with _open(url) as response:
            return response.read()
    except OSError as error:
        raise CatalogError(f"{url}: download failed: {error}") from None


def fetch_stock(part, *, offline=None, registry=None, kind=None):
    """The Stock font for Garmin part `part` (e.g. "006-D8448-70").

    `registry` defaults to the repository's. A Stock's `kind` ("bitmap" for `.fnt`, "truetype"
    for `.bin`) is the registry's for a pinned part and read from the bytes for any other; a
    `kind` passed here must agree with it, else CatalogError (`invalid_input`). See the module
    for where the bytes come from. Raises CatalogError.
    """
    if not PART_NUMBER.match(part):
        raise CatalogError(f"{part!r} is not a Garmin part number like 006-D8448-70", "invalid_input")
    if kind is not None and kind not in font_registry.SUFFIX:
        raise CatalogError(f"kind is bitmap or truetype, not {kind!r}", "invalid_input")
    registry = font_registry.default() if registry is None else registry
    pin = font_registry.parts(registry).get(part)
    if pin is not None:
        if kind is not None and kind != pin.kind:
            raise CatalogError(f"{part} is a {pin.kind} part, not {kind}", "invalid_input")
        return _fetch_pinned(pin, is_offline(offline))
    stock = _fetch_unpinned(part, is_offline(offline))
    if kind is not None and kind != stock.kind:
        raise CatalogError(f"{part} is a {stock.kind} font, not {kind}", "invalid_input")
    return stock


def kind_of(data):
    """"bitmap" or "truetype": which kind of Garmin font `data` is. Raises CatalogError when it
    is neither."""
    try:
        decode(data)
        return "bitmap"
    except ValueError as bitmap_error:
        try:
            version_of(data)
            return "truetype"
        except ValueError:
            raise CatalogError(f"not a Garmin font: {bitmap_error}") from None


# Weight words in Garmin's catalog names, as Noto Sans Georgian's wght values.
WEIGHTS = {"thin": 100, "extralight": 200, "light": 300, "regular": 400, "medium": 500,
           "semibold": 600, "bold": 700, "extrabold": 800, "black": 900}


def weight_of(name):
    """The weight a catalog product name gives ("APAC Font, Nanum Gothic Bold 27px, KOR": 700),
    or None when it names none or more than one."""
    found = {WEIGHTS[word] for word in re.findall(r"[a-z]+", name.lower()) if word in WEIGHTS}
    return found.pop() if len(found) == 1 else None


def _fetch_pinned(pin, offline):
    cached = cache_dir() / pin.part / pin.version / pin.file_name
    candidates = [
        (Path(os.environ[var]) / pin.file_name, "nix") for var in PINNED_DIRS if os.environ.get(var)
    ] + [(cached, "cache")]
    for path, source in candidates:
        if path.is_file():
            data = path.read_bytes()
            if _sha256(data) == pin.sha256:
                return _stock(pin, data, True, source, path)
    if offline:
        raise CatalogError(
            f"{pin.part} {pin.version} is not at hand offline: run inside `nix develop` "
            "(GFG_STOCK_DIR, GFG_CATALOG_DIR) or fetch it online first"
        )
    data = _download(pin.url)
    if _sha256(data) != pin.sha256:
        raise CatalogError(
            f"{pin.url} has SHA-256 {_sha256(data)}, but the registry pins {pin.sha256}"
        )
    _store(cached, data)
    return _stock(pin, data, True, "download", cached)


def _stock(pin, data, pinned, source, path):
    return Stock(pin.part, pin.version, pin.kind, pin.name, _sha256(data), pinned, source,
                 Path(path), data)


def _matches(data, deliverable):
    """Whether `data` has the size and MD5 Garmin's catalog gives for the file."""
    return (len(data) == deliverable["sizeInBytes"]
            and hashlib.md5(data).hexdigest() == deliverable["md5"])


def _fetch_unpinned(part, offline):
    folder = cache_dir() / part
    names = [f"{part}{suffix}" for suffix in font_registry.SUFFIX.values()]
    if offline:
        # The newest release fetched before, checked against the catalog entry saved beside it.
        found = sorted((p for name in names for p in folder.glob(f"*/{name}")),
                       key=lambda p: _version_key(p.parent.name))
        if not found:
            raise CatalogError(
                f"{part} is not pinned in the registry and was never fetched; fetch it online first"
            )
        path = found[-1]
        try:
            entry = json.loads((path.parent / "catalog.json").read_text())
            deliverable = entry["deliverable"]
            version = entry["version"]
        except (OSError, ValueError, KeyError) as error:
            raise CatalogError(f"{path}: no readable catalog entry beside it ({error}); "
                               "fetch it online again") from None
        data = path.read_bytes()
        if version != path.parent.name or not _matches(data, deliverable):
            raise CatalogError(f"{path} doesn't match the size and MD5 Garmin's catalog gave for "
                               f"{part} {version}; fetch it online again")
        return Stock(part, version, kind_of(data), entry["productName"], _sha256(data), False,
                     "cache", path, data)
    entry = catalog_entry(part)
    deliverable = entry["deliverable"]
    folder = folder / entry["version"]
    cached = [folder / name for name in names if (folder / name).is_file()]
    data, source = next(((p.read_bytes(), "cache") for p in cached
                         if _matches(p.read_bytes(), deliverable)), (None, "catalog"))
    if data is None:
        data = _download(deliverable["secureDownloadUrl"])
        if not _matches(data, deliverable):
            raise CatalogError(f"{part}: the download doesn't match the catalog's size and MD5")
    kind = kind_of(data)
    path = folder / f"{part}{font_registry.SUFFIX[kind]}"
    if source == "catalog" or not path.is_file():
        _store(path, data)
    for stale in cached:
        if stale != path:
            stale.unlink()
    _store(folder / "catalog.json", json.dumps(entry, indent=2).encode())
    return Stock(part, entry["version"], kind, entry["productName"], _sha256(data), False,
                 source, path, data)


def _version_key(version):
    try:
        major, minor = version.split(".")
        return int(major), int(minor)
    except ValueError:
        return (-1, -1)


def georgian_font(*, offline=None, registry=None):
    """The path of Noto Sans Georgian, as the registry pins it: `GFG_NOTO_SANS_GEORGIAN` (set by
    `nix develop`), else the cache, else downloaded. Raises CatalogError."""
    registry = font_registry.default() if registry is None else registry
    source = registry["georgian_font"]
    path = os.environ.get("GFG_NOTO_SANS_GEORGIAN")
    if path and Path(path).is_file() and _sha256(Path(path).read_bytes()) == source["sha256"]:
        return Path(path)
    cached = cache_dir() / "georgian-font" / f"{source['sha256']}.ttf"
    if cached.is_file() and _sha256(cached.read_bytes()) == source["sha256"]:
        return cached
    if is_offline(offline):
        raise CatalogError("Noto Sans Georgian is not at hand offline: run inside `nix develop`")
    data = _download(source["url"])
    if _sha256(data) != source["sha256"]:
        raise CatalogError(f"{source['url']} doesn't match the registry's SHA-256")
    _store(cached, data)
    return cached
