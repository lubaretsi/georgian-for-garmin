import copy

import pytest
from registry_data import ENTRIES, REGISTRY

from garmin_fonts.support import (
    BEGIN,
    EMAILED_REPORT,
    END,
    ISSUES,
    PREDICTED_BEGIN,
    PREDICTED_END,
    predicted_rows,
    predicted_table,
    rows,
    section,
    table,
    update,
    version_key,
)

README = REGISTRY.parents[1] / "README.md"

# The generator's words are checked on this support list of its own, so confirming a watch in
# fonts/registry.json changes no test here; the real registry is checked by invariants below.
# Made-up models and part numbers; the reasons are the shape of the registry's.
USB_DRIVE = "It connects only as a USB drive."
FONT_UNUSED = "It doesn't use the font for Georgian letters."


def watch(model, model_part, issues=(), **extra):
    return {"model": model, "model_part": model_part, "issues": list(issues), **extra}


def predicted(family, source):
    return {"family": family, "confidence": "high", "basis": "made up", "source": source}


CONNECT_IQ = "https://developer.garmin.com/connect-iq/articles/device-reference/example.html"
FIXTURE = {
    "reasons": [
        {"id": "usb_drive_only", "en": USB_DRIVE, "ka": "-"},
        {"id": "font_unused", "en": FONT_UNUSED, "ka": "-"},
    ],
    "packs": [
        {"id": "a", "tested": [
            {"model_part": "006-B9901-00", "software": "19.05", "parts": ["006-D0000-01"]},
            {"model_part": "006-B9902-00", "software": "19.05", "parts": ["006-D0000-01"]},
        ]},
        {"id": "b", "tested": [
            {"model_part": "006-B9901-00", "software": "9.80", "parts": ["006-D0000-10"]},
            {"model_part": "006-B9901-00", "software": "19.05", "parts": ["006-D0000-10"]},
        ]},
    ],
    "watches": [
        watch("Example Tested", "006-B9901-00"),
        watch("Example Tested Issue", "006-B9902-00", [4]),
        watch("Example Drive", "006-B9903-00", [16],
              unsupported={"software": ["13.70"], "reason": "usb_drive_only"}),
        watch("Example Unused", "006-B9904-00", [6, 20],
              unsupported={"software": ["28.02", "28.10"], "reason": "font_unused"}),
        watch("Example Expected", "006-B9905-00", [5, 2]),
        watch("Example Predicted", "006-B9906-00", prediction=predicted("noto_truetype", CONNECT_IQ)),
        watch("Example Predicted", "006-B9907-00", prediction=predicted("noto_truetype", CONNECT_IQ)),
        watch("Example Sibling", "006-B9908-00",
              prediction=predicted("bitmap_armenian", "firmware family of Example Tested")),
        watch("Other Korean", "006-B9909-00", [12],
              prediction=predicted("bitmap_korean", f"{ISSUES}/12")),
        watch("Other Emailed", "006-B9910-00",
              prediction=predicted("bitmap_korean", EMAILED_REPORT)),
    ],
}


def test_the_readme_tables_are_the_support_list():
    readme = README.read_text()
    assert section(readme) == table(ENTRIES), (
        "the README's watch table differs from fonts/registry.json; run "
        "`python -m garmin_fonts.support` from builder/"
    )
    assert section(readme, PREDICTED_BEGIN, PREDICTED_END) == predicted_table(ENTRIES), (
        "the README's predicted watch table differs from fonts/registry.json; run "
        "`python -m garmin_fonts.support` from builder/"
    )


def by_model(registry):
    return {watch: (software, status) for watch, software, status in rows(registry)}


def test_each_status_has_its_words():
    assert rows(FIXTURE) == [
        ("Example Tested", "9.80, 19.05", "Tested"),
        ("Example Tested Issue", "19.05", f"Tested ([#4]({ISSUES}/4))"),
        ("Example Drive", "13.70", f"Doesn't work ([#16]({ISSUES}/16)). {USB_DRIVE}"),
        (
            "Example Unused",
            "28.02, 28.10",
            f"Doesn't work ([#6]({ISSUES}/6), [#20]({ISSUES}/20)). {FONT_UNUSED}",
        ),
        ("Example Expected", "–", f"Not confirmed yet ([#5]({ISSUES}/5), [#2]({ISSUES}/2))"),
    ]


def test_each_prediction_source_has_its_words_one_row_per_name_grouped_by_family():
    assert predicted_rows(FIXTURE) == [
        ("Example", "Example Predicted", f"[Connect IQ device reference]({CONNECT_IQ})"),
        ("", "Example Sibling", "Firmware family of the tested Example Tested"),
        ("Other", "Other Korean", f"Korean bitmap fonts in its watch report ([#12]({ISSUES}/12))"),
        ("", "Other Emailed", "Korean bitmap fonts in its watch report, emailed to the maintainer"),
    ]


def test_the_registry_gets_one_row_per_model_in_list_order():
    known = [w["model"] for w in ENTRIES["watches"] if "prediction" not in w]
    assert [watch for watch, _, _ in rows(ENTRIES)] == known
    predicted = [w for w in ENTRIES["watches"] if "prediction" in w]
    table_rows = predicted_rows(ENTRIES)
    names = [watch for _, watch, _ in table_rows]
    assert names == list(dict.fromkeys(w["model"] for w in predicted))
    families = [family for family, _, _ in table_rows if family]
    assert families == list(dict.fromkeys(w["model"].split()[0] for w in predicted))
    assert len(set(families)) == len(families)


def test_the_registry_rows_follow_its_entries():
    # A model with tested entries is Tested on exactly those versions; an unsupported one doesn't
    # work on its versions; the rest aren't confirmed yet. Whichever models those are today.
    tested = {}
    for pack in ENTRIES["packs"]:
        for test in pack["tested"]:
            tested.setdefault(test["model_part"], set()).add(test["software"])
    by_part = {w["model_part"]: w for w in ENTRIES["watches"] if "prediction" not in w}
    for (watch, software, status), entry in zip(rows(ENTRIES), by_part.values(), strict=True):
        assert watch == entry["model"]
        if entry["model_part"] in tested:
            versions = sorted(tested[entry["model_part"]], key=version_key)
            assert (software, status.split(" ")[0]) == (", ".join(versions), "Tested"), watch
        elif "unsupported" in entry:
            assert software == ", ".join(entry["unsupported"]["software"]), watch
            assert status.startswith("Doesn't work"), watch
        else:
            assert (software, status.split(" (")[0]) == ("–", "Not confirmed yet"), watch


def test_tested_software_versions_are_merged_and_sorted():
    entries = copy.deepcopy(FIXTURE)
    entries["packs"][1]["tested"].append(
        {"model_part": "006-B9902-00", "software": "9.80", "parts": ["006-D0000-10"]}
    )
    assert by_model(entries)["Example Tested Issue"] == (
        "9.80, 19.05",
        f"Tested ([#4]({ISSUES}/4))",
    )


def test_update_replaces_only_the_tables():
    text = f"intro\n{BEGIN}\nold\n{END}\nmiddle\n{PREDICTED_BEGIN}\nold\n{PREDICTED_END}\noutro\n"
    assert update(text, ENTRIES) == (
        f"intro\n{table(ENTRIES)}\nmiddle\n{predicted_table(ENTRIES)}\noutro\n"
    )
    with pytest.raises(ValueError, match="no watch table"):
        update("no markers", ENTRIES)
    with pytest.raises(ValueError, match="no watch table"):
        update(f"{BEGIN}\nold\n{END}\n", ENTRIES)
