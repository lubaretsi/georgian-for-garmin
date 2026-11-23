"""The Watch client over a stand-in for `georgian-for-garmin rpc` (fake_rpc.py); the real dev rpc
is exercised by hand (docs/development.md)."""

import json
import os
import signal
import sys
import threading
from pathlib import Path

import pytest
from registry_data import PINS

from garmin_fonts.watch import Watch, WatchError, _part_of

FAKE = [sys.executable, str(Path(__file__).with_name("fake_rpc.py"))]


@pytest.fixture
def watch(tmp_path):
    log = tmp_path / "rpc.jsonl"
    with Watch(FAKE, env={"FAKE_RPC_LOG": str(log)}) as session:
        session.log = log
        yield session


def received(watch):
    return [json.loads(line) for line in watch.log.read_text().splitlines()]


def test_operations_return_the_cores_output(watch):
    assert watch.summary()["model"] == "Venu 2"
    page = watch.extdata_list(limit=2)
    assert [f["name"] for f in page["files"]] == ["006-D9035-03.fnt", "006-D8947-22.fnt"]
    assert page["next_offset"] == 2
    first = received(watch)[0]
    assert (first["jsonrpc"], first["method"], first["params"]) == ("2.0", "inspect", {})


def test_hash_reads_every_page(watch):
    hashes = watch.hash("006-D8448-27.FNT", "006-D9035-03.fnt", "006-D0000-00.fnt")
    assert hashes["006-D8448-27.FNT"] and hashes["006-D9035-03.fnt"]
    assert hashes["006-D0000-00.fnt"] is None


def test_errors_carry_the_cores_code(watch):
    with pytest.raises(WatchError) as error:
        watch.install()
    assert (error.value.code, error.value.rpc_code) == ("no_device", -32000)
    with pytest.raises(WatchError) as error:
        watch.restore_stock(["006-D9035-03"])
    assert error.value.code == "device_busy"
    assert error.value.data == {"busy": {"holder": "garmin_express"}}
    with pytest.raises(WatchError) as error:
        watch.call("no_such_operation")
    assert error.value.code == "invalid_input"
    # The session goes on after errors.
    assert watch.summary()["model"] == "Venu 2"


def test_progress_reaches_the_callback(watch):
    steps = []
    assert watch.wait_for_watch(5, progress=steps.append)["model"] == "Venu 2"
    assert [s["progress"] for s in steps] == [1, 2] and steps[0]["message"] == "waiting 1"
    request = received(watch)[-1]
    assert request["params"] == {"timeout_seconds": 5, "_meta": {"progressToken": request["id"]}}


def test_stage_writes_bytes_and_vouches_for_the_pinned_original(watch, tmp_path):
    class Build:
        part, data = "006-D8947-22", b"a test font"

    staged = watch.stage(Build(), dry_run=True)
    assert staged["status"] == "dry_run" and staged["name"] == "006-D8947-22.tmp"
    assert staged["bytes"] == len(Build.data)
    assert staged["garmin_sha256"] == [PINS["006-D8947-22"].sha256]
    font = tmp_path / "font.fnt"
    font.write_bytes(b"another font")
    staged = watch.stage(font, "D844870.tmp")
    assert staged["id"] == "staged-1"
    assert staged["garmin_sha256"] == [PINS["006-D8448-70"].sha256]
    assert watch.remove_staged("staged-1")["status"] == "removed"
    with pytest.raises(WatchError, match="pass name"):
        watch.stage(font)


def test_short_staging_names_name_their_part():
    assert _part_of("D844870.tmp") == "006-D8448-70"
    assert _part_of("006-D8448-70.tmp") == "006-D8448-70"


def test_ctrl_c_cancels_the_operation_in_the_core(watch):
    timer = threading.Timer(0.5, os.kill, (os.getpid(), signal.SIGINT))
    timer.start()
    with pytest.raises(KeyboardInterrupt):
        watch.call("slow")
    cancel = received(watch)[-1]
    assert cancel == {"jsonrpc": "2.0", "method": "notifications/cancelled",
                      "params": {"requestId": received(watch)[-2]["id"]}}
    assert watch.summary()["model"] == "Venu 2"


def test_an_exited_rpc_process_is_an_error(watch):
    with pytest.raises(WatchError, match="rpc process exited") as error:
        watch.call("exit")
    assert error.value.code == "internal"


def test_a_missing_cli_says_what_to_pass(monkeypatch):
    monkeypatch.delenv("GEORGIAN_FOR_GARMIN_CLI", raising=False)
    monkeypatch.setenv("PATH", "")
    with pytest.raises(WatchError, match="not on PATH"):
        Watch()


def test_close_ends_the_process(tmp_path):
    session = Watch(FAKE)
    session.close()
    assert session._process.returncode == 0


@pytest.mark.skipif(not os.environ.get("GEORGIAN_FOR_GARMIN_CLI"),
                    reason="set GEORGIAN_FOR_GARMIN_CLI to a dev build of the CLI")
def test_the_real_rpc_checks_input_before_touching_the_watch(tmp_path):
    """Against the real `georgian-for-garmin rpc` (a dev build): only calls the core refuses
    before it connects, so it never touches a watch that may be plugged in."""
    with Watch(env={"GEORGIAN_FOR_GARMIN_STATE_DIR": str(tmp_path)}) as real:
        with pytest.raises(WatchError) as error:
            real.extdata_list(limit=0)
        assert (error.value.code, error.value.rpc_code) == ("invalid_input", -32602)
        with pytest.raises(WatchError) as error:
            real.stage(tmp_path / "x.fnt", "../escape.tmp", garmin_sha256=[])
        assert error.value.code == "invalid_input"
        with pytest.raises(WatchError) as error:
            real.call("no_such_operation")
        assert error.value.rpc_code == -32601
    assert real._process.returncode == 0
