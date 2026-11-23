"""The README's watch tables, generated from the registry's support list (`watches`) and the
packs' `tested` entries, so the README and the list can't disagree: the known watches, and the
ones Garmin's data predicts to work (`prediction`).

`python -m garmin_fonts.support` (from `builder/`) rewrites both tables in the repository's
README.md between their markers; the builder tests fail when they differ from the registry.
"""

import argparse
import json
import sys
from pathlib import Path

from . import __version__
from . import registry as font_registry

PROG = "garmin-fonts-support"
REGISTRY = font_registry.REGISTRY
README = REGISTRY.parents[1] / "README.md"
ISSUES = "https://github.com/lubaretsi/georgian-for-garmin/issues"
BEGIN = (
    "<!-- watches: generated from fonts/registry.json by `python -m garmin_fonts.support` "
    "(from builder/); edit the registry, not this table -->"
)
END = "<!-- /watches -->"
PREDICTED_BEGIN = (
    "<!-- predicted watches: generated from fonts/registry.json by `python -m garmin_fonts.support` "
    "(from builder/); edit the registry, not this table -->"
)
PREDICTED_END = "<!-- /predicted watches -->"
FIRMWARE_FAMILY = "firmware family of "
EMAILED_REPORT = "a watch report emailed to the maintainer"
# What each prediction family's fonts are called where a watch's own report is the source.
REPORTED_FONTS = {
    "noto_truetype": "Noto Sans Armenian",
    "bitmap_armenian": "Bitmap Armenian fonts",
    "bitmap_korean": "Korean bitmap fonts",
}


def based_on_text(prediction):
    """The README's "Based on" cell for a prediction, from its source: a Connect IQ device page,
    a tested model's firmware family, or the watch's own report (an issue, or by email)."""
    source = prediction["source"]
    if source.startswith(FIRMWARE_FAMILY):
        return f"Firmware family of the tested {source.removeprefix(FIRMWARE_FAMILY)}"
    fonts = REPORTED_FONTS[prediction["family"]]
    if source.startswith(f"{ISSUES}/"):
        issue = source.removeprefix(f"{ISSUES}/")
        return f"{fonts} in its watch report ([#{issue}]({source}))"
    if source == EMAILED_REPORT:
        return f"{fonts} in its watch report, emailed to the maintainer"
    return f"[Connect IQ device reference]({source})"


def issue_links(issues):
    """`([#4](…), [#5](…))`, or "" without issues."""
    if not issues:
        return ""
    return " (" + ", ".join(f"[#{n}]({ISSUES}/{n})" for n in issues) + ")"


def rows(registry):
    """(watch, software, status) per model in the support list but not predicted, in its order.

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
    for watch in (w for w in registry["watches"] if "prediction" not in w):
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


def predicted_rows(registry):
    """(family, watch, based on) per predicted model name, grouped by model family (the name's
    first word, in list order); `family` is set on each group's first row only. Parts that share
    a Connect IQ device page's name share a row."""
    groups = {}
    for watch in registry["watches"]:
        if "prediction" not in watch:
            continue
        based_on = based_on_text(watch["prediction"])
        group = groups.setdefault(watch["model"].split()[0], [])
        if (watch["model"], based_on) not in group:
            group.append((watch["model"], based_on))
    return [
        (family if i == 0 else "", model, based_on)
        for family, group in groups.items()
        for i, (model, based_on) in enumerate(group)
    ]


def table(registry):
    """The known watches' README section, markers included."""
    lines = [BEGIN, "| Watch | Software | Status |", "|---|---|---|"]
    lines += [f"| {watch} | {software} | {status} |" for watch, software, status in rows(registry)]
    lines.append(END)
    return "\n".join(lines)


def predicted_table(registry):
    """The predicted watches' README section, markers included."""
    lines = [PREDICTED_BEGIN, "| Family | Watch | Based on |", "|---|---|---|"]
    lines += [f"| {family} | {watch} | {based} |" for family, watch, based in predicted_rows(registry)]
    lines.append(PREDICTED_END)
    return "\n".join(lines)


def section(readme_text, begin=BEGIN, end=END):
    """The generated section of `readme_text` between `begin` and `end`, markers included."""
    start = readme_text.find(begin)
    stop = readme_text.find(end, start)
    if start < 0 or stop < 0:
        raise ValueError(f"the README has no watch table between {begin!r} and {end!r}")
    return readme_text[start:stop + len(end)]


def update(readme_text, registry):
    """`readme_text` with both watch tables regenerated."""
    text = readme_text.replace(section(readme_text), table(registry))
    predicted = section(text, PREDICTED_BEGIN, PREDICTED_END)
    return text.replace(predicted, predicted_table(registry))


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
        description="Rewrite the README's watch tables from the registry's support list.",
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
