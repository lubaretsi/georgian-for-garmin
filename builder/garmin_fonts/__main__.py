"""Command line: `python -m garmin_fonts fetch|build|render|roundtrip`.

Each command is one declaration (COMMANDS): JSON Schema in and out, both checked on every call,
and its behaviour hints. Flags are the input's names in kebab-case; `--input <file|->` gives
the input as JSON instead (flags override it); `--json` prints the output as one JSON value;
`--help --json` prints the commands as MCP tool definitions. Exit status: 0 success, 2 invalid
input, 1 otherwise; with `--json`, errors go to stderr as {"error": {"code", "message"}}.
"""

import argparse
import json
import os
import sys
from pathlib import Path

from fontTools.ttLib import TTLibError
from jsonschema import Draft202012Validator
from jsonschema.exceptions import best_match

from . import __version__, catalog
from . import registry as font_registry

PROG = "garmin-fonts"
INVALID_INPUT = (
    FileExistsError,
    FileNotFoundError,
    IsADirectoryError,
    NotADirectoryError,
    TTLibError,
    ValueError,
)

STRINGS = {"type": "array", "items": {"type": "string", "minLength": 1}}
OFFLINE = {"type": "boolean", "description": "never download (also GFG_OFFLINE=1)"}
FONT_SUMMARY = {
    "type": "object",
    "required": ["part", "version", "kind", "sha256", "bytes"],
    "properties": {
        "part": {"type": "string"}, "version": {"type": "string"},
        "kind": {"enum": ["bitmap", "truetype"]}, "sha256": {"type": "string"},
        "bytes": {"type": "integer"}, "path": {"type": "string"},
    },
}
TRACE = {
    "type": "object",
    "required": ["char", "code_point", "state", "font", "evidence", "reason", "candidates",
                 "unknown_fonts"],
    "properties": {
        "char": {"type": "string"},
        "code_point": {"type": "string"},
        "state": {"enum": ["confirmed", "coverage_only", "unknown"]},
        "font": {"type": ["string", "null"],
                 "description": "the part drawing it, \"missing\" for the watch's box, or null"},
        "evidence": STRINGS | {"description": "ids in fonts/fallback-models.json"},
        "reason": {"type": "string"},
        "candidates": STRINGS | {"description": "declared parts whose known bytes have it"},
        "unknown_fonts": STRINGS | {"description": "declared parts whose bytes are unavailable"},
    },
}


def run_fetch(args):
    out = Path(args["out"]) if "out" in args else None
    fonts = []
    for part in args["parts"]:
        stock = catalog.fetch_stock(part, offline=args.get("offline"))
        summary = stock.summary()
        if out is not None:
            out.mkdir(parents=True, exist_ok=True)
            path = out / stock.path.name
            path.write_bytes(stock.data)
            summary["path"] = str(path)
        fonts.append(summary)
    return {"fonts": fonts}


def run_build(args):
    from .build import add_georgian, build

    out = Path(args["out"]) if "out" in args else None
    if not args.get("fonts"):
        if out is None:
            raise ValueError("pass --out for the asset directory, or fonts to build")

        def given(key, env):
            value = args.get(key) or os.environ.get(env)
            if not value:
                raise ValueError(f"pass --{key.replace('_', '-')}, or run inside `nix develop` ({env})")
            return value

        return build(
            args.get("registry", font_registry.REGISTRY), given("stock_dir", "GFG_STOCK_DIR"),
            given("noto", "GFG_NOTO_SANS_GEORGIAN"), given("license", "GFG_NOTO_OFL"), out,
            test_marks=args.get("test_marks", False),
        )
    registry = font_registry.load(args["registry"]) if "registry" in args else None
    built = [
        add_georgian(font, args.get("size_like"), args.get("markers"), weight=args.get("weight"),
                     noto=args.get("noto"), offline=args.get("offline"), registry=registry)
        for font in args["fonts"]
    ]
    fonts = []
    if out is not None:
        out.mkdir(parents=True, exist_ok=True)
        taken = [out / font.file_name for font in built if (out / font.file_name).exists()]
        if taken:
            raise FileExistsError(f"{taken[0]} exists")
    for font in built:
        summary = font.summary()
        if out is not None:
            path = out / font.file_name
            path.write_bytes(font.data)
            summary["path"] = str(path)
        fonts.append(summary)
    return {"fonts": fonts}


def run_render(args):
    from .build import add_georgian
    from .render import TEST_TEXT, load_report, render

    report = load_report(args["report"]) if "report" in args else None
    if report is None and not args.get("fonts"):
        raise ValueError("pass the watch's fonts (part numbers or files) or --report")
    fonts = report["declared"] if report else [_declared(f) for f in args["fonts"]]
    replace = list(args.get("replace", []))
    if args.get("with_our_fonts"):
        from .render import _entries, installed_builds

        pins = font_registry.parts(font_registry.default())
        ours = [add_georgian(e["part"], offline=args.get("offline")) for e in _entries(fonts)
                if e["part"] in pins and pins[e["part"]].pack is not None]
        replace += installed_builds(fonts, ours)
    family = args.get("family") or (report["model_part"] if report else None)
    software = args.get("software") or (report["software"] if report else None)
    rendering = render(fonts, args.get("text", TEST_TEXT), replace, family, software=software,
                       offline=args.get("offline"))
    result = rendering.summary()
    if "out" in args:
        rendering.save(args["out"])
        result["png"] = args["out"]
    return result


def _declared(font):
    """A render font argument: a part number, `part@version`, or a font file."""
    if "@" in font:
        part, version = font.split("@", 1)
        return {"part": part, "version": version}
    if catalog.PART_NUMBER.match(font):
        return font
    return Path(font).read_bytes()


def run_roundtrip(args):
    import hashlib

    from .build import font_bytes
    from .codec import compressed, decode, encode, identity

    fonts = []
    for font in args["fonts"]:
        data = font_bytes(font, offline=args.get("offline"))
        header, footer, cmap = decode(data)
        part, version = identity(footer)
        fonts.append({
            "part": part, "version": version, "compressed": compressed(header),
            "glyphs": len(cmap), "roundtrip": encode(header, footer, cmap) == data,
            "sha256": hashlib.sha256(data).hexdigest(),
        })
    return {"fonts": fonts, "ok": all(f["roundtrip"] for f in fonts)}


COMMANDS = {
    "fetch": {
        "title": "Fetch Garmin stock fonts",
        "description": "Fetch Garmin stock fonts by part number: pinned parts from the nix "
        "directories, the cache or their pinned URL (checked against the registry's SHA-256), "
        "others from Garmin's update catalog (checked against its size and MD5).",
        "example": "python -m garmin_fonts fetch 006-D8448-70 006-D8947-22 --out stock --json",
        "positional": "parts",
        "run": run_fetch,
        "hints": {"readOnlyHint": False, "destructiveHint": False, "idempotentHint": True,
                  "openWorldHint": True},
        "input": {
            "type": "object", "required": ["parts"], "additionalProperties": False,
            "properties": {
                "parts": STRINGS | {"minItems": 1, "description": "part numbers, e.g. 006-D8448-70"},
                "out": {"type": "string", "description": "also copy each font into this directory"},
                "offline": OFFLINE,
            },
        },
        "output": {
            "type": "object", "required": ["fonts"],
            "properties": {"fonts": {"type": "array", "items": FONT_SUMMARY}},
        },
    },
    "build": {
        "title": "Add Georgian",
        "description": "Add Georgian to Garmin fonts (part numbers or files) and write each to "
        "--out as <part>.fnt or .bin; without fonts, build the app's font asset directory from "
        "the registry (stock/, georgian/, previews/, LICENSES/OFL.txt, manifest.json) into "
        "--out, which must not exist, and print its manifest.",
        "example": "python -m garmin_fonts build 006-D8947-22 --size-like thai --markers ჰ=გ "
        "ก=ข --out builds --json\n  python -m garmin_fonts build --out assets",
        "positional": "fonts",
        "run": run_build,
        "hints": {"readOnlyHint": False, "destructiveHint": False, "idempotentHint": True,
                  "openWorldHint": True},
        "input": {
            "type": "object", "additionalProperties": False,
            "properties": {
                "fonts": STRINGS | {"description": "part numbers or font files"},
                "out": {"type": "string",
                        "description": "output directory (without it, fonts are only summarized; "
                                       "the asset build needs it)"},
                "size_like": {"type": "string",
                              "description": "host family id whose letters size Georgian "
                              "(default: the part's, from the registry)"},
                "markers": {"type": "object", "additionalProperties": {"type": "string"},
                            "description": "activation markers, glyph=drawn_as"},
                "weight": {"type": "integer", "minimum": 100, "maximum": 900,
                           "description": "Noto Sans Georgian weight (default: the registry's)"},
                "registry": {"type": "string", "description": "registry.json (default: the repo's)"},
                "stock_dir": {"type": "string",
                              "description": "asset build: every pack part as <part>.fnt/.bin "
                              "(default: $GFG_STOCK_DIR)"},
                "noto": {"type": "string",
                         "description": "Noto Sans Georgian TTF (default: $GFG_NOTO_SANS_GEORGIAN)"},
                "license": {"type": "string",
                            "description": "asset build: the font's OFL.txt (default: $GFG_NOTO_OFL)"},
                "test_marks": {"type": "boolean",
                               "description": "asset build: pre-release test marks "
                               "(manifest: test_marks, mark)"},
                "offline": OFFLINE,
            },
        },
        "output": {
            "type": "object", "required": ["fonts"],
            "description": "{fonts: [each build]}; for the asset build, its manifest",
            "properties": {"fonts": {"type": "array"}},
        },
    },
    "render": {
        "title": "Render text as a watch would",
        "description": "Render text from a watch's fonts (part numbers, part@version, font "
        "files, or a report fixture's declared list) with the family's fallback model; print a "
        "per-character trace (confirmed, coverage_only, unknown) and write a PNG.",
        "example": "python -m garmin_fonts render --report "
        "../crates/core/tests/reports/maintainer-venu2-stock-imported.json --with-our-fonts "
        "--out venu2.png --json",
        "positional": "fonts",
        "run": run_render,
        "hints": {"readOnlyHint": False, "destructiveHint": False, "idempotentHint": True,
                  "openWorldHint": True},
        "input": {
            "type": "object", "additionalProperties": False,
            "properties": {
                "fonts": STRINGS | {"description": "the watch's fonts, in its order"},
                "report": {"type": "string", "description": "a report fixture (JSON)"},
                "text": {"type": "string", "description": "default: the app's test notification"},
                "replace": STRINGS | {"description": "font files replacing the stock part they name"},
                "with_our_fonts": {"type": "boolean",
                                   "description": "replace what install would with our builds"},
                "family": {"type": "string",
                           "description": "fallback model id or watch model part "
                           "(default: the report's)"},
                "software": {"type": "string", "description": "watch software (default: the report's)"},
                "out": {"type": "string", "description": "PNG to write"},
                "offline": OFFLINE,
            },
        },
        "output": {
            "type": "object", "required": ["text", "family", "model", "counts", "trace"],
            "properties": {
                "text": {"type": "string"},
                "family": {"type": ["string", "null"]},
                "model": {"type": ["string", "null"]},
                "counts": {"type": "object",
                           "required": ["confirmed_drawn", "confirmed_box", "coverage_only",
                                        "unknown"]},
                "not_installed": {"type": "array"},
                "trace": {"type": "array", "items": TRACE},
                "png": {"type": "string"},
            },
        },
    },
    "roundtrip": {
        "title": "Check the codec's round trip",
        "description": "Decode and re-encode bitmap fonts (part numbers or files), plain or "
        "Compressed; exit 1 unless every one comes back byte for byte.",
        "example": "python -m garmin_fonts roundtrip 006-D8448-70 006-D9035-03 --json",
        "positional": "fonts",
        "run": run_roundtrip,
        "hints": {"readOnlyHint": True, "destructiveHint": False, "idempotentHint": True,
                  "openWorldHint": True},
        "input": {
            "type": "object", "required": ["fonts"], "additionalProperties": False,
            "properties": {
                "fonts": STRINGS | {"minItems": 1, "description": "part numbers or .fnt files"},
                "offline": OFFLINE,
            },
        },
        "output": {
            "type": "object", "required": ["fonts", "ok"],
            "properties": {
                "fonts": {"type": "array", "items": {
                    "type": "object",
                    "required": ["part", "version", "compressed", "glyphs", "roundtrip", "sha256"],
                }},
                "ok": {"type": "boolean"},
            },
        },
    },
}


def tools(names=None):
    """The commands as MCP tool definitions."""
    return {"tools": [
        {"name": name, "title": c["title"], "description": c["description"],
         "inputSchema": c["input"], "outputSchema": c["output"],
         "annotations": {"title": c["title"], **c["hints"]}}
        for name, c in COMMANDS.items() if names is None or name in names
    ]}


class Fail(Exception):
    def __init__(self, status, code, message):
        super().__init__(message)
        self.status, self.code = status, code


def check(schema, value, what):
    error = best_match(Draft202012Validator(schema).iter_errors(value))
    if error is not None:
        where = "/".join(str(k) for k in error.absolute_path) or "(root)"
        raise Fail(2 if what == "input" else 1, "invalid_input" if what == "input" else "internal",
                   f"{what}: {where}: {error.message}")


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise Fail(2, "invalid_input", f"{message}\n{self.format_usage()}".strip())


def parser():
    top = Parser(
        prog=PROG,
        description="Garmin font toolkit: fetch stock fonts, add Georgian, render text as a "
        "watch would, check the codec. `--help --json` prints the commands as MCP tools.",
        epilog="examples:\n" + "\n".join(f"  {c['example']}" for c in COMMANDS.values()),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    top.add_argument("--version", action="version", version=f"{PROG} {__version__}")
    sub = top.add_subparsers(dest="command", metavar="{" + ",".join(COMMANDS) + "}")
    for name, command in COMMANDS.items():
        p = sub.add_parser(
            name, help=command["title"], description=command["description"],
            epilog=f"example:\n  {command['example']}",
            formatter_class=argparse.RawDescriptionHelpFormatter, argument_default=argparse.SUPPRESS,
        )
        p.error = Parser.error.__get__(p)
        for key, spec in command["input"]["properties"].items():
            help_text = spec.get("description")
            if key == command["positional"]:
                p.add_argument(key, nargs="*", help=help_text)
            elif spec["type"] == "boolean":
                p.add_argument(f"--{key.replace('_', '-')}", dest=key, action="store_true",
                               help=help_text)
            elif spec["type"] == "array" or spec["type"] == "object":
                p.add_argument(f"--{key.replace('_', '-')}", dest=key, nargs="+", help=help_text)
            elif spec["type"] == "integer":
                p.add_argument(f"--{key.replace('_', '-')}", dest=key, type=int, help=help_text)
            else:
                p.add_argument(f"--{key.replace('_', '-')}", dest=key, help=help_text)
        p.add_argument("--input", dest="_input", metavar="FILE|-",
                       help="the input as JSON, keyed as in its schema")
        p.add_argument("--json", dest="_json", action="store_true",
                       help="print the output as one JSON value")
        p.add_argument("--no-input", dest="_no_input", action="store_true",
                       help="never prompt (no command prompts)")
    return top


def _pairs(values):
    out = {}
    for value in values:
        glyph, sep, drawn_as = value.partition("=")
        if not sep:
            raise Fail(2, "invalid_input", f"--markers takes glyph=drawn_as, not {value!r}")
        out[glyph] = drawn_as
    return out


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    as_json = "--json" in argv
    try:
        if "--json" in argv and ("--help" in argv or "-h" in argv):
            names = [a for a in argv if a in COMMANDS] or None
            print(json.dumps(tools(names), ensure_ascii=False))
            return
        namespace = vars(parser().parse_args(argv))
        name = namespace.pop("command", None)
        if name is None:
            raise Fail(2, "invalid_input", f"name a command: {', '.join(COMMANDS)} (see --help)")
        command = COMMANDS[name]
        source = namespace.pop("_input", None)
        namespace.pop("_json", None)
        namespace.pop("_no_input", None)
        args = {}
        if source is not None:
            text = sys.stdin.read() if source == "-" else Path(source).read_text()
            try:
                args = json.loads(text)
            except json.JSONDecodeError as error:
                raise Fail(2, "invalid_input", f"--input: invalid JSON: {error}") from None
        if command["positional"] in namespace and not namespace[command["positional"]]:
            namespace.pop(command["positional"])
        if "markers" in namespace:
            namespace["markers"] = _pairs(namespace["markers"])
        args |= namespace
        check(command["input"], args, "input")
        result = command["run"](args)
        check(command["output"], result, "output")
    except Fail as error:
        fail(as_json, error.status, error.code, str(error))
    except catalog.CatalogError as error:
        fail(as_json, 2 if error.code == "invalid_input" else 1, error.code, str(error))
    except INVALID_INPUT as error:
        fail(as_json, 2, "invalid_input", str(error))
    except Exception as error:  # noqa: BLE001 - report every other failure as exit 1
        fail(as_json, 1, "internal", f"{type(error).__name__}: {error}")
    if as_json:
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    else:
        print(_human(name, args, result))
    if name == "roundtrip" and not result["ok"]:
        sys.exit(1)


def _human(name, args, result):
    if name == "build" and not args.get("fonts"):
        return args["out"]
    if name == "render":
        counts = ", ".join(f"{state} {n}" for state, n in result["counts"].items())
        lines = [f"{result['model'] or 'no model'}: {counts}"]
        lines += [f"  {t['char']} {t['code_point']} {t['state']:<13} {t['font'] or '-':<13} "
                  f"{t['reason']}" for t in result["trace"]]
        if "png" in result:
            lines.append(result["png"])
        return "\n".join(lines)
    if name == "roundtrip":
        return "\n".join(f"{f['part']} {f['version']} "
                         f"{'compressed' if f['compressed'] else 'plain'} "
                         f"{'ok' if f['roundtrip'] else 'DIFFERS'}" for f in result["fonts"])
    return "\n".join((f.get("path") or f"{f['part']} {f['sha256']}")
                     + ("" if f.get("rasterizer_pinned", True) else " (unpinned rasterizer)")
                     for f in result["fonts"])


def fail(as_json, status, code, message):
    if as_json:
        print(json.dumps({"error": {"code": code, "message": message}}, ensure_ascii=False),
              file=sys.stderr)
    else:
        print(f"{PROG}: error: {message}", file=sys.stderr)
    sys.exit(status)


if __name__ == "__main__":
    main()
