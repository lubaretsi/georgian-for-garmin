import copy

import pytest
from registry_data import ENTRIES, REGISTRY

from garmin_fonts.support import BEGIN, END, ISSUES, rows, section, table, update

README = REGISTRY.parents[1] / "README.md"


def test_the_readme_table_is_the_support_list():
    readme = README.read_text()
    assert section(readme) == table(ENTRIES), (
        "the README's watch table differs from fonts/registry.json; run "
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
    assert status.startswith(f"Doesn't work ([#16]({ISSUES}/16)). It connects to a computer only")
    assert status.endswith("could be added to.")
    assert table_rows["fenix 6S Pro"] == ("–", f"Being tested in the beta ([#6]({ISSUES}/6))")
    assert table_rows["Venu 3"] == ("–", f"Not confirmed yet ([#5]({ISSUES}/5))")


def test_every_model_gets_one_row_in_list_order():
    assert [watch for watch, _, _ in rows(ENTRIES)] == [w["model"] for w in ENTRIES["watches"]]


def test_tested_software_versions_are_merged_and_sorted():
    entries = copy.deepcopy(ENTRIES)
    venu2 = dict(entries["packs"][0]["tested"][0], software="9.80")
    entries["packs"][0]["tested"].append(venu2)
    entries["packs"][1]["tested"].append(dict(venu2, software="19.05", parts=["006-D9035-10"]))
    assert by_model(entries)["Venu 2"] == ("9.80, 19.05", "Tested")


def test_update_replaces_only_the_table():
    text = f"intro\n{BEGIN}\nold\n{END}\noutro\n"
    assert update(text, ENTRIES) == f"intro\n{table(ENTRIES)}\noutro\n"
    with pytest.raises(ValueError, match="no watch table"):
        update("no markers", ENTRIES)
