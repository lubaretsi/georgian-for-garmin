"""Command line: build the Georgian-extended Garmin font asset directory."""

import argparse
import json
import sys

from fontTools.ttLib import TTLibError

from . import __version__
from .build import build

PROG = "garmin-fonts"
INVALID_INPUT = (
    FileExistsError,
    FileNotFoundError,
    IsADirectoryError,
    NotADirectoryError,
    TTLibError,
    ValueError,
)


class Parser(argparse.ArgumentParser):
    def error(self, message):
        fail(self.json, 2, "invalid_input", message, self.format_usage())


def fail(as_json, status, code, message, usage=""):
    if as_json:
        print(json.dumps({"error": {"code": code, "message": message}}), file=sys.stderr)
    else:
        print(f"{usage}{PROG}: error: {message}", file=sys.stderr)
    sys.exit(status)


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    parser = Parser(
        prog=PROG,
        description="Add Georgian glyphs to every Garmin stock ExtData font in the font registry "
        "and write the asset directory (stock/, georgian/, previews/, LICENSES/OFL.txt, "
        "manifest.json).",
        epilog="example:\n  python -m garmin_fonts --registry ../fonts/registry.json --stock-dir "
        "stock --noto NotoSansGeorgian.ttf --license OFL.txt --out assets --json",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.json = "--json" in argv
    parser.add_argument("--version", action="version", version=f"{PROG} {__version__}")
    parser.add_argument(
        "--registry", required=True, help="registry.json, with registry.schema.json beside it"
    )
    parser.add_argument(
        "--stock-dir", required=True,
        help="directory with every registry part as <part>.fnt (bitmap) or <part>.bin (TrueType)",
    )
    parser.add_argument("--noto", required=True, help="Noto Sans Georgian TTF")
    parser.add_argument("--license", required=True, help="the font's OFL.txt")
    parser.add_argument("--out", required=True, help="output directory; must not exist")
    parser.add_argument("--json", action="store_true", help="print the manifest as JSON")
    args = parser.parse_args(argv)
    try:
        manifest = build(args.registry, args.stock_dir, args.noto, args.license, args.out)
    except INVALID_INPUT as error:
        fail(args.json, 2, "invalid_input", str(error))
    except Exception as error:  # noqa: BLE001 - report every other failure as exit 1
        fail(args.json, 1, "internal", f"{type(error).__name__}: {error}")
    if args.json:
        print(json.dumps(manifest, sort_keys=True))
    else:
        print(args.out)


if __name__ == "__main__":
    main()
