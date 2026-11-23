"""The watch, from Python: a `georgian-for-garmin rpc` session.

`Watch()` starts `georgian-for-garmin rpc` (JSON-RPC 2.0, one message per line; method =
operation name, params = its input, result = its output) and keeps it for its lifetime, so the
watch connection stays open between calls. Device work stays in the Rust core: its pre-write
checks, readback and identification apply to every call. The dev operations (`read_file`,
`stage_file`, `remove_staged`) exist only in a dev build of the CLI (`cargo build -p
georgian-for-garmin --features dev`); pass its path as `command`.

    with Watch() as watch:
        watch.summary()["model"]
        watch.hash("006-D9035-03.fnt")
        staged = watch.stage(add_georgian("006-D8947-22", markers={"ჰ": "გ"}))
        watch.remove_staged(staged["id"])

Errors become WatchError with the core's code (`no_device`, `device_busy`, `conflict`, …; the
busy holder in `data`). Ctrl-C during a call cancels it in the core (`notifications/cancelled`)
and raises KeyboardInterrupt once the core has answered. The rpc process runs in its own
process group, so a terminal's Ctrl-C reaches only Python, which forwards it; `close()`, the
`with` block's end, garbage collection and interpreter exit end the process, and it also exits
by itself when its input closes (if Python is killed).
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import weakref
from pathlib import Path

from . import registry as font_registry

# The CLI to run when `command` isn't given: $GEORGIAN_FOR_GARMIN_CLI, else on PATH.
CLI_ENV = "GEORGIAN_FOR_GARMIN_CLI"
CLI = "georgian-for-garmin"
# How long ending a session waits for the rpc process at each step: after its input closes, then
# after SIGTERM, before SIGKILL.
CLOSE_TIMEOUT = 10


def _end(process):
    """End an rpc process: close its input (it answers what it has read, then exits), then
    terminate it, then kill it, waiting CLOSE_TIMEOUT between steps."""
    if process.stdin and not process.stdin.closed:
        try:
            process.stdin.close()
        except OSError:
            pass
    for stop in (None, process.terminate, process.kill):
        if stop is not None and process.poll() is None:
            stop()
        try:
            process.wait(timeout=CLOSE_TIMEOUT)
            break
        except subprocess.TimeoutExpired:
            continue
    if process.stdout:
        process.stdout.close()


# Start the rpc process outside the terminal's foreground process group, so Ctrl-C (SIGINT to
# the whole group) doesn't kill it mid-call: Python gets it and cancels through JSON-RPC.
if sys.platform == "win32":
    _ISOLATED = {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
else:
    _ISOLATED = {"start_new_session": True}


class WatchError(Exception):
    """An operation failed: `code` is the core's error code, `data` the error's other fields
    (such as `busy`), `rpc_code` the JSON-RPC code."""

    def __init__(self, code, message, data=None, rpc_code=None):
        super().__init__(f"{code}: {message}")
        self.code, self.message, self.data, self.rpc_code = code, message, data or {}, rpc_code


class Watch:
    """A long-lived `georgian-for-garmin rpc` session; a context manager.

    `command` is the CLI (a path or a list of arguments before `rpc`); `env` adds environment
    variables for it (e.g. GEORGIAN_FOR_GARMIN_STATE_DIR, where staged ids are logged).
    """

    def __init__(self, command=None, *, env=None):
        if command is None:
            command = os.environ.get(CLI_ENV) or shutil.which(CLI)
            if command is None:
                raise WatchError(
                    "invalid_input",
                    f"{CLI} is not on PATH: pass command=<path to the CLI> or set {CLI_ENV}",
                )
        args = [*command, "rpc"] if isinstance(command, (list, tuple)) else [str(command), "rpc"]
        try:
            self._process = subprocess.Popen(
                args, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                env={**os.environ, **(env or {})}, **_ISOLATED,
            )
        except OSError as error:
            raise WatchError("invalid_input", f"can't run {args[0]}: {error}") from None
        # Runs once: on close(), when the Watch is collected, or at interpreter exit.
        self._end = weakref.finalize(self, _end, self._process)
        self._next_id = 0

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def close(self):
        """End the session: close the rpc process's input and wait for it to exit (terminating
        it if it doesn't). Calling it again does nothing."""
        self._end()

    def _send(self, message):
        try:
            self._process.stdin.write(json.dumps(message, ensure_ascii=False).encode() + b"\n")
            self._process.stdin.flush()
        except (BrokenPipeError, ValueError):
            raise self._exited() from None

    def _exited(self):
        return WatchError("internal", f"the rpc process exited (status {self._process.poll()})")

    def _read(self):
        line = self._process.stdout.readline()
        if not line:
            raise self._exited()
        try:
            message = json.loads(line)
        except ValueError:
            raise WatchError("internal", "the rpc process wrote a line that isn't JSON-RPC: "
                                         f"{line[:200]!r}") from None
        if not isinstance(message, dict):
            raise WatchError("internal", f"the rpc process wrote {line[:200]!r}, not a message")
        return message

    def call(self, method, params=None, progress=None):
        """Run operation `method` with `params` (its input) and return its output.

        `progress` is called with each `notifications/progress` params ({progress, total?,
        message}) the operation sends. Raises WatchError.
        """
        self._next_id += 1
        request_id = self._next_id
        params = dict(params or {})
        if progress is not None:
            params["_meta"] = {"progressToken": request_id}
        self._send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        interrupted = False
        while True:
            try:
                message = self._read()
            except KeyboardInterrupt:
                if interrupted:
                    raise
                interrupted = True
                self._send({"jsonrpc": "2.0", "method": "notifications/cancelled",
                            "params": {"requestId": request_id}})
                continue
            if "id" not in message:
                if message.get("method") == "notifications/progress" and progress is not None:
                    progress(message["params"])
                continue
            if message["id"] != request_id:
                continue
            if interrupted:
                raise KeyboardInterrupt
            if "error" in message:
                error = message["error"]
                data = dict(error.get("data") or {})
                code = data.pop("code", None) or _RPC_CODES.get(error.get("code"), "internal")
                raise WatchError(code, error.get("message", ""), data, error.get("code"))
            return message["result"]

    # Operations, as the core declares them (crates/core/src/ops.rs).

    def summary(self):
        """`inspect`: the watch's model, software, fonts, support status and report."""
        return self.call("inspect")

    def extdata_list(self, hash=None, offset=0, limit=200, output_file=None):
        """`extdata_list`: one page of GARMIN/ExtData (normal and hidden listing), with the
        SHA-256 of the files named in `hash`; with `output_file`, the whole listing goes there."""
        params = {"offset": offset, "limit": limit}
        if hash:
            params["hash"] = list(hash)
        if output_file is not None:
            params["output_file"] = str(output_file)
        return self.call("extdata_list", params)

    def hash(self, *names):
        """{name: SHA-256} of the named ExtData files, every page read; None for a name the
        watch doesn't have."""
        found, offset = {}, 0
        while offset is not None:
            page = self.extdata_list(hash=names, offset=offset, limit=1000)
            for file in page["files"]:
                if file.get("sha256"):
                    found[file["name"].lower()] = file["sha256"]
            offset = page["next_offset"]
        return {name: found.get(name.lower()) for name in names}

    def install(self, parts=None, progress=None):
        """`install`: write this build's Georgian fonts (only `parts`, when given)."""
        return self.call("install", {} if parts is None else {"parts": list(parts)}, progress)

    def restore_stock(self, parts=None, progress=None):
        """`restore_stock`: write Garmin's original fonts back (only `parts`, when given)."""
        return self.call("restore_stock", {} if parts is None else {"parts": list(parts)}, progress)

    def inspect_drive(self, path=None):
        """`inspect_drive`: read a watch connected as a USB drive (the mounted volume found, or
        `path`)."""
        return self.call("inspect_drive", {} if path is None else {"path": str(path)})

    def issue_link(self, report=None, *, busy=None, outcome=None, answers=None):
        """`issue_link`: the GitHub new-issue link for a report (from `summary` or
        `inspect_drive`), the last install or restore `outcome`, the owner's `answers`, and/or a
        `device_busy` error's `busy`."""
        given = {"report": report, "busy": busy, "outcome": outcome, "answers": answers}
        return self.call("issue_link", {k: v for k, v in given.items() if v is not None})

    def read_file(self, name, path):
        """`read_file` (dev): the ExtData file `name` to the local `path`, which must not exist."""
        return self.call("read_file", {"name": name, "path": str(path)})

    def stage(self, source, name=None, *, dry_run=False, garmin_sha256=None,
              allow_unpinned_env=False, progress=None):
        """`stage_file` (dev): stage `source` as GARMIN/ExtData/`name`.

        `source` is a local path, bytes, or a build (build.Built); `name` defaults to a build's
        `<part>.tmp`. A Garmin font whose footer or trailer names another part than `name` is
        refused. A build drawn outside the pinned rasterizer environment
        (`Built.rasterizer_pinned` false) is refused unless `allow_unpinned_env`, since its bytes
        may differ from what a nix build of the same inputs gives. `garmin_sha256` (hashes
        vouched to be Garmin's own file for the part) defaults to the registry's pin for the
        part `name` stands for, so Garmin's original may be replaced. The bytes checked here are
        the bytes staged (from a private copy). Returns the core's output, with the staged `id`
        for `remove_staged`.
        """
        from .build import Built, identify

        if isinstance(source, Built) and not source.rasterizer_pinned and not allow_unpinned_env:
            raise WatchError(
                "invalid_input",
                f"{source.part} was drawn with {source.rasterizer}, not the pinned rasterizer; "
                "build it in nix develop or the omp .venv, or pass allow_unpinned_env=True",
            )
        data = getattr(source, "data", source)
        if not isinstance(data, (bytes, bytearray)):
            data = Path(source).read_bytes()
        try:
            _, own_part, _ = identify(bytes(data))
        except ValueError:
            own_part = getattr(source, "part", None)
        if name is None:
            if own_part is None:
                raise WatchError("invalid_input", "pass name, e.g. 006-D8448-70.tmp or D844870.tmp")
            name = f"{own_part}.tmp"
        part = _part_of(name)
        if own_part is not None and own_part != part:
            raise WatchError("invalid_input",
                             f"the font is {own_part}, but the staging name {name} is for {part}")
        if garmin_sha256 is None:
            pin = font_registry.parts(font_registry.default()).get(part)
            garmin_sha256 = [pin.sha256] if pin is not None else []
        params = {"name": name, "dry_run": dry_run, "garmin_sha256": list(garmin_sha256)}
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / (Path(name).name or "source")
            path.write_bytes(data)
            return self.call("stage_file", params | {"source": str(path)}, progress)

    def remove_staged(self, staged_id):
        """`remove_staged` (dev): delete a file this tool staged, by its id."""
        return self.call("remove_staged", {"id": staged_id})

    def wait_for_watch(self, timeout_seconds=60, progress=None):
        """`wait_for_watch`: wait until a watch is connected; {model, waited_seconds}."""
        return self.call("wait_for_watch", {"timeout_seconds": timeout_seconds}, progress)


# The core's code for a JSON-RPC error without data.code.
_RPC_CODES = {-32700: "invalid_input", -32600: "invalid_input", -32601: "invalid_input",
              -32602: "invalid_input", -32800: "cancelled"}


def _part_of(name):
    """The part number a staging name stands for: `006-D8448-70.tmp`, or the short form
    `D844870.tmp` (006- and the dash dropped)."""
    stem = Path(name).stem.upper()
    if len(stem) == 7 and stem[0].isalpha() and stem[1:].isdigit():
        return f"006-{stem[:5]}-{stem[5:]}"
    return stem
