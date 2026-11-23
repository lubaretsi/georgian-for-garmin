import copy

import pytest
from registry_data import ENTRIES, REGISTRY

from garmin_fonts.support import (
    BEGIN,
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
)

README = REGISTRY.parents[1] / "README.md"


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
    table_rows = by_model(ENTRIES)
    assert table_rows["Venu 2"] == ("19.05", "Tested")
    assert table_rows["Venu 2 Plus"] == ("19.05", f"Tested ([#4]({ISSUES}/4))")
    assert table_rows["Forerunner 165"][1].startswith("Tested ([#5]")
    software, status = table_rows["Forerunner 245"]
    assert software == "13.70"
    usb_drive = (
        "The app found no font on this watch it can add Georgian to: it connects only as a USB "
        "drive."
    )
    assert status == f"Doesn't work ([#16]({ISSUES}/16)). {usb_drive}"
    assert table_rows["fenix 5"] == (
        "25.00",
        f"Doesn't work ([#30]({ISSUES}/30), [#29]({ISSUES}/29)). {usb_drive}",
    )
    software, status = table_rows["fenix 6S Pro"]
    assert software == "28.02"
    assert status == (
        f"Doesn't work ([#6]({ISSUES}/6), [#20]({ISSUES}/20)). The watch has the font file but "
        "doesn't use it for Georgian letters: they still show as question marks."
    )
    assert table_rows["Venu 3"] == ("–", f"Not confirmed yet ([#5]({ISSUES}/5), [#2]({ISSUES}/2))")


def test_every_known_model_gets_one_row_in_list_order():
    known = [w["model"] for w in ENTRIES["watches"] if "prediction" not in w]
    assert [watch for watch, _, _ in rows(ENTRIES)] == known


def test_predicted_models_are_grouped_by_family_one_row_per_name():
    predicted = [w for w in ENTRIES["watches"] if "prediction" in w]
    table_rows = predicted_rows(ENTRIES)
    names = [watch for _, watch, _ in table_rows]
    assert names == list(dict.fromkeys(w["model"] for w in predicted))
    families = [family for family, _, _ in table_rows if family]
    assert families == list(dict.fromkeys(w["model"].split()[0] for w in predicted))
    assert len(set(families)) == len(families)
    by_name = {watch: based_on for _, watch, based_on in table_rows}
    assert by_name["Forerunner 265"] == (
        "[Connect IQ device reference]"
        "(https://developer.garmin.com/connect-iq/articles/device-reference/fr265.html)"
    )
    assert by_name["fenix 7S Pro"] == (
        "Firmware family of the tested fenix 7 Pro Solar, Pro Sapphire Solar"
    )
    assert by_name["Venu"] == f"Korean bitmap fonts in its watch report ([#12]({ISSUES}/12))"
    assert by_name["Lily 2 Active"] == (
        "Korean bitmap fonts in its watch report, emailed to the maintainer"
    )


def test_tested_software_versions_are_merged_and_sorted():
    entries = copy.deepcopy(ENTRIES)
    venu2 = dict(entries["packs"][0]["tested"][0], software="9.80")
    entries["packs"][0]["tested"].append(venu2)
    entries["packs"][1]["tested"].append(dict(venu2, software="19.05", parts=["006-D9035-10"]))
    assert by_model(entries)["Venu 2"] == ("9.80, 19.05", "Tested")


def test_update_replaces_only_the_tables():
    text = f"intro\n{BEGIN}\nold\n{END}\nmiddle\n{PREDICTED_BEGIN}\nold\n{PREDICTED_END}\noutro\n"
    assert update(text, ENTRIES) == (
        f"intro\n{table(ENTRIES)}\nmiddle\n{predicted_table(ENTRIES)}\noutro\n"
    )
    with pytest.raises(ValueError, match="no watch table"):
        update("no markers", ENTRIES)
    with pytest.raises(ValueError, match="no watch table"):
        update(f"{BEGIN}\nold\n{END}\n", ENTRIES)
