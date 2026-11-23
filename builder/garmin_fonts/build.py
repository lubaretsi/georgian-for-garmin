"""Add Georgian to one Garmin font (`add_georgian`), and build the font asset directory: Garmin
stock fonts, Georgian builds and a manifest (`build`)."""

import hashlib
import io
import json
import warnings
from dataclasses import dataclass, field
from functools import cache
from pathlib import Path

from . import catalog
from . import hosts as host_families
from . import marks as test_mark
from . import registry as font_registry
from . import truetype
from .codec import decode, identity
from .georgian import Outlines, Sizing, choose_sizing, rasterizer, with_georgian
from .preview import bitmap_mark_picture, truetype_mark_picture, truetype_preview

# The size the installer stages by default: verified on hardware (Venu 2, firmware 19.05).
DEFAULT_PART = "006-D9035-03"


class UnpinnedRasterizerWarning(UserWarning):
    """Georgian was drawn outside the rasterizer environment the pinned builds come from, so
    its bytes may differ from a nix build's for the same inputs."""


@dataclass(frozen=True)
class Built:
    """A font with Georgian added: its bytes and how it was made.

    `sizing` is the georgian.Sizing (bitmap) or truetype.TrueTypeSizing it was drawn at;
    `clipped` maps "U+XXXX" to the faint pixels dropped outside a bitmap row; `markers` are the
    activation markers applied ({glyph: drawn_as}). `stock_sha256` is the input font's SHA-256
    and `stock_pinned` whether it is the file the registry pins for the part (Garmin's
    original at the pinned version), as opposed to other bytes given for it. `rasterizer` is
    the environment that drew it (georgian.rasterizer) and `rasterizer_pinned` whether it is the
    registry's `georgian_font.rasterizer`, the one the pinned builds come from (`nix develop`).
    """

    part: str
    version: str
    kind: str
    host: str
    weight: int
    sizing: object
    clipped: dict
    markers: dict
    stock_sha256: str
    stock_pinned: bool
    rasterizer: dict
    rasterizer_pinned: bool
    data: bytes = field(repr=False)

    @property
    def sha256(self):
        return hashlib.sha256(self.data).hexdigest()

    @property
    def file_name(self):
        """`<part>.fnt` or `<part>.bin`; a bitmap font is staged on the watch as `<part>.tmp`."""
        return self.part + font_registry.SUFFIX[self.kind]

    def summary(self):
        """What `build` reports for it, without the bytes."""
        if self.kind == "bitmap":
            sizing = {"georgian_size_px": self.sizing.size,
                      "georgian_descender_percent": self.sizing.descender}
        else:
            sizing = {"georgian_scale": str(self.sizing.scale),
                      "georgian_scale_basis": self.sizing.basis,
                      "georgian_descender_percent": self.sizing.descender}
        return {"part": self.part, "version": self.version, "kind": self.kind, "host": self.host,
                "weight": self.weight, **sizing, "clipped": self.clipped,
                "markers": self.markers, "stock_sha256": self.stock_sha256,
                "stock_pinned": self.stock_pinned, "rasterizer": self.rasterizer,
                "rasterizer_pinned": self.rasterizer_pinned, "bytes": len(self.data),
                "sha256": self.sha256}


@cache
def outlines_at(noto, weight):
    """Noto Sans Georgian at `noto` (a path) at `weight`, loaded once per session."""
    return Outlines(str(noto), weight)


def font_bytes(font, *, offline=None, registry=None):
    """The bytes of `font`: bytes as they are, a catalog.Stock or Built's bytes, a part number
    (fetched with catalog.fetch_stock), or a path to a file."""
    if isinstance(font, (bytes, bytearray)):
        return bytes(font)
    if isinstance(font, (catalog.Stock, Built)):
        return font.data
    if isinstance(font, str) and catalog.PART_NUMBER.match(font):
        return catalog.fetch_stock(font, offline=offline, registry=registry).data
    return Path(font).read_bytes()


def identify(data):
    """(kind, part, version) of a Garmin font's bytes: a 2-bit `.fnt` (bitmap) or a masked
    TrueType `.bin`. Raises ValueError when it is neither."""
    try:
        _, footer, _ = decode(data)
    except ValueError as bitmap_error:
        try:
            version, part = truetype.version_of(data)
        except ValueError:
            raise ValueError(f"not a Garmin font: {bitmap_error}") from None
        return "truetype", part, version
    part, version = identity(footer)
    return "bitmap", part, version


def add_georgian(font, size_like=None, markers=None, *, weight=None, nominal=None, mark=None,
                 noto=None, offline=None, registry=None):
    """`font` with Georgian (modern Mkhedruli and Mtavruli) added, as a Built.

    `font` is a Garmin bitmap `.fnt` or masked TrueType `.bin`: bytes, a path, a catalog.Stock,
    or a part number (fetched with catalog.fetch_stock). Every stock glyph, the header and the
    footer or trailer are kept; the same inputs always give the same bytes.

    `size_like` says what Georgian is sized by: a host family (hosts.Host or its id, e.g.
    "thai"), whose letters it is measured against (georgian.choose_sizing,
    truetype.choose_sizing); or an exact georgian.Sizing / truetype.TrueTypeSizing. By default
    it is the host family the registry gives the part. `markers` ({glyph: drawn_as}) are
    activation markers for test builds, applied after Georgian is drawn, e.g. {"ჰ": "გ", "ก":
    "ข"}; a host's own is `hosts.get("thai").markers`. `weight` (Noto Sans Georgian's wght) and
    `nominal` (the part's nominal px, which bounds the size search) default to the registry's
    entry for the part; for an unpinned font fetched by part number (or a catalog.Stock) the
    weight comes from its catalog name when that names one weight (Bold 700, Medium 500, …),
    and an unpinned bitmap font's nominal defaults to its line height. `mark` is a beta test
    mark (marks.Mark). `noto` defaults to the pinned Noto Sans Georgian (catalog.georgian_font).

    Outside the registry's pinned rasterizer environment the result is marked
    `rasterizer_pinned=False` and an UnpinnedRasterizerWarning is issued: the same inputs may
    give other bytes there (`nix develop`, or the `.venv` from `nix run .#omp-venv`, is pinned).

    Raises ValueError when Georgian doesn't fit or an input is missing, catalog.CatalogError
    when the font can't be fetched.
    """
    registry = font_registry.default() if registry is None else registry
    if isinstance(font, str) and catalog.PART_NUMBER.match(font):
        font = catalog.fetch_stock(font, offline=offline, registry=registry)
    data = font_bytes(font, offline=offline, registry=registry)
    kind, part, version = identify(data)
    pin = font_registry.parts(registry).get(part)
    if size_like is None:
        if pin is None:
            raise ValueError(f"{part} is not in the registry: pass size_like (a host id or sizing)")
        size_like = pin.host
    if isinstance(size_like, str):
        size_like = host_families.get(size_like, registry)
    if weight is None:
        if pin is not None:
            weight = pin.weight
        elif isinstance(font, catalog.Stock) and catalog.weight_of(font.name):
            weight = catalog.weight_of(font.name)
        else:
            named = f" (catalog name {font.name!r})" if isinstance(font, catalog.Stock) else ""
            raise ValueError(f"{part} is not in the registry and its weight isn't known{named}: "
                             "pass weight (400 Regular, 500 Medium, 700 Bold)")
    if isinstance(size_like, host_families.Host):
        if size_like.kind != kind:
            raise ValueError(f"host {size_like.id} is a {size_like.kind} host; {part} is {kind}")
        host = size_like.id
    else:
        host = pin.host if pin is not None else "custom"
    outlines = outlines_at(noto or catalog.georgian_font(offline=offline, registry=registry),
                           weight)
    markers = dict(markers or {})
    if kind == "truetype":
        if isinstance(size_like, host_families.Host):
            sizing = truetype.choose_sizing(data, outlines, size_like)
        elif isinstance(size_like, truetype.TrueTypeSizing):
            sizing = size_like
        else:
            raise ValueError(f"size_like for a TrueType font: a host or TrueTypeSizing, not {size_like!r}")
        out = truetype.with_georgian(data, outlines, sizing, mark, markers)
        clipped = {}
    else:
        if isinstance(size_like, host_families.Host):
            if nominal is None:
                nominal = pin.size_px if pin is not None and pin.size_px else decode(data)[0][7]
            sizing = choose_sizing(data, outlines, nominal, size_like)
        elif isinstance(size_like, Sizing):
            sizing = size_like
        else:
            raise ValueError(f"size_like for a bitmap font: a host or Sizing, not {size_like!r}")
        out, lost = with_georgian(data, outlines, sizing, mark, markers)
        clipped = {f"U+{cp:04X}": pixels for cp, pixels in lost.items()}
    stock_sha256 = hashlib.sha256(data).hexdigest()
    environment = rasterizer()
    pinned_environment = environment == registry["georgian_font"]["rasterizer"]
    if not pinned_environment:
        warnings.warn(
            f"{part}: Georgian drawn with {environment}, not the pinned "
            f"{registry['georgian_font']['rasterizer']}; its bytes may differ from a nix build's "
            "(use nix develop, or nix run .#omp-venv for omp's eval)",
            UnpinnedRasterizerWarning, stacklevel=2,
        )
    return Built(part, version, kind, host, weight, sizing, clipped, markers, stock_sha256,
                 pin is not None and stock_sha256 == pin.sha256, environment, pinned_environment,
                 out)


def build(registry, stock_dir, noto, license_file, out, test_marks=False):
    """Write the asset directory to `out`, which must not exist yet; return the manifest.

    `registry` is the font registry JSON (its schema beside it); `stock_dir` holds every part
    its packs list, as `<part>.fnt` (bitmap packs) or `<part>.bin` (TrueType packs); `noto` is
    the Noto Sans Georgian TTF and `license_file` its OFL text, both matching the registry's
    `georgian_font` hashes. Every file is checked before anything is written. Each part's
    Georgian is `add_georgian` with its pack's host family, weight and size. Each TrueType
    build also gets a preview, `previews/<part>.png`.

    With `test_marks` (pre-release builds), each part's Georgian marks one capital of the test
    text with a bar (`marks`), and gets a picture of the marked capitals,
    `previews/mark-<part>.png`, described by its manifest entry's `mark`. Without, the fonts
    are the ones tested on watches.
    """
    stock_dir, out = Path(stock_dir), Path(out)
    entries = font_registry.load(registry)
    packs, source = entries["packs"], entries["georgian_font"]
    families = host_families.hosts(entries)
    marks = test_mark.assign(packs) if test_marks else {}
    font_registry.check_file(noto, source["sha256"])
    license_text = font_registry.check_file(license_file, source["license"]["sha256"])
    stock = {
        part["part"]: (stock_dir / font_registry.file_name(pack, part)).read_bytes()
        for pack in packs
        for part in pack["parts"]
    }
    for pack in packs:
        font_registry.check_pack(pack, stock)
    built = []
    for pack in packs:
        for part in pack["parts"]:
            georgian = add_georgian(
                stock[part["part"]], families[pack["host"]], weight=font_registry.weight(pack, part),
                nominal=part.get("size_px"), mark=marks.get(part["part"]), noto=noto,
                registry=entries,
            )
            if pack["kind"] == "truetype":
                details = truetype_entry(georgian.sizing)
            else:
                details = bitmap_entry(part, georgian.sizing, georgian.clipped)
            built.append((pack, part, georgian.data, details))

    out.mkdir(parents=True, exist_ok=False)
    for sub in ("stock", "georgian", "previews", "LICENSES"):
        (out / sub).mkdir()
    (out / "LICENSES" / "OFL.txt").write_bytes(license_text)
    fonts = []
    for pack, part, georgian, details in built:
        number = part["part"]
        entry = {"part": number, "pack": pack["id"], "version": part["version"], **details}
        # Georgian builds earlier releases installed, which the installer recognizes as its own.
        entry["earlier_georgian"] = part.get("earlier_georgian", [])
        name = font_registry.file_name(pack, part)
        for kind, data in (("stock", stock[number]), ("georgian", georgian)):
            path = f"{kind}/{name}"
            (out / path).write_bytes(data)
            entry[kind] = path
            entry[f"{kind}_sha256"] = hashlib.sha256(data).hexdigest()
        if pack["kind"] == "truetype":
            entry["preview"] = f"previews/{number}.png"
            label = f"{number} {pack['name']}, weight {font_registry.weight(pack, part)}, with Georgian"
            image = truetype_preview(truetype.unmask(georgian), label)
            image.save(out / entry["preview"])
        if number in marks:
            if pack["kind"] == "truetype":
                picture = truetype_mark_picture(truetype.unmask(georgian))
            else:
                picture = bitmap_mark_picture(georgian)
            buffer = io.BytesIO()
            picture.save(buffer, format="PNG")
            path = f"previews/mark-{number}.png"
            (out / path).write_bytes(buffer.getvalue())
            entry["mark"] = test_mark.manifest_entry(
                marks[number], path, hashlib.sha256(buffer.getvalue()).hexdigest()
            )
        fonts.append(entry)
    manifest = {
        "test_marks": bool(test_marks),
        "fonts": fonts,
        "packs": [
            {
                "id": pack["id"],
                "name": pack["name"],
                "kind": pack["kind"],
                "weight": pack["weight"],
                # Whether the installer writes this pack's Georgian; false packs are
                # restore-only. Every build, pre-release or stable, installs the same packs.
                "install": pack.get("install", True),
                "parts": [part["part"] for part in pack["parts"]],
                "tested": pack["tested"],
            }
            for pack in packs
        ],
        # The support list, for the installer's per-watch status, with each reason written out.
        "watches": font_registry.resolved_watches(entries),
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def bitmap_entry(part, sizing, clipped):
    """Manifest fields of a bitmap build."""
    return {
        "size_px": part["size_px"],
        "default": part["part"] == DEFAULT_PART,
        # The font size Georgian is drawn at, and its descenders' length in percent.
        "georgian_size_px": sizing.size,
        "georgian_descender_percent": sizing.descender,
        # Faint Georgian pixels dropped outside the row: {"U+10DA": 1}.
        "clipped": clipped,
    }


def truetype_entry(sizing):
    """Manifest fields of a TrueType build."""
    return {
        "default": False,
        # Host font units per Noto unit, as an exact fraction ("79/39"), what it was matched to,
        # and the descenders' length in percent.
        "georgian_scale": str(sizing.scale),
        "georgian_scale_basis": sizing.basis,
        "georgian_descender_percent": sizing.descender,
    }
