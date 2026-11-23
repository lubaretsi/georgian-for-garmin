"""A stand-in for `georgian-for-garmin rpc` in the Watch client's tests: the same protocol (JSON-RPC
2.0, one message per line, errors with the core's code in data.code, -32700 for a malformed line,
progress and cancellation) over a made-up watch. Every message it receives is appended to
$FAKE_RPC_LOG."""

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


def main():
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
        if method == "inspect":
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
            result(request_id, {"model": "Venu 2", "waited_seconds": 1})
        elif method == "slow":
            # Answers only once cancelled, as a long operation does.
            for following in sys.stdin:
                cancel = json.loads(following)
                log(cancel)
                if cancel.get("method") == "notifications/cancelled" and \
                        cancel["params"]["requestId"] == request_id:
                    error(request_id, -32800, "Cancelled.", {"code": "cancelled"})
                    break
        elif method == "exit":
            sys.exit(3)
        else:
            error(request_id, -32601, f"unknown method {method}", {"code": "invalid_input"})


if __name__ == "__main__":
    main()
