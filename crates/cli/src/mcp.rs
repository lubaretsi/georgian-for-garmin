//! `georgian-for-garmin mcp`: the operations as MCP tools over stdio, with the official Rust SDK
//! (`rmcp`). Tools come from the core's declarations; calls run one at a time on a session that
//! keeps the watch open. A call whose `_meta` carries a `progressToken` gets
//! `notifications/progress`; `notifications/cancelled` stops it. An operation error is a tool
//! result with `isError` and `{"error": {"code", "message"}}` as its structured content.

use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use georgian_for_garmin_core::ops;
use georgian_for_garmin_core::session::{Job, Progress, Worker};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, InitializeResult,
    ListToolsResult, PaginatedRequestParams, ProgressNotificationParam, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::Value;

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
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<Progress>();
        let progress = token.is_some().then(|| {
            Box::new(move |p: Progress| {
                let _ = progress_tx.send(p);
            }) as Box<dyn Fn(Progress) + Send>
        });
        let cancel = Arc::new(AtomicBool::new(false));
        self.worker.submit(Job {
            name,
            input,
            progress,
            cancel: cancel.clone(),
            done: Box::new(move |result| {
                let _ = result_tx.send(result);
            }),
        });
        let mut result_rx = pin!(result_rx);
        let outcome = loop {
            tokio::select! {
                result = &mut result_rx => break result,
                Some(p) = progress_rx.recv() => {
                    if let Some(token) = &token {
                        let mut param = ProgressNotificationParam::new(token.clone(), p.progress)
                            .with_message(p.message);
                        if let Some(total) = p.total {
                            param = param.with_total(total);
                        }
                        let _ = context.peer.notify_progress(param).await;
                    }
                }
                () = context.ct.cancelled(), if !cancel.load(Ordering::SeqCst) => {
                    cancel.store(true, Ordering::SeqCst);
                }
            }
        };
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

    use georgian_for_garmin_core::session::Session;
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

    async fn start(watch: &Watch) -> Client {
        let (client, server) = tokio::io::duplex(1 << 20);
        let watch = watch.clone();
        let worker = Worker::spawn(move || {
            Session::new(Cow::Owned(fake::fonts()), Box::new(move || watch.connect()))
                .with_poll_interval(Duration::from_millis(10))
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
}
