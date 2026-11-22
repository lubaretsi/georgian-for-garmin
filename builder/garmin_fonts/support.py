"""The README's watch table, generated from the registry's support list (`watches`) and the
packs' `tested` entries, so the README and the list can't disagree.

`python -m garmin_fonts.support` (from `builder/`) rewrites the table in the repository's
README.md between its markers; the builder tests fail when it differs from the registry.
"""

import argparse
import json
import sys
from pathlib import Path

from . import __version__
from . import registry as font_registry

PROG = "garmin-fonts-support"
REGISTRY = Path(__file__).resolve().parents[2] / "fonts" / "registry.json"
README = REGISTRY.parents[1] / "README.md"
ISSUES = "https://github.com/lubaretsi/georgian-for-garmin/issues"
BEGIN = (
    "<!-- watches: generated from fonts/registry.json by `python -m garmin_fonts.support` "
    "(from builder/); edit the registry, not this table -->"
)
END = "<!-- /watches -->"


def issue_links(issues):
    """`([#4](…), [#5](…))`, or "" without issues."""
    if not issues:
        return ""
    return " (" + ", ".join(f"[#{n}]({ISSUES}/{n})" for n in issues) + ")"


def rows(registry):
    """(watch, software, status) per model in the support list, in its order.

    A model with tested entries is Tested on those software versions. An unsupported one says
    why; every other model is not confirmed yet. On a connected watch the app derives the status
    from the fonts the watch lists too.
    """
    tested = {}
    for pack in registry["packs"]:
        for test in pack["tested"]:
            tested.setdefault(test["model_part"], set()).add(test["software"])
    reasons = {r["id"]: r["en"] for r in registry["reasons"]}
    out = []
    for watch in registry["watches"]:
        links = issue_links(watch["issues"])
        if watch["model_part"] in tested:
            versions = sorted(tested[watch["model_part"]], key=version_key)
            out.append((watch["model"], ", ".join(versions), f"Tested{links}"))
        elif "unsupported" in watch:
            unsupported = watch["unsupported"]
            status = f"Doesn't work{links}. {reasons[unsupported['reason']]}"
            out.append((watch["model"], ", ".join(unsupported["software"]), status))
        else:
            out.append((watch["model"], "–", f"Not confirmed yet{links}"))
    return out


def version_key(version):
    major, minor = version.split(".")
    return int(major), int(minor)


def table(registry):
    """The README section, markers included."""
    lines = [BEGIN, "| Watch | Software | Status |", "|---|---|---|"]
    lines += [f"| {watch} | {software} | {status} |" for watch, software, status in rows(registry)]
    lines.append(END)
    return "\n".join(lines)


def section(readme_text):
    """The generated section of `readme_text`, markers included."""
    start = readme_text.find(BEGIN)
    end = readme_text.find(END, start)
    if start < 0 or end < 0:
        raise ValueError(f"the README has no watch table between {BEGIN!r} and {END!r}")
    return readme_text[start:end + len(END)]


def update(readme_text, registry):
    """`readme_text` with its watch table regenerated."""
    return readme_text.replace(section(readme_text), table(registry))


def fail(as_json, message):
    if as_json:
        print(json.dumps({"error": {"code": "invalid_input", "message": message}}), file=sys.stderr)
    else:
        print(f"{PROG}: error: {message}", file=sys.stderr)
    sys.exit(2)


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    parser = argparse.ArgumentParser(
        prog=PROG,
        description="Rewrite the README's watch table from the registry's support list.",
        epilog="example:\n  python -m garmin_fonts.support",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--version", action="version", version=f"{PROG} {__version__}")
    parser.add_argument("--registry", default=REGISTRY, help="registry.json (default: the repo's)")
    parser.add_argument("--readme", default=README, help="README.md (default: the repo's)")
    parser.add_argument("--json", action="store_true", help="print {\"readme\": path} as JSON")
    args = parser.parse_args(argv)
    try:
        registry = font_registry.load(args.registry)
        path = Path(args.readme)
        path.write_text(update(path.read_text(), registry))
    except (OSError, ValueError) as error:
        fail(args.json, str(error))
    print(json.dumps({"readme": str(args.readme)}) if args.json else args.readme)


if __name__ == "__main__":
    main()
