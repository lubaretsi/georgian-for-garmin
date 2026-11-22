#!/usr/bin/env python3
"""Turn a watch report's text into a replay fixture for crates/core/tests/replay.rs.

Reads a report from a file or stdin: the compact text the app shares (0.3.x: `app …`,
`watch …`, `parts: …`, `declared: …`, `extdata: …`, `hidden: …`) or the Markdown of 0.1.x and
0.2.x (`- Model: …` lines and a `| Part number | Version | File name |` table), anywhere in an
issue body or comment. Writes one JSON fixture to stdout or --out.

Only allowlisted fields are copied: model, part number, software version, the app version, the
connection, the `Garmin/ExtData` files the watch declares, the ExtData listing (name, size,
SHA-256 and what the report says it holds) and the hidden listing. Free text, answers and every
other declared file are dropped, so a unit ID or serial number can't reach a fixture; the script
also refuses any value that looks like one (nine or more digits in a row outside a SHA-256).

Example:
  gh api repos/lubaretsi/georgian-for-garmin/issues/20 --jq .body \\
    | scripts/report_fixture.py --source https://github.com/lubaretsi/georgian-for-garmin/issues/20 \\
      --out crates/core/tests/reports/issue-20.json
"""

import argparse
import json
import re
import sys

EXTDATA = "garmin/extdata"
FILE_STATES = {"none", "georgian", "georgian_beta", "stock", "other", "not_visible", "unknown"}
ID_LIKE = re.compile(r"\d{9,}")


class ReportError(ValueError):
    pass


def columns(line):
    """Space-separated columns; a column quoted as JSON (the report quotes anything with spaces
    or non-ASCII) is decoded."""
    out, i, decoder = [], 0, json.JSONDecoder()
    while i < len(line):
        if line[i] == " ":
            i += 1
        elif line[i] == '"':
            value, i = decoder.raw_decode(line, i)
            out.append(value)
        else:
            j = line.find(" ", i)
            j = len(line) if j < 0 else j
            out.append(line[i:j])
            i = j
    return [None if c == "-" else c for c in out]


def parse_compact(lines):
    fixture = {"app_version": None, "connection": "mtp", "extdata": None, "hidden": None}
    declared, extdata, parts = [], None, {}
    section = None
    for line in lines:
        head = line.split(" ", 1)[0]
        if head == "app":
            fixture["app_version"] = columns(line)[1]
            section = None
        elif head == "connection":
            fixture["connection"] = columns(line)[1]
            section = None
        elif head == "watch":
            _, model, part, software = columns(line)[:4]
            fixture.update(model=model, model_part=part, software=software)
            section = None
        elif line.startswith("parts"):
            section = "parts"
        elif line.startswith("declared:"):
            section = "declared"
        elif line == "extdata not found":
            extdata, section = "not_found", None
        elif line.startswith("extdata:"):
            extdata, section = [], "extdata"
        elif line.startswith("hidden:"):
            fixture["hidden"] = parse_hidden(line[len("hidden:"):].strip())
            section = None
        elif section == "parts":
            cols = columns(line)
            if len(cols) >= 2 and cols[1] in FILE_STATES:
                parts[cols[0]] = cols[1]
        elif section == "declared":
            cols = columns(line)
            if len(cols) >= 3 and cols[0] and cols[0].lower() == EXTDATA:
                declared.append(
                    {"path": "Garmin/ExtData", "file": cols[1], "part": cols[2],
                     "version": cols[3] if len(cols) > 3 else None}
                )
        elif section == "extdata":
            cols = columns(line)
            if len(cols) >= 2 and cols[1] and cols[1].isdigit():
                part = cols[0].rsplit(".", 1)[0]
                state = parts.get(part)
                extdata.append(
                    {"name": cols[0], "bytes": int(cols[1]),
                     "sha256": cols[2] if len(cols) > 2 else None,
                     # What the report says the part's file holds; only for files named for a
                     # part of that build.
                     "file": state if state not in (None, "none", "not_visible") else None}
                )
    fixture["declared"] = declared
    fixture["extdata"] = extdata
    return fixture


def parse_hidden(text):
    if text == "unsupported":
        return {"status": "unsupported", "files": []}
    if text == "not read":
        return {"status": "not_read", "files": []}
    if text.startswith("error"):
        return {"status": "error", "files": []}
    files, status = [], "listed"
    for entry in (e.strip() for e in text.split(";")):
        if not entry or entry == "none":
            continue
        if entry.startswith("error"):
            status = "error"
            continue
        cols = columns(entry)
        files.append(
            {"name": cols[0], "bytes": int(cols[1]),
             "read_only": {"ro": True, "rw": False}.get(cols[2]),
             "file": cols[3] if len(cols) > 3 else None}
        )
    return {"status": status, "files": files}


def parse_markdown(lines):
    fields = {}
    declared = []
    for line in lines:
        m = re.match(r"^-\s*(Model|Model part number|Software version|Georgian for Garmin):\s*(.+)$", line)
        if m:
            fields.setdefault(m[1], m[2].strip())
            continue
        m = re.match(r"^\|\s*(\d{3}-[A-Z0-9]{5}-\d{2})\s*\|\s*([^|]*?)\s*\|\s*([^|]+?)\s*\|$", line)
        if m:
            declared.append(
                {"path": "Garmin/ExtData", "file": m[3], "part": m[1], "version": m[2] or None}
            )
    missing = [k for k in ("Model", "Model part number", "Software version") if k not in fields]
    if missing:
        raise ReportError(f"no report found (missing {', '.join(missing)})")
    app = fields.get("Georgian for Garmin", "")
    return {
        "app_version": app if re.fullmatch(r"\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?", app) else None,
        "connection": "mtp",
        "model": fields["Model"],
        "model_part": fields["Model part number"],
        "software": fields["Software version"],
        "declared": declared,
        # 0.1.x and 0.2.x reports didn't list the folder.
        "extdata": None,
        "hidden": None,
    }


def fixture(text, source, connection=None):
    lines = [line.strip().strip("\r") for line in text.splitlines()]
    lines = [line for line in lines if not line.startswith("```")]
    if any(re.match(r"^watch \S", line) for line in lines):
        out = parse_compact(lines)
    else:
        out = parse_markdown(lines)
    if connection:
        out["connection"] = connection
    order = ("app_version", "connection", "model", "model_part", "software", "declared",
             "extdata", "hidden")
    out = {"source": source, **{key: out[key] for key in order}}
    check(out)
    return out


def check(value, key=None):
    """Refuse anything that looks like a unit ID or serial number."""
    if isinstance(value, dict):
        for k, v in value.items():
            check(v, k)
    elif isinstance(value, list):
        for v in value:
            check(v, key)
    elif isinstance(value, str) and key not in ("sha256", "source") and ID_LIKE.search(value):
        raise ReportError(f"{key}: {value!r} looks like an identifier; not writing it")


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Turn a watch report's text into a replay fixture (JSON).",
        epilog=__doc__.split("Example:", 1)[1],
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("file", nargs="?", help="the report text (default: stdin)")
    parser.add_argument("--source", required=True,
                        help="the report's URL, or \"maintainer's watch: …\" for our own watch")
    parser.add_argument("--connection", choices=["mtp", "usb_drive"],
                        help="override the connection (prose reports of USB-drive watches)")
    parser.add_argument("--out", help="write here instead of stdout")
    args = parser.parse_args(argv)
    text = open(args.file, encoding="utf-8").read() if args.file else sys.stdin.read()
    try:
        out = fixture(text, args.source, args.connection)
    except (ReportError, json.JSONDecodeError, IndexError, ValueError) as error:
        print(json.dumps({"error": {"code": "invalid_input", "message": str(error)}}),
              file=sys.stderr)
        return 2
    rendered = json.dumps(out, indent=2, ensure_ascii=False) + "\n"
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(rendered)
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    sys.exit(main())
