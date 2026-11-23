"""The Watch client against the real CLI's operation declarations, so client and server can't
drift apart while the stand-in server's tests stay green.

Every request each `Watch` method builds is checked against the input schema of the operation
it names, as a dev build of `georgian-for-garmin --help --json` declares them; and every declared
operation must have a method, or be in OMITTED with the reason. Runs when
GEORGIAN_FOR_GARMIN_CLI is a dev build (`cargo build -p georgian-for-garmin --features dev`), as
in CI.
"""

import hashlib
import json
import os
import subprocess

import pytest
from jsonschema import Draft202012Validator
from registry_data import PINS

from garmin_fonts.watch import Mtp, Watch

CLI = os.environ.get("GEORGIAN_FOR_GARMIN_CLI")
pytestmark = pytest.mark.skipif(not CLI, reason="set GEORGIAN_FOR_GARMIN_CLI to a dev build of the CLI")

# Operations the client deliberately has no method for, with why. None so far.
OMITTED = {}
# Watch's own methods that send no operation.
NOT_OPERATIONS = {"call", "close"}


class Recorder(Watch):
    """A Watch that records each request instead of sending it, answering paged listings."""

    def __init__(self):
        self.requests = []

    def call(self, method, params=None, progress=None):
        params = dict(params or {})
        if progress is not None:
            params["_meta"] = {"progressToken": len(self.requests) + 1}
        self.requests.append((method, params))
        if method == "extdata_list":
            offset = params.get("offset", 0)
            name = f"006-D9035-0{offset // 1000}.fnt"
            return {"files": [{"name": name, "sha256": "0" * 64}],
                    "next_offset": offset + 1000 if offset < 1000 else None}
        # What the raw operations' methods read from an answer.
        empty = {"bytes": 0, "sha256": hashlib.sha256(b"").hexdigest(), "base64": "", "path": None}
        return {"storage_ids": [], "handles": [], "data": empty, "response_code": 0x2001,
                "response": "OK", "response_params": [], "operation": None}

    def close(self):
        pass


@pytest.fixture(scope="module")
def tools():
    result = subprocess.run([CLI, "--help", "--json"], capture_output=True, text=True, check=True)
    return {tool["name"]: tool for tool in json.loads(result.stdout)["tools"]}


def traced(watch):
    """A trace around one call: mtp_trace on, the call, mtp_trace off."""
    with watch.trace(on_transaction=print):
        watch.mtp.storage_ids()


def exercise(tmp_path):
    """Every public Watch method, with the parameters callers pass: {method: [requests]}."""
    font = tmp_path / "font.fnt"
    font.write_bytes(b"not a font")
    calls = {
        "summary": lambda w: w.summary(),
        "extdata_list": lambda w: (w.extdata_list(), w.extdata_list(
            hash=["006-D9035-03.fnt"], offset=200, limit=1000, output_file=tmp_path / "all.json")),
        "hash": lambda w: w.hash("006-D9035-00.fnt", "006-D9035-01.FNT", "absent.fnt"),
        "install": lambda w: (w.install(), w.install(["006-D9035-03"], progress=print)),
        "restore_stock": lambda w: (w.restore_stock(), w.restore_stock(["006-D9035-03"])),
        "inspect_drive": lambda w: (w.inspect_drive(), w.inspect_drive("/Volumes/GARMIN")),
        "issue_link": lambda w: (
            w.issue_link(busy={"holder": {"kind": "garmin_express", "confirmed": True}}),
            w.issue_link(outcome={"operation": "install", "status": "staged_verified",
                                  "parts": ["006-D9035-03"]},
                         busy={"holder": {"kind": "unknown", "confirmed": False}}),
        ),
        "read_file": lambda w: (
            w.read_file("006-D9035-03.fnt", tmp_path / "out.fnt"),
            w.read_file("GarminDevice.xml", tmp_path / "id.xml", reveal_identifiers=True),
        ),
        "stage": lambda w: (
            w.stage(font, "D844870.tmp", dry_run=True),
            w.stage(b"not a font either", "006-D8947-22.tmp", garmin_sha256=[], progress=print),
            w.stage(font, "006-D8945-22.tmp",
                    garmin_sha256=[PINS["006-D8945-22"].sha256, "1" * 64]),
        ),
        "remove_staged": lambda w: w.remove_staged("staged-1"),
        "wait_for_watch": lambda w: (w.wait_for_watch(), w.wait_for_watch(0, progress=print)),
        "ptp": lambda w: (
            w.ptp(0x1001),
            w.ptp(0x1008, [0x01000032], max_bytes=4096, reveal_identifiers=True),
            w.ptp(0x101B, (0x01000032, 0, 64)),
        ),
        "trace": traced,
        "mtp.device_info": lambda w: (w.mtp.device_info(),
                                      w.mtp.device_info(reveal_identifiers=True)),
        "mtp.storage_ids": lambda w: w.mtp.storage_ids(),
        "mtp.storage_info": lambda w: w.mtp.storage_info(0x00020001),
        "mtp.handles": lambda w: (w.mtp.handles(0x00020001, 0xFFFFFFFF),
                                  w.mtp.handles(0x00020001, 0x01000009, hidden=True)),
        "mtp.object_info": lambda w: w.mtp.object_info(0x01000032),
        "mtp.read": lambda w: (
            w.mtp.read(0x01000032),
            w.mtp.read(0x01000032, 10, 20, max_bytes=100, reveal_identifiers=True),
            w.mtp.read(0x01000040, path=tmp_path / "font.fnt"),
        ),
        "mtp.create": lambda w: (
            w.mtp.create("006-D9035-03.tmp", b"bytes"),
            w.mtp.create("D844870.tmp", font, allow_write=True, storage=0x00020001,
                         parent=0x01000009),
        ),
        "mtp.delete": lambda w: (w.mtp.delete(0x01000102),
                                 w.mtp.delete(0x01000102, allow_write=True)),
    }
    recorded = {}
    for method, run in calls.items():
        watch = Recorder()
        run(watch)
        recorded[method] = watch.requests
    return recorded


def public_methods(cls):
    return {name for name, value in vars(cls).items()
            if not name.startswith("_") and callable(value)}


def test_every_method_is_exercised(tmp_path):
    """Watch's methods and those of its `mtp` namespace, so none skips the schema check."""
    public = public_methods(Watch) | {f"mtp.{name}" for name in public_methods(Mtp)}
    assert public - NOT_OPERATIONS == set(exercise(tmp_path))


def test_every_request_matches_the_declared_input_schema(tools, tmp_path):
    for method, requests in exercise(tmp_path).items():
        assert requests, f"{method} sent nothing"
        for operation, params in requests:
            assert operation in tools, f"{method} calls {operation}, which the CLI doesn't declare"
            params = {k: v for k, v in params.items() if k != "_meta"}
            errors = list(Draft202012Validator(tools[operation]["inputSchema"]).iter_errors(params))
            assert not errors, f"{method} → {operation} {params}: {errors[0].message}"


def test_the_short_name_vouches_its_parts_pinned_hash(tmp_path):
    (operation, params), *_ = exercise(tmp_path)["stage"]
    assert operation == "stage_file" and params["name"] == "D844870.tmp"
    assert params["garmin_sha256"] == [PINS["006-D8448-70"].sha256] and params["dry_run"] is True


def test_hash_pages_through_the_listing(tmp_path):
    requests = exercise(tmp_path)["hash"]
    assert [p["offset"] for _, p in requests] == [0, 1000]
    assert all(p["hash"] == ["006-D9035-00.fnt", "006-D9035-01.FNT", "absent.fnt"]
               for _, p in requests)


def test_every_declared_operation_has_a_method(tools, tmp_path):
    called = {operation for requests in exercise(tmp_path).values() for operation, _ in requests}
    missing = set(tools) - called - set(OMITTED)
    assert not missing, f"operations without a Watch method: {sorted(missing)}; add one or list it in OMITTED"
    assert not set(OMITTED) - set(tools), "OMITTED names operations the CLI doesn't declare"
    assert {"read_file", "stage_file", "remove_staged"} <= set(tools), "not a dev build"
