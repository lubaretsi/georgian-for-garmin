"""The Watch client over a stand-in for `georgian-for-garmin rpc` (fake_rpc.py), and, with
GEORGIAN_FOR_GARMIN_CLI set to a dev build, over the real one without a watch."""

import dataclasses
import json
import os
import signal
import subprocess
import sys
import threading
import time
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


def test_stage_refuses_a_font_for_another_part(watch, catalog_stock):
    thai = catalog_stock["006-D8947-22"]
    with pytest.raises(WatchError, match="the font is 006-D8947-22, but the staging name") as error:
        watch.stage(thai, "006-D9035-03.tmp", dry_run=True)
    assert error.value.code == "invalid_input"
    # The vouched Garmin hash is always the staging name's part's.
    staged = watch.stage(thai, dry_run=True)
    assert staged["name"] == "006-D8947-22.tmp"
    assert staged["garmin_sha256"] == [PINS["006-D8947-22"].sha256]


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


def test_trace_collects_each_transaction_in_order(watch):
    live = []
    with watch.trace(on_transaction=live.append) as transactions:
        assert watch.mtp.handles(0x00020001, 0x01000009, hidden=True) == [0x01000001, 0x01000002]
        answer = watch.ptp(0x1008, [0x01000001])
        steps = []
        assert watch.wait_for_watch(5, progress=steps.append)["model"] == "Venu 2"
        assert len(transactions) == 7, "appended as they arrive, inside the block"
    assert (answer.ok, answer.name, answer.data) == (True, "OK", b"answer to GetObjectInfo")
    assert [s["progress"] for s in steps] == [1, 2], "progress still reaches its callback"
    assert [t["operation"] for t in transactions] == [
        "GetStorageIDs", "GarminGetObjectIds", "GetStorageIDs", "GetObjectInfo",
        "GetStorageIDs", "poll 1", "poll 2"]
    assert live == transactions
    sent = [m for m in received(watch) if m.get("method") == "mtp_trace"]
    assert [m["params"] for m in sent] == [{"enabled": True}, {"enabled": False}]
    # Off: nothing more is collected.
    watch.mtp.handles(0x00020001, 0x01000009, hidden=True)
    assert len(transactions) == 7


def test_trace_is_turned_off_after_ctrl_c_and_the_session_goes_on(watch):
    timer = threading.Timer(0.5, os.kill, (os.getpid(), signal.SIGINT))
    timer.start()
    with pytest.raises(KeyboardInterrupt):
        with watch.trace() as transactions:
            watch.call("slow")
    assert [t["operation"] for t in transactions] == ["GetStorageIDs", "GetObject"]
    sent = [m["params"] for m in received(watch) if m.get("method") == "mtp_trace"]
    assert sent == [{"enabled": True}, {"enabled": False}]
    with watch.trace() as again:
        watch.mtp.read(0x01000032)
    assert [t["operation"] for t in again] == ["GetStorageIDs", "GetObject"]


def test_raw_reads_return_checked_bytes_and_refusals_are_the_cores(watch):
    assert watch.mtp.read(0x01000032) == b"<Device><Id>**********</Id></Device>"
    for refused in (lambda: watch.ptp(0x100B, [0xFFFFFFF0]),
                    lambda: watch.mtp.create("006-D9035-03.tmp", b"x"),
                    lambda: watch.mtp.delete(0x01000102)):
        with pytest.raises(WatchError) as error:
            refused()
        assert (error.value.code, error.value.rpc_code) == ("invalid_input", -32602)
    sent = [m for m in received(watch) if m.get("method") in ("mtp_create", "mtp_delete")]
    assert all(m["params"]["allow_write"] is False for m in sent), "sent as given"


def test_a_missing_cli_says_what_to_pass(monkeypatch):
    monkeypatch.delenv("GEORGIAN_FOR_GARMIN_CLI", raising=False)
    monkeypatch.setenv("PATH", "")
    with pytest.raises(WatchError, match="not on PATH"):
        Watch()
    with pytest.raises(WatchError, match="can't run /no/such/georgian-for-garmin") as error:
        Watch("/no/such/georgian-for-garmin")
    assert error.value.code == "invalid_input"


def test_output_that_isnt_json_rpc_is_an_error(watch):
    with pytest.raises(WatchError, match="isn't JSON-RPC: b'this is not JSON") as error:
        watch.call("garble")
    assert error.value.code == "internal"


def test_stage_refuses_a_build_from_an_unpinned_rasterizer(watch, catalog_stock):
    from garmin_fonts.build import Built

    data = catalog_stock["006-D8947-22"]
    built = Built("006-D8947-22", "2.00", "bitmap", "thai", 500, None, {}, {}, "", True,
                  {"pillow": "12.3.0", "raqm": None}, False, data)
    with pytest.raises(WatchError, match="not the pinned rasterizer") as error:
        watch.stage(built, dry_run=True)
    assert error.value.code == "invalid_input"
    assert watch.stage(built, dry_run=True, allow_unpinned_env=True)["status"] == "dry_run"
    pinned = dataclasses.replace(built, rasterizer_pinned=True)
    assert watch.stage(pinned, dry_run=True)["status"] == "dry_run"


def test_close_ends_the_process(tmp_path):
    session = Watch(FAKE)
    session.close()
    assert session._process.returncode == 0
    session.close()  # again: nothing to do


def test_a_dropped_session_ends_the_process():
    session = Watch(FAKE)
    process = session._process
    del session
    assert process.poll() is not None


# A terminal's Ctrl-C: SIGINT to the whole foreground process group, during a call. Run in its own
# session so the signal reaches nothing but this script and what shares its group.
GROUP_SIGINT = r"""
import json, os, signal, sys, threading
sys.path.insert(0, sys.argv[1])
from garmin_fonts.watch import Watch, WatchError
method, params, command = sys.argv[2], json.loads(sys.argv[3]), sys.argv[4:]
watch = Watch(command)
threading.Timer(1.0, os.killpg, (os.getpgid(0), signal.SIGINT)).start()
try:
    watch.call(method, params)
    sys.exit("the call wasn't interrupted")
except KeyboardInterrupt:
    pass
alive = watch._process.poll() is None
try:
    after = watch.summary() if method == "slow" else watch.extdata_list(limit=0)
except WatchError as error:
    after = error.code
process = watch._process
watch.close()
print(json.dumps({"alive": alive, "after": after, "exit": process.returncode}))
"""


def group_sigint(method, params, command):
    result = subprocess.run(
        [sys.executable, "-c", GROUP_SIGINT, str(Path(__file__).parents[1]), method,
         json.dumps(params), *command],
        capture_output=True, text=True, start_new_session=True, timeout=120,
    )
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


@pytest.mark.skipif(sys.platform == "win32", reason="process groups are POSIX")
def test_ctrl_c_in_a_terminal_cancels_and_keeps_the_session():
    outcome = group_sigint("slow", {}, FAKE)
    assert outcome == {"alive": True, "after": {"model": "Venu 2", "model_part": "006-B3703-00",
                                                "software": "19.05"}, "exit": 0}


@pytest.mark.skipif(not (os.environ.get("GEORGIAN_FOR_GARMIN_CLI")
                         and os.environ.get("GFG_TEST_NO_WATCH") == "1"),
                    reason="needs a dev CLI and no watch attached (GFG_TEST_NO_WATCH=1, as in CI): "
                           "wait_for_watch looks for one over USB")
def test_ctrl_c_in_a_terminal_cancels_the_real_rpc():
    outcome = group_sigint("wait_for_watch", {"timeout_seconds": 60},
                           [os.environ["GEORGIAN_FOR_GARMIN_CLI"]])
    assert outcome == {"alive": True, "after": "invalid_input", "exit": 0}


# A Python that opens two sessions, makes a call (one the real core refuses before it looks for a
# watch), reports its rpc children's pids, then waits to be killed, as omp kills a kernel on
# reset, timeout or crash. The second session's child must not hold the first's input open.
KILLED_PARENT = r"""
import sys, time
sys.path.insert(0, sys.argv[1])
from garmin_fonts.watch import Watch, WatchError
first, second = Watch(sys.argv[2:]), Watch(sys.argv[2:])
try:
    first.extdata_list(limit=0)
except WatchError:
    pass
print(first._process.pid, second._process.pid, flush=True)
time.sleep(600)
"""


def alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True


def children_outlive_a_killed_parent(command):
    """The rpc children still running 10 s after their Python was SIGKILLed (killed then)."""
    parent = subprocess.Popen(
        [sys.executable, "-c", KILLED_PARENT, str(Path(__file__).parents[1]), *command],
        stdout=subprocess.PIPE, text=True, start_new_session=True,
    )
    pids = [int(pid) for pid in parent.stdout.readline().split()]
    assert len(pids) == 2 and all(map(alive, pids))
    parent.kill()
    parent.wait()
    deadline = time.monotonic() + 10
    while any(map(alive, pids)) and time.monotonic() < deadline:
        time.sleep(0.05)
    left = [pid for pid in pids if alive(pid)]
    for pid in left:
        os.kill(pid, signal.SIGKILL)
    return left


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX signals")
def test_a_killed_python_leaves_no_rpc_behind():
    assert children_outlive_a_killed_parent(FAKE) == []


@pytest.mark.skipif(not os.environ.get("GEORGIAN_FOR_GARMIN_CLI") or sys.platform == "win32",
                    reason="set GEORGIAN_FOR_GARMIN_CLI to a dev build of the CLI")
def test_a_killed_python_leaves_no_real_rpc_behind():
    assert children_outlive_a_killed_parent([os.environ["GEORGIAN_FOR_GARMIN_CLI"]]) == []


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
            (tmp_path / "x.fnt").write_bytes(b"not a font")
            real.stage(tmp_path / "x.fnt", "../escape.tmp", garmin_sha256=[])
        assert error.value.code == "invalid_input"
        with pytest.raises(WatchError) as error:
            real.call("no_such_operation")
        assert error.value.rpc_code == -32601
        # The raw operations' refusals, all before the core opens the watch; mtp_trace itself
        # never opens it. A write opcode refused, and writes without allow_write.
        with real.trace() as transactions:
            for refused in (lambda: real.ptp(0x100B, [0xFFFFFFF0]),
                            lambda: real.mtp.create("006-D9035-03.tmp", b"x"),
                            lambda: real.mtp.delete(0xFFFFFFF0)):
                with pytest.raises(WatchError) as error:
                    refused()
                assert (error.value.code, error.value.rpc_code) == ("invalid_input", -32602)
        assert transactions == [], "nothing was sent to a watch"
    assert real._process.returncode == 0
