//! `georgian-for-garmin rpc`: JSON-RPC 2.0, one message per line, over a [`Worker`].
//!
//! Method = operation name, params = its input (`params._meta` is protocol metadata, removed
//! before validation), result = its output. Requests run one at a time, in the order they
//! arrive; the loop keeps reading meanwhile, so `notifications/cancelled` (`requestId`) reaches
//! a running call. A request whose params carry `_meta.progressToken` gets
//! `notifications/progress` for each step. With the `dev` feature, while `mtp_trace` is on, a
//! request's PTP transactions come as `notifications/trace` (the transaction, and the request's
//! id as `requestId`), before its response. Every request gets a response, a cancelled one
//! included (error -32800). Errors carry the operation's code in `data.code`. A line that isn't
//! JSON gets -32700 and the loop goes on. Batches (arrays) aren't supported (-32600). At the end
//! of input, the calls already read still run and are answered, so a script can pipe its
//! requests in; then the session ends.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value, json};

use crate::error::{Error, ErrorCode};
use crate::ops;
use crate::session::{Job, Progress, Worker};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
/// LSP's `RequestCancelled`: the call stopped on `notifications/cancelled`.
pub const REQUEST_CANCELLED: i64 = -32800;
/// Any other operation error; `data.code` says which.
pub const OPERATION_FAILED: i64 = -32000;

/// The JSON-RPC error code for an operation error.
pub fn error_code(code: ErrorCode) -> i64 {
    match code {
        ErrorCode::InvalidInput => INVALID_PARAMS,
        ErrorCode::Cancelled => REQUEST_CANCELLED,
        _ => OPERATION_FAILED,
    }
}

/// Where messages go: one line each, written whole.
#[derive(Clone)]
struct Out(Arc<Mutex<Box<dyn Write + Send>>>);

impl Out {
    fn send(&self, message: &Value) {
        let mut line = message.to_string();
        line.push('\n');
        let mut out = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        // A closed output means the client is gone; the end of input stops the loop.
        let _ = out.write_all(line.as_bytes()).and_then(|()| out.flush());
    }

    fn fail(&self, id: Value, code: i64, error: &Error) {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": code, "message": error.message, "data": error.to_json()["error"]},
        }));
    }

    fn respond(&self, id: Value, result: Result<Value, Error>) {
        match result {
            Ok(result) => self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result})),
            Err(e) => self.fail(id, error_code(e.code), &e),
        }
    }

    fn progress(&self, token: &Value, progress: Progress) {
        let mut params = json!({
            "progressToken": token,
            "progress": progress.progress,
            "message": progress.message,
        });
        if let Some(total) = progress.total {
            params["total"] = json!(total);
        }
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": params}));
    }

    /// One PTP transaction of request `id`, while the session's trace is on.
    #[cfg(feature = "dev")]
    fn trace(&self, id: &Value, transaction: &crate::trace::Transaction) {
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/trace",
                          "params": trace_params(id, transaction)}));
    }
}

/// `notifications/trace`'s params: the transaction's fields, and `requestId`.
#[cfg(feature = "dev")]
pub fn trace_params(id: &Value, transaction: &crate::trace::Transaction) -> Value {
    let mut params = json!(transaction);
    params["requestId"] = id.clone();
    params
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::InvalidInput, message)
}

type Pending = Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>;

/// Serves requests from `input` until it ends, then lets `worker` finish.
pub fn serve(mut input: impl BufRead, output: impl Write + Send + 'static, worker: Worker) {
    let out = Out(Arc::new(Mutex::new(Box::new(output))));
    let pending: Pending = Arc::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        match input.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        handle(&line, &out, &pending, &worker);
    }
    worker.finish();
}

fn handle(line: &[u8], out: &Out, pending: &Pending, worker: &Worker) {
    let message: Value = match serde_json::from_slice(line) {
        Ok(message) => message,
        Err(e) => {
            return out.fail(
                Value::Null,
                PARSE_ERROR,
                &invalid(format!("Parse error: {e}")),
            );
        }
    };
    let Value::Object(mut message) = message else {
        return out.fail(
            Value::Null,
            INVALID_REQUEST,
            &invalid("Invalid Request: a message is one JSON object; batches aren't supported"),
        );
    };
    let id = message.remove("id");
    if let Some(id) = &id
        && !(id.is_string() || id.is_number() || id.is_null())
    {
        return out.fail(
            Value::Null,
            INVALID_REQUEST,
            &invalid("Invalid Request: id must be a string, a number or null"),
        );
    }
    let method = match (message.get("jsonrpc"), message.get("method")) {
        (Some(Value::String(v)), Some(Value::String(m))) if v == "2.0" => m.clone(),
        _ => {
            // A notification that isn't one is dropped: nobody waits for an answer.
            if let Some(id) = id {
                out.fail(
                    id,
                    INVALID_REQUEST,
                    &invalid("Invalid Request: needs \"jsonrpc\": \"2.0\" and a method"),
                );
            }
            return;
        }
    };
    let params = message.remove("params");
    let Some(id) = id else {
        if method == "notifications/cancelled"
            && let Some(request) = params.as_ref().and_then(|p| p.get("requestId"))
            && let Some(flag) = pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&request.to_string())
        {
            flag.store(true, Ordering::SeqCst);
        }
        return;
    };
    let mut params = match params {
        None => Map::new(),
        Some(Value::Object(params)) => params,
        Some(_) => {
            return out.fail(
                id,
                INVALID_PARAMS,
                &invalid("params must be an object keyed like the operation's input schema"),
            );
        }
    };
    let token = params
        .remove("_meta")
        .and_then(|meta| meta.get("progressToken").cloned())
        .filter(|t| t.is_string() || t.is_number());
    if ops::operation(&method).is_none() {
        return out.fail(
            id,
            METHOD_NOT_FOUND,
            &invalid(format!("Method not found: {method}")),
        );
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let key = id.to_string();
    pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key.clone(), cancel.clone());
    let progress = token.map(|token| {
        let out = out.clone();
        Box::new(move |p: Progress| out.progress(&token, p)) as Box<dyn Fn(Progress) + Send>
    });
    #[cfg(feature = "dev")]
    let trace: crate::trace::Sink = {
        let (out, id) = (out.clone(), id.clone());
        Arc::new(move |t: &crate::trace::Transaction| out.trace(&id, t))
    };
    let done = {
        let (out, pending) = (out.clone(), pending.clone());
        Box::new(move |result: Result<Value, Error>| {
            pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&key);
            out.respond(id, result);
        })
    };
    worker.submit(Job {
        name: method,
        input: Value::Object(params),
        progress,
        cancel,
        #[cfg(feature = "dev")]
        trace: Some(trace),
        done,
    });
}
