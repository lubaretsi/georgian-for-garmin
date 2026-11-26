//! `georgian-for-garmin mcp`: the operations as MCP tools over stdio, with the official Rust SDK
//! (`rmcp`). Tools come from the core's declarations; calls run one at a time on a session that
//! keeps the watch open. A call whose `_meta` carries a `progressToken` gets
//! `notifications/progress`; `notifications/cancelled` stops it. With the `dev` feature, while
//! `mtp_trace` is on, a call's PTP transactions come as `notifications/trace`, as in `rpc`. An
//! operation error is a tool result with `isError` and `{"error": {"code", "message"}}` as its
//! structured content.

use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use georgian_for_garmin_core::session::{Job, Progress, Worker};
use georgian_for_garmin_core::{Error, ops};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, InitializeResult,
    ListToolsResult, PaginatedRequestParams, ProgressNotificationParam, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::Value;

/// What a running call sends before its result.
enum Update {
    Progress(Progress),
    /// `notifications/trace`'s params.
    #[cfg(feature = "dev")]
    Trace(Value),
}

pub struct Server {
    worker: Worker,
    tools: Vec<Tool>,
}

impl Server {
    pub fn new(worker: Worker) -> Self {
        let tools = ops::operations()
            .map(|op| serde_json::from_value(op.tool()).expect("declarations are MCP tools"))
            .collect();
        Server { worker, tools }
    }
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("georgian-for-garmin", ops::APP_VERSION))
            .with_instructions(
                "Install and inspect a Georgian-extended font on a Garmin watch over USB. Calls \
                 run one at a time and keep the watch open between them; another program can't \
                 use the watch while this server runs. After install or restore_stock, unplug \
                 the watch and restart it; wait_for_watch waits for it to come back.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.clone()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.to_string();
        if ops::operation(&name).is_none() {
            return Err(ErrorData::invalid_params(
                format!("Unknown tool: {name}"),
                None,
            ));
        }
        let input = Value::Object(request.arguments.unwrap_or_default());
        let token = context.meta.get_progress_token();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel::<Update>();
        let progress = token.is_some().then(|| {
            let update_tx = update_tx.clone();
            Box::new(move |p: Progress| {
                let _ = update_tx.send(Update::Progress(p));
            }) as Box<dyn Fn(Progress) + Send>
        });
        #[cfg(feature = "dev")]
        let trace: georgian_for_garmin_core::trace::Sink = {
            let id = serde_json::to_value(&context.id).unwrap_or(Value::Null);
            Arc::new(move |t: &georgian_for_garmin_core::trace::Transaction| {
                let params = georgian_for_garmin_core::rpc::trace_params(&id, t);
                let _ = update_tx.send(Update::Trace(params));
            })
        };
        #[cfg(not(feature = "dev"))]
        drop(update_tx);
        let cancel = Arc::new(AtomicBool::new(false));
        self.worker.submit(Job {
            name,
            input,
            progress,
            cancel: cancel.clone(),
            #[cfg(feature = "dev")]
            trace: Some(trace),
            done: Box::new(move |result| {
                let _ = result_tx.send(result);
            }),
        });
        let notify = |update: Update| {
            let (peer, token) = (context.peer.clone(), token.clone());
            async move {
                match update {
                    Update::Progress(p) => {
                        let Some(token) = token else { return };
                        let param = ProgressNotificationParam::new(token, p.progress)
                            .with_message(p.message);
                        let param = match p.total {
                            Some(total) => param.with_total(total),
                            None => param,
                        };
                        let _ = peer.notify_progress(param).await;
                    }
                    #[cfg(feature = "dev")]
                    Update::Trace(params) => {
                        let trace = rmcp::model::CustomNotification::new(
                            "notifications/trace",
                            Some(params),
                        );
                        let notification =
                            rmcp::model::ServerNotification::CustomNotification(trace);
                        let _ = peer.send_notification(notification).await;
                    }
                }
            }
        };
        let outcome = relay(
            result_rx,
            &mut update_rx,
            context.ct.cancelled(),
            &cancel,
            notify,
        )
        .await;
        match outcome {
            Ok(Ok(output)) => Ok(CallToolResult::structured(output).into()),
            Ok(Err(e)) => Ok(CallToolResult::structured_error(e.to_json()).into()),
            Err(_) => Err(ErrorData::internal_error(
                "the session thread stopped",
                None,
            )),
        }
    }
}

/// Forwards an operation's updates until its result comes, and stops it when `cancelled`
/// completes. The worker sends every update before the result, but both can be ready at once:
/// what is still queued then is delivered before the result is returned, so a client sees every
/// update, the last one included.
async fn relay<U, N, F>(
    result: tokio::sync::oneshot::Receiver<Result<Value, Error>>,
    progress: &mut tokio::sync::mpsc::UnboundedReceiver<U>,
    cancelled: impl Future<Output = ()>,
    cancel: &AtomicBool,
    mut notify: N,
) -> Result<Result<Value, Error>, tokio::sync::oneshot::error::RecvError>
where
    N: FnMut(U) -> F,
    F: Future<Output = ()>,
{
    let mut result = pin!(result);
    let mut cancelled = pin!(cancelled);
    let outcome = loop {
        tokio::select! {
            outcome = &mut result => break outcome,
            Some(p) = progress.recv() => notify(p).await,
            () = &mut cancelled, if !cancel.load(Ordering::SeqCst) => {
                cancel.store(true, Ordering::SeqCst);
            }
        }
    };
    while let Ok(p) = progress.try_recv() {
        notify(p).await;
    }
    outcome
}

/// Serves MCP on `transport` until the client closes it.
pub async fn serve<T, E, A>(worker: Worker, transport: T) -> Result<(), String>
where
    T: IntoTransport<RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let running = Server::new(worker)
        .serve(transport)
        .await
        .map_err(|e| format!("MCP session failed to start: {e}"))?;
    running
        .waiting()
        .await
        .map(drop)
        .map_err(|e| format!("MCP session failed: {e}"))
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::time::Duration;

    use georgian_for_garmin_core::garmin_mode::{SwitchOutput, SwitchStatus};
    use georgian_for_garmin_core::session::{Control, Session};
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, Lines};

    use super::*;
    use crate::fake::{self, Watch};

    struct Client {
        lines: Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
        writer: tokio::io::WriteHalf<DuplexStream>,
        /// Progress notifications seen so far: (token, progress, total).
        progress: Vec<(Value, f64, Option<f64>)>,
    }

    impl Client {
        async fn send(&mut self, message: Value) {
            let mut line = message.to_string();
            line.push('\n');
            self.writer.write_all(line.as_bytes()).await.unwrap();
        }

        async fn next(&mut self) -> Value {
            let line = tokio::time::timeout(Duration::from_secs(10), self.lines.next_line())
                .await
                .expect("a message within 10 s")
                .unwrap()
                .expect("the server is still there");
            serde_json::from_str(&line).unwrap()
        }

        /// The response to `id`, recording notifications on the way; `on_progress` runs at
        /// each progress notification.
        async fn response(&mut self, id: i64) -> Value {
            loop {
                let message = self.next().await;
                if message["method"] == "notifications/progress" {
                    let p = &message["params"];
                    self.progress.push((
                        p["progressToken"].clone(),
                        p["progress"].as_f64().unwrap(),
                        p["total"].as_f64(),
                    ));
                } else if message["id"] == id {
                    return message;
                }
            }
        }

        async fn call(&mut self, id: i64, name: &str, arguments: Value) -> Value {
            self.send(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
                             "params": {"name": name, "arguments": arguments}}))
                .await;
            self.response(id).await
        }
    }

    /// `switch_to_mtp` here never reaches USB: the watch comes back at once.
    fn switched(_: &Control<'_>) -> Result<SwitchOutput, georgian_for_garmin_core::Error> {
        Ok(SwitchOutput {
            status: SwitchStatus::Switched,
            product: None,
            protocols: vec!["A1050".into()],
            waited_seconds: 4.1,
        })
    }

    async fn start(watch: &Watch) -> Client {
        let (client, server) = tokio::io::duplex(1 << 20);
        let watch = watch.clone();
        let worker = Worker::spawn(move || {
            Session::new(Cow::Owned(fake::fonts()), Box::new(move || watch.connect()))
                .with_poll_interval(Duration::from_millis(10))
                .with_switch(Box::new(switched))
        });
        tokio::spawn(async move { serve(worker, server).await.unwrap() });
        let (reader, writer) = tokio::io::split(client);
        let mut client = Client {
            lines: BufReader::new(reader).lines(),
            writer,
            progress: Vec::new(),
        };
        client
            .send(
                json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": {"name": "test", "version": "1"}}}),
            )
            .await;
        let init = client.response(0).await;
        assert_eq!(init["result"]["serverInfo"]["name"], "georgian-for-garmin");
        assert!(
            init["result"]["capabilities"]["tools"].is_object(),
            "{init}"
        );
        client
            .send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        client
    }

    #[tokio::test]
    async fn tools_are_the_declarations() {
        let watch = Watch::venu2();
        let mut client = start(&watch).await;
        client
            .send(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
            .await;
        let list = client.response(1).await;
        let tools = list["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        let expected: Vec<&str> = ops::operations().map(|o| o.name).collect();
        assert_eq!(names, expected);
        for (tool, op) in tools.iter().zip(ops::operations()) {
            assert_eq!(tool["inputSchema"], op.input_schema());
            assert_eq!(tool["annotations"]["readOnlyHint"], op.read_only);
            assert_eq!(tool["annotations"]["destructiveHint"], op.destructive);
        }

        let out = client.call(2, "extdata_list", json!({})).await;
        assert_eq!(out["result"]["isError"], false);
        assert_eq!(out["result"]["structuredContent"]["total"], 2);

        let out = client.call(3, "extdata_list", json!({"limit": 0})).await;
        assert_eq!(out["result"]["isError"], true);
        assert_eq!(
            out["result"]["structuredContent"]["error"]["code"],
            "invalid_input"
        );

        let out = client.call(4, "no_such_tool", json!({})).await;
        assert_eq!(out["error"]["code"], -32602);
        if !cfg!(feature = "dev") {
            let out = client.call(5, "stage_file", json!({})).await;
            assert_eq!(out["error"]["code"], -32602);
        }
    }

    #[tokio::test]
    async fn switch_to_mtp_is_a_tool_that_changes_the_watch() {
        let watch = Watch::venu2();
        let mut client = start(&watch).await;
        let out = client.call(1, "switch_to_mtp", json!({})).await;
        assert_eq!(out["result"]["isError"], false, "{out}");
        assert_eq!(out["result"]["structuredContent"]["status"], "switched");
        let out = client
            .call(2, "switch_to_mtp", json!({"retry": true}))
            .await;
        assert_eq!(
            out["result"]["structuredContent"]["error"]["code"],
            "invalid_input"
        );
        let tool = ops::operation("switch_to_mtp").unwrap().tool();
        assert_eq!(tool["annotations"]["readOnlyHint"], false);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
    }

    /// The worker's last updates and its result ready at once: every update still arrives,
    /// in order, before the result. With both ready, `select!` picks a branch at random, so
    /// many rounds make a lost update certain to show.
    #[tokio::test]
    async fn every_update_sent_before_the_result_is_delivered_first() {
        for _ in 0..200 {
            let (result_tx, result_rx) = tokio::sync::oneshot::channel();
            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
            for step in 0..5 {
                let p = Progress {
                    progress: f64::from(step),
                    total: Some(4.0),
                    message: String::new(),
                };
                progress_tx.send(p).unwrap();
            }
            result_tx
                .send(Ok(json!({"status": "staged_verified"})))
                .unwrap();
            let seen = std::cell::RefCell::new(Vec::new());
            let cancel = AtomicBool::new(false);
            let outcome = relay(
                result_rx,
                &mut progress_rx,
                std::future::pending(),
                &cancel,
                |p| {
                    seen.borrow_mut().push(p.progress);
                    async {}
                },
            )
            .await;
            assert!(outcome.unwrap().is_ok());
            assert_eq!(seen.into_inner(), [0.0, 1.0, 2.0, 3.0, 4.0]);
        }
    }

    /// End to end over MCP: every call's full sequence comes before its result.
    #[tokio::test]
    async fn every_call_gets_all_of_its_progress_before_its_result() {
        let watch = Watch::venu2();
        let mut client = start(&watch).await;
        for id in 1..=40 {
            let name = if id % 2 == 1 {
                "install"
            } else {
                "restore_stock"
            };
            client.progress.clear();
            client
                .send(
                    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
                    "name": name, "arguments": {}, "_meta": {"progressToken": id}}}),
                )
                .await;
            let out = client.response(id).await;
            assert_eq!(out["result"]["isError"], false, "{out}");
            let steps: Vec<f64> = client.progress.iter().map(|p| p.1).collect();
            assert_eq!(steps, [0.0, 1.0, 2.0, 3.0, 4.0], "call {id}");
        }
    }

    #[tokio::test]
    async fn progress_cancellation_and_reconnect() {
        let watch = Watch::venu2();
        let mut client = start(&watch).await;
        client
            .send(
                json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
                "name": "install", "arguments": {}, "_meta": {"progressToken": "p1"}}}),
            )
            .await;
        let out = client.response(1).await;
        assert_eq!(
            out["result"]["structuredContent"]["status"],
            "staged_verified"
        );
        let steps: Vec<f64> = client.progress.iter().map(|p| p.1).collect();
        assert_eq!(steps, [0.0, 1.0, 2.0, 3.0, 4.0]);
        assert!(
            client
                .progress
                .iter()
                .all(|p| p.0 == "p1" && p.2 == Some(4.0))
        );

        // restore_stock, cancelled after its first file.
        client.progress.clear();
        watch.state().write_delay = Duration::from_millis(300);
        client
            .send(
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                "name": "restore_stock", "arguments": {}, "_meta": {"progressToken": 7}}}),
            )
            .await;
        loop {
            let message = client.next().await;
            if message["method"] == "notifications/progress" && message["params"]["progress"] == 1.0
            {
                break;
            }
            assert_ne!(
                message["id"], 2,
                "finished before it was cancelled: {message}"
            );
        }
        client
            .send(
                json!({"jsonrpc": "2.0", "method": "notifications/cancelled",
                         "params": {"requestId": 2, "reason": "test"}}),
            )
            .await;
        // Calls run in order: this one answers once the cancelled one has stopped.
        let out = client.call(3, "extdata_list", json!({})).await;
        assert_eq!(out["result"]["isError"], false);
        let georgian: Vec<(String, Vec<u8>)> = fake::ARMENIAN
            .iter()
            .map(|p| (format!("{p}.tmp"), fake::georgian(p)))
            .collect();
        let mut listed = watch.listed();
        listed.sort();
        assert_eq!(listed, georgian, "restore was rolled back");
        watch.state().write_delay = Duration::ZERO;

        // Unplugged: no_device; plugged back: the session opens it again.
        watch.unplug();
        let out = client.call(4, "inspect", json!({})).await;
        assert_eq!(
            out["result"]["structuredContent"]["error"]["code"],
            "no_device"
        );
        watch.plug();
        let out = client
            .call(5, "wait_for_watch", json!({"timeout_seconds": 5}))
            .await;
        assert_eq!(out["result"]["structuredContent"]["model"], "Venu 2");
        let out = client.call(6, "inspect", json!({})).await;
        assert_eq!(out["result"]["isError"], false, "{out}");
    }

    /// With the trace on, a call's transactions come as `notifications/trace`, in order, before
    /// its result, carrying its request id; off, none come.
    #[cfg(feature = "dev")]
    #[tokio::test]
    async fn traced_calls_send_their_transactions_before_the_result() {
        let watch = Watch::venu2();
        let mut client = start(&watch).await;
        let out = client.call(1, "mtp_trace", json!({"enabled": true})).await;
        assert_eq!(out["result"]["structuredContent"]["enabled"], true);
        for id in 2..=4 {
            client
                .send(
                    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
                    "name": "ptp_transaction",
                    "arguments": {"opcode": 0x9002, "params": [fake::STORAGE, fake::EXTDATA]}}}),
                )
                .await;
            let mut traces = Vec::new();
            let out = loop {
                let message = client.next().await;
                if message["method"] == "notifications/trace" {
                    assert_eq!(message["params"]["requestId"], id, "{message}");
                    traces.push(message["params"]["operation"].clone());
                } else {
                    break message;
                }
            };
            assert_eq!(out["id"], id);
            assert_eq!(
                out["result"]["structuredContent"]["response"], "OK",
                "{out}"
            );
            let mut expected = vec![json!("GetStorageIDs"), json!("GarminGetObjectIds")];
            if id == 2 {
                expected.remove(0); // the first call opens the watch; no check before it
            }
            assert_eq!(traces, expected, "call {id}");
        }
        client.call(5, "mtp_trace", json!({"enabled": false})).await;
        let out = client.call(6, "mtp_storage_ids", json!({})).await;
        assert_eq!(out["result"]["isError"], false);
        client
            .send(json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
                         "params": {"name": "mtp_storage_ids", "arguments": {}}}))
            .await;
        let next = client.next().await;
        assert_eq!(next["id"], 7, "no trace while off: {next}");
    }
}
