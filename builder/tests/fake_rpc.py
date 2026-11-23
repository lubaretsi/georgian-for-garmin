"""A stand-in for `georgian-for-garmin rpc` in the Watch client's tests: the same protocol (JSON-RPC
2.0, one message per line, errors with the core's code in data.code, -32700 for a malformed line,
progress and cancellation, and `notifications/trace` while `mtp_trace` is on) over a made-up
watch. Every message it receives is appended to $FAKE_RPC_LOG."""

import base64
import hashlib
import json
import os
import sys
from pathlib import Path

FILES = [
    {"name": "006-D9035-03.fnt", "handle": "0x01000001", "bytes": 35638, "hidden": True,
     "read_only": True, "sha256": None},
    {"name": "006-D8947-22.fnt", "handle": "0x01000002", "bytes": 22529, "hidden": True,
     "read_only": True, "sha256": None},
    {"name": "006-D8448-27.fnt", "handle": "0x01000003", "bytes": 6760129, "hidden": True,
     "read_only": True, "sha256": None},
]


def send(message):
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def log(message):
    if os.environ.get("FAKE_RPC_LOG"):
        with open(os.environ["FAKE_RPC_LOG"], "a") as handle:
            handle.write(json.dumps(message, ensure_ascii=False) + "\n")


def error(request_id, code, message, data=None):
    send({"jsonrpc": "2.0", "id": request_id,
          "error": {"code": code, "message": message, **({"data": data} if data else {})}})


def result(request_id, value):
    send({"jsonrpc": "2.0", "id": request_id, "result": value})


def extdata_list(params):
    offset, limit = params.get("offset", 0), params.get("limit", 200)
    wanted = {n.lower() for n in params.get("hash", [])}
    files = [dict(f, sha256=hashlib.sha256(f["name"].encode()).hexdigest()
                  if f["name"].lower() in wanted else None) for f in FILES]
    page = files[offset:offset + limit]
    following = offset + limit if offset + limit < len(files) else None
    return {"extdata": {"handle": "0x01000000", "storage_id": "0x00020001", "found_by": "path"},
            "hidden_listing": "listed", "note": None, "total": len(files), "offset": offset,
            "next_offset": following, "files": page, "output_file": None,
            "hash_not_found": sorted(wanted - {f["name"].lower() for f in FILES})}


# The reads ptp_transaction sends here, as the core's allowlist does (a few of them).
PTP_READS = {0x1001: "GetDeviceInfo", 0x1005: "GetStorageInfo", 0x1008: "GetObjectInfo",
             0x9002: "GarminGetObjectIds"}


def transaction(request_id, operation, data_in=0):
    """One `notifications/trace` for request `request_id`, as the core sends while tracing."""
    send({"jsonrpc": "2.0", "method": "notifications/trace", "params": {
        "opcode": 0, "operation": operation, "params": [], "response_code": 0x2001,
        "response": "OK", "response_params": [], "data_out_bytes": 0, "data_in_bytes": data_in,
        "duration_ms": 1.0, "error": None, "requestId": request_id}})


def encoded(raw):
    return {"bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
            "base64": base64.b64encode(raw).decode(), "path": None}


def main():
    tracing = False
    for line in sys.stdin:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            error(None, -32700, "Parse error")
            continue
        log(message)
        method, params, request_id = message.get("method"), message.get("params", {}), message.get("id")
        if request_id is None:
            continue  # a notification outside a request
        token = params.pop("_meta", {}).get("progressToken")
        # Refused before the watch is touched, as the core does: no transaction to trace.
        refused = (method == "ptp_transaction" and params.get("opcode") not in PTP_READS) or \
            (method in ("mtp_create", "mtp_delete") and params.get("allow_write") is not True)
        if tracing and method not in ("mtp_trace", "garble", "exit") and not refused:
            transaction(request_id, "GetStorageIDs")   # the session's check that it answers
        if refused:
            error(request_id, -32602, f"{method} refused: nothing was sent.",
                  {"code": "invalid_input"})
        elif method == "mtp_trace":
            tracing = params["enabled"]
            result(request_id, {"enabled": tracing})
        elif method == "mtp_handles":
            hidden = params.get("hidden", False)
            if tracing:
                transaction(request_id, "GarminGetObjectIds" if hidden else "GetObjectHandles", 12)
            result(request_id, {"handles": [0x01000001, 0x01000002] if hidden else []})
        elif method == "ptp_transaction":
            raw = b"answer to " + PTP_READS[params["opcode"]].encode()
            if tracing:
                transaction(request_id, PTP_READS[params["opcode"]], len(raw))
            result(request_id, {"opcode": params["opcode"],
                                "operation": PTP_READS[params["opcode"]], "response_code": 0x2001,
                                "response": "OK", "response_params": [], "data": encoded(raw)})
        elif method == "mtp_read":
            raw = b"<Device><Id>**********</Id></Device>"
            if tracing:
                transaction(request_id, "GetObject", len(raw))
            result(request_id, {"handle": params["handle"], "offset": 0, "data": encoded(raw)})
        elif method == "inspect":
            result(request_id, {"model": "Venu 2", "model_part": "006-B3703-00", "software": "19.05"})
        elif method == "extdata_list":
            result(request_id, extdata_list(params))
        elif method == "install":
            error(request_id, -32000, "No watch is connected.", {"code": "no_device"})
        elif method == "restore_stock":
            error(request_id, -32000, "Garmin Express holds the watch.",
                  {"code": "device_busy", "busy": {"holder": "garmin_express"}})
        elif method == "stage_file":
            data = Path(params["source"]).read_bytes()
            result(request_id, {
                "status": "dry_run" if params.get("dry_run") else "staged_verified",
                "action": "create", "id": None if params.get("dry_run") else "staged-1",
                "name": params["name"], "path": f"GARMIN/ExtData/{params['name']}",
                "part": params["name"].split(".")[0], "handle": None, "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(), "replaces": None,
                "garmin_sha256": params.get("garmin_sha256"),
            })
        elif method == "remove_staged":
            result(request_id, {"id": params["id"], "status": "removed", "name": "x.tmp",
                                "path": "GARMIN/ExtData/x.tmp"})
        elif method == "wait_for_watch":
            if token is not None:
                for step in (1, 2):
                    send({"jsonrpc": "2.0", "method": "notifications/progress",
                          "params": {"progressToken": token, "progress": step, "total": 2,
                                     "message": f"waiting {step}"}})
                    if tracing:
                        transaction(request_id, f"poll {step}")
            result(request_id, {"model": "Venu 2", "waited_seconds": 1})
        elif method == "slow":
            # Two traces and two progress steps, then it waits: a callback that fails at the
            # first must not be called for the second.
            for step in (1, 2):
                if tracing:
                    transaction(request_id, f"GetObject {step}")
                if token is not None:
                    send({"jsonrpc": "2.0", "method": "notifications/progress",
                          "params": {"progressToken": token, "progress": step}})
            # Answers only once cancelled, as a long operation does.
            for following in sys.stdin:
                cancel = json.loads(following)
                log(cancel)
                if cancel.get("method") == "notifications/cancelled" and \
                        cancel["params"]["requestId"] == request_id:
                    error(request_id, -32800, "Cancelled.", {"code": "cancelled"})
                    break
        elif method == "garble":
            sys.stdout.write("this is not JSON\n")
            sys.stdout.flush()
        elif method == "exit":
            sys.exit(3)
        else:
            error(request_id, -32601, f"unknown method {method}", {"code": "invalid_input"})


if __name__ == "__main__":
    main()
