"""Render text as a watch would draw it from its fonts, with a per-character trace of how much of
that is known.

`render(fonts, text, replace=[…], family=…)` takes the fonts a watch declares (part numbers, as
a watch report lists them), the text, fonts that replace some of them (our builds), and the
watch's family (a fallback model id or the watch's model part). It returns a Rendering: a PNG
image and a trace with one entry per character, in one of three states:

- `confirmed`: every input is known and the fallback step that picks the font is backed by a
  recorded observation for that family, cited by id (`evidence`);
- `coverage_only`: the glyph exists in a font the watch has, but the family's order isn't
  confirmed for it;
- `unknown`: font bytes unavailable (a part the registry doesn't pin, another version, the
  watch's built-in font), or no model for the family.

The models are data, `fonts/fallback-models.json`, each step citing observations. A report
fixture gives the declared files, not the renderer's order, so it never confirms an order on
its own. A font the call doesn't replace counts as Garmin's original.
"""

import io
import json
from collections import Counter
from dataclasses import dataclass, field
from functools import cache
from pathlib import Path

from jsonschema import Draft202012Validator
from jsonschema.exceptions import best_match

from . import catalog, truetype
from . import registry as font_registry
from .build import Built, font_bytes, identify
from .codec import decode
from .georgian import rows

MODELS = font_registry.REGISTRY.with_name("fallback-models.json")
REPORTS = font_registry.REGISTRY.parents[1] / "crates" / "core" / "tests" / "reports"
# The app's test notification.
TEST_TEXT = "Test მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ Test"
STATES = ("confirmed", "coverage_only", "unknown")
# Pixel size TrueType fonts are drawn at; no TrueType watch has a model to say otherwise.
TRUETYPE_PX = 24
SCALE = 2
GREYS = (0, 85, 170, 255)
STATE_COLOURS = {"confirmed": (0, 170, 0), "coverage_only": (230, 180, 0), "unknown": (200, 0, 0)}


class ModelError(ValueError):
    """The fallback models are malformed or cite evidence they don't list."""


def load_models(path=MODELS):
    """The fallback models at `path`, checked against the schema beside it and for citations of
    evidence it doesn't list."""
    path = Path(path)
    data = json.loads(path.read_text())
    schema = json.loads(path.with_name("fallback-models.schema.json").read_text())
    error = best_match(Draft202012Validator(schema).iter_errors(data))
    if error is not None:
        where = "/".join(str(key) for key in error.absolute_path) or "(root)"
        raise ModelError(f"{path}: {where}: {error.message}")
    known = {e["id"] for e in data["evidence"]}
    for model in data["models"]:
        cited = [*model["system_lacks"]["evidence"], *model["missing"]["evidence"],
                 *model["stock_without_effect"]["evidence"]]
        for group in (model["order"], model["unused"], model["caveats"]):
            cited += [i for entry in group for i in entry["evidence"]]
        missing = sorted(set(cited) - known)
        if missing:
            raise ModelError(f"{path}: model {model['id']} cites unknown evidence {missing}")
    return data


@cache
def _default_models():
    return load_models()


def find_model(family, software=None, models=None):
    """The model for `family` (a model id or a watch model part) and, when given, `software`;
    None when there is none."""
    models = _default_models() if models is None else models
    for model in models["models"]:
        if family == model["id"] or family in model["model_parts"]:
            if software is None or software in model["software"]:
                return model
    return None


@dataclass
class Font:
    """One declared font as the render sees it: its part and declared version, its decoded
    glyphs when its bytes are known (`cmap`: code point to a bitmap tile, or True for a TrueType
    glyph), where they came from (`stock`, `replace`) and why not when they aren't (`why`)."""

    part: str
    version: str | None = None
    kind: str | None = None
    origin: str | None = None
    why: str | None = None
    height: int = 0
    baseline: int = 0
    cmap: dict | None = None
    plain: bytes | None = field(default=None, repr=False)

    @property
    def known(self):
        return self.cmap is not None

    def maps(self, cp):
        return self.known and cp in self.cmap


@cache
def _decoded(data):
    kind, part, version = identify(data)
    if kind == "bitmap":
        header, _, cmap = decode(data)
        return kind, part, version, header[7], header[8], cmap, None
    plain = truetype.unmask(data)
    from fontTools.ttLib import TTFont

    cmap = dict.fromkeys(TTFont(io.BytesIO(plain)).getBestCmap(), True)
    return kind, part, version, TRUETYPE_PX + TRUETYPE_PX // 3, TRUETYPE_PX, cmap, plain


def _load(entry, replaced, offline, registry):
    """The Font for a declared entry {"part", "version"?}."""
    part, version = entry["part"], entry.get("version")
    if part in replaced:
        data, origin = replaced[part], "replace"
    else:
        pin = font_registry.parts(registry).get(part)
        if pin is None:
            return Font(part, version, why="not pinned in the registry")
        if version is not None and version != pin.version:
            return Font(part, version, kind=pin.kind,
                        why=f"the watch declares version {version}, the registry pins {pin.version}")
        try:
            data = catalog.fetch_stock(part, offline=offline, registry=registry).data
        except catalog.CatalogError as error:
            return Font(part, version, kind=pin.kind, why=str(error))
        origin = "stock"
    kind, _, _, height, baseline, cmap, plain = _decoded(data)
    return Font(part, version, kind, origin, None, height, baseline, cmap, plain)


def _entries(fonts):
    """Declared entries {"part", "version"?} from part numbers, (part, version) pairs, dicts or
    font bytes (identified by their footer or trailer)."""
    out = []
    for font in fonts:
        if isinstance(font, str):
            out.append({"part": font})
        elif isinstance(font, dict):
            out.append({k: font[k] for k in ("part", "version") if k in font})
        elif isinstance(font, tuple):
            out.append({"part": font[0], "version": font[1]})
        else:
            _, part, version = identify(font_bytes(font))
            out.append({"part": part, "version": version})
    return out


@dataclass
class Rendering:
    """The result of `render`: the text, the model used, a trace entry per character and the
    image."""

    text: str
    family: str | None
    model: str | None
    trace: list
    image: object = field(repr=False)

    def counts(self):
        """{state: characters} for every state."""
        counts = Counter(entry["state"] for entry in self.trace)
        return {state: counts.get(state, 0) for state in STATES}

    def png(self):
        buffer = io.BytesIO()
        self.image.save(buffer, format="PNG")
        return buffer.getvalue()

    def save(self, path):
        Path(path).write_bytes(self.png())

    def summary(self):
        """The JSON view: text, family, model, counts and trace."""
        return {"text": self.text, "family": self.family, "model": self.model,
                "counts": self.counts(), "trace": self.trace}


def _in_ranges(cp, ranges):
    return any(int(lo[2:], 16) <= cp <= int(hi[2:], 16) for lo, hi in ranges)


def _trace(char, fonts, model, family):
    """The trace entry for `char` over the declared `fonts` (in declared order) under `model`."""
    cp = ord(char)
    entry = {"char": char, "code_point": f"U+{cp:04X}"}
    having = [f.part for f in fonts if f.maps(cp)]
    unknown = [f.part for f in fonts if not f.known]
    if model is None:
        return entry | {
            "state": "unknown", "font": having[0] if having else None, "evidence": [],
            "reason": f"no fallback model for {family}" if family else "no family given",
            "candidates": having, "unknown_fonts": unknown,
        }
    if not _in_ranges(cp, model["system_lacks"]["ranges"]):
        return entry | {
            "state": "unknown", "font": having[0] if having else None, "evidence": [],
            "reason": "the watch's built-in font may draw it, and its bytes are unavailable",
            "candidates": having, "unknown_fonts": unknown,
        }
    evidence = list(model["system_lacks"]["evidence"])
    unused = {p for group in model["unused"] for p in group["parts"]}
    without_effect = {(f["part"], f["version"]) for f in model["stock_without_effect"]["fonts"]}
    considered = []
    for font in fonts:
        if font.part in unused:
            evidence += [i for g in model["unused"] if font.part in g["parts"] for i in g["evidence"]]
        elif (font.part, font.version) in without_effect and font.origin != "replace":
            evidence += model["stock_without_effect"]["evidence"]
        else:
            considered.append(font)
    by_part = {f.part: f for f in considered}
    hit, blocked = None, None
    for step in model["order"]:
        font = by_part.get(step["part"])
        if font is None:
            continue
        if not font.known:
            blocked = font
            break
        evidence += step["evidence"]
        if font.maps(cp):
            hit = font
            break
    ordered = {step["part"] for step in model["order"]}
    others = [f for f in considered if f.part not in ordered and (not f.known or f.maps(cp))]
    others_unknown = [f.part for f in others if not f.known]
    others_having = [f.part for f in others if f.known]
    common = {"candidates": having, "unknown_fonts": unknown}
    if blocked is not None:
        return entry | common | {
            "state": "unknown", "font": having[0] if having else None, "evidence": [],
            "reason": f"{blocked.part} comes first in the model, and its bytes are unavailable "
                      f"({blocked.why})",
        }
    if others_unknown:
        return entry | common | {
            "state": "unknown", "font": hit.part if hit else (having[0] if having else None),
            "evidence": [], "reason": f"fonts whose bytes are unavailable may have it: "
                                      f"{', '.join(others_unknown)}",
        }
    if others_having:
        return entry | common | {
            "state": "coverage_only", "font": hit.part if hit else others_having[0],
            "evidence": [], "reason": f"the model doesn't place {', '.join(others_having)}, "
                                      "which also has it",
        }
    if hit is not None:
        return entry | common | {
            "state": "confirmed", "font": hit.part, "evidence": _unique(evidence),
            "reason": f"{hit.part} is the first font in the model's order that has it",
        }
    return entry | common | {
        "state": "confirmed", "font": "missing",
        "evidence": _unique(evidence + model["missing"]["evidence"]),
        "reason": "no font the view takes it from has it: the watch draws a box",
    }


def _unique(ids):
    return list(dict.fromkeys(ids))


def render(fonts, text=TEST_TEXT, replace=(), family=None, *, software=None, label=None,
           offline=None, registry=None, models=None):
    """Render `text` from the watch fonts `fonts`, as a Rendering.

    `fonts` lists the fonts the watch declares, in its order: part numbers, (part, version)
    pairs, {"part", "version"} dicts (a report's `declared`), or font bytes. Bytes come from
    `replace` (fonts identified by their footer or trailer: bytes, paths, Built results), else
    from catalog.fetch_stock for parts the registry pins at the declared version. `family` is a
    fallback model id or the watch's model part (006-B3703-00); with `software`, a model applies
    only to the versions it was observed on. `label` heads the image. Offline by default
    unless `offline=False`.
    """
    registry = font_registry.default() if registry is None else registry
    offline = True if offline is None else offline
    replaced = {}
    for font in replace:
        data = font_bytes(font, offline=offline, registry=registry)
        replaced[identify(data)[1]] = data
    declared = _entries(fonts)
    loaded = [_load(entry, replaced, offline, registry) for entry in declared]
    model = find_model(family, software, models) if family else None
    trace = [_trace(char, loaded, model, family) for char in text]
    image = _image(text, trace, {f.part: f for f in loaded}, label or _label(family, model, software))
    return Rendering(text, family, model["id"] if model else None, trace, image)


def _label(family, model, software):
    if model is not None:
        return f"{model['watch']} {software or ''} · model {model['id']}".strip()
    return f"{family or 'no family'} · no fallback model"


def _tile_levels(font, char):
    """Rows of 2-bit levels for `char` from `font`, with the row's baseline."""
    if font.kind == "bitmap":
        return rows(font.cmap[ord(char)], font.height), font.baseline
    from PIL import Image, ImageDraw, ImageFont

    pillow_font = ImageFont.truetype(io.BytesIO(font.plain), TRUETYPE_PX)
    width = max(1, round(pillow_font.getlength(char)))
    image = Image.new("L", (width, font.height))
    ImageDraw.Draw(image).text((0, font.baseline), char, font=pillow_font, anchor="ls", fill=255)
    data = image.point(lambda p: (p + 42) // 85).tobytes()
    return [list(data[y * width:(y + 1) * width]) for y in range(font.height)], font.baseline


def _image(text, trace, fonts, label):
    """White-on-black glyphs on a common baseline, each underlined in its state's colour: a
    font's own tile, a hollow box for the watch's missing glyph, a grey block where nothing is
    known to draw it."""
    from PIL import Image, ImageDraw, ImageFont

    known = [f for f in fonts.values() if f.known]
    ascent = max([f.baseline for f in known] + [16])
    descent = max([f.height - f.baseline for f in known] + [6])
    cells = []
    for char, item in zip(text, trace):
        font = fonts.get(item["font"])
        if char == " ":
            cells.append(("space", (ascent + descent) // 4, None))
        elif font is not None and font.maps(ord(char)):
            levels, baseline = _tile_levels(font, char)
            cells.append(("glyph", len(levels[0]) if levels else 0, (levels, baseline)))
        elif item["font"] == "missing":
            cells.append(("box", max(4, ascent // 2), None))
        else:
            cells.append(("none", max(4, ascent // 2), None))
    width = sum(w for _, w, _ in cells) + 8
    line = Image.new("L", (width, ascent + descent))
    draw = ImageDraw.Draw(line)
    x = 4
    marks = []
    for (kind, w, payload), item in zip(cells, trace):
        if kind == "glyph":
            levels, baseline = payload
            top = ascent - baseline
            for y, row in enumerate(levels):
                for col, level in enumerate(row):
                    if level and 0 <= top + y < line.height:
                        line.putpixel((x + col, top + y), GREYS[level])
        elif kind == "box":
            draw.rectangle((x + 1, ascent // 4, x + w - 2, ascent - 1), outline=GREYS[3])
        elif kind == "none":
            draw.rectangle((x + 1, ascent // 4, x + w - 2, ascent - 1), fill=GREYS[1])
        marks.append((x, w, item["state"]))
        x += w
    label_font = ImageFont.load_default(size=14)
    image = Image.new("RGB", (max(line.width * SCALE, 420), 26 + line.height * SCALE + 10))
    canvas = ImageDraw.Draw(image)
    canvas.text((6, 5), label, font=label_font, fill=(150, 150, 150))
    image.paste(line.resize((line.width * SCALE, line.height * SCALE), Image.NEAREST).convert("RGB"),
                (0, 26))
    bar = 26 + line.height * SCALE + 3
    for x, w, state in marks:
        canvas.rectangle((x * SCALE, bar, (x + w) * SCALE - 2, bar + 4), fill=STATE_COLOURS[state])
    return image


def installed_builds(declared, builds, registry=None):
    """The fonts of `builds` (Built or bytes, ours) that install puts on a watch declaring
    `declared` parts: the core's rule, bitmap packs when the watch declares any bitmap part of
    them, else TrueType packs, only packs with `install` true."""
    registry = font_registry.default() if registry is None else registry
    parts = {e["part"] for e in _entries(declared)}
    installable = {
        part["part"]: pack["kind"] for pack in registry["packs"] if pack.get("install", True)
        for part in pack["parts"]
    }
    listed = {p: installable[p] for p in parts if p in installable}
    kind = "bitmap" if "bitmap" in listed.values() else "truetype"
    out = []
    for build in builds:
        data = build.data if isinstance(build, Built) else font_bytes(build)
        part = identify(data)[1]
        if listed.get(part) == kind:
            out.append(data)
    return out


def load_report(report):
    """A report fixture (path or dict): its JSON."""
    return report if isinstance(report, dict) else json.loads(Path(report).read_text())


def render_report(report, text=TEST_TEXT, builds=(), *, offline=None, registry=None,
                  models=None):
    """`render` over a report fixture's declared fonts with its model part and software, the
    installed ones of `builds` replacing Garmin's (`installed_builds`); labelled with the
    fixture's watch and whether builds were installed."""
    data = load_report(report)
    replace = installed_builds(data["declared"], builds, registry)
    name = Path(report).stem if not isinstance(report, dict) else data.get("model", "report")
    label = (f"{name}: {data['model']} {data['software']} · "
             f"{'with our fonts' if replace else 'Garmin fonts only'}")
    return render(data["declared"], text, replace, data["model_part"],
                  software=data["software"], label=label, offline=offline, registry=registry,
                  models=models)
