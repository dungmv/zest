use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use zest_core::graphql::{parse_introspection, GraphQLSchemaSummary, INTROSPECTION_QUERY};
use zest_core::http::{HttpExecutor, HttpProgress, ReqwestExecutor};
use zest_core::models::{Environment, HttpMethod, Protocol, RequestBody, RequestItem};
use zest_core::scripting::{PostResponseContext, PostResponseOutput, ScriptCancel, ScriptSandbox};
use zest_core::sse::SseOutput;
use zest_core::transport::{
    FrameDirection, FrameKind, RequestError, ResolvedRequest, TransportFrame,
};

use super::pipeline::{merge_env_mutations, run_pre_request_stage};
use super::send_event::SendEvent;

/// Per-request SSE frames kept engine-side so the view survives request
/// switches while a stream is still running (mirrors `GrpcEngine`).
const MAX_SSE_LOG_FRAMES: usize = 1000;

pub struct HttpEngine {
    executor: Arc<ReqwestExecutor>,
    tokio_rt: Arc<tokio::runtime::Runtime>,
    /// Bounded per-request SSE frame log for live-stream restoration.
    sse_logs: Mutex<HashMap<String, Vec<TransportFrame>>>,
}

impl HttpEngine {
    pub fn new(executor: Arc<ReqwestExecutor>, tokio_rt: Arc<tokio::runtime::Runtime>) -> Self {
        Self {
            executor,
            tokio_rt,
            sse_logs: Mutex::new(HashMap::new()),
        }
    }

    /// Appends one inbound SSE frame to the engine-side log.
    pub fn register_sse_frame(&self, request_id: &str, frame: TransportFrame) {
        let mut logs = self.sse_logs.lock().unwrap_or_else(|e| e.into_inner());
        let log = logs.entry(request_id.to_string()).or_default();
        log.push(frame);
        if log.len() > MAX_SSE_LOG_FRAMES {
            log.drain(..log.len() - MAX_SSE_LOG_FRAMES);
        }
    }

    /// Snapshot of the SSE frames for a request (oldest first).
    pub fn snapshot_sse_frames(&self, request_id: &str) -> Option<Vec<TransportFrame>> {
        let logs = self.sse_logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.get(request_id).cloned()
    }

    /// Drops the SSE log of a request before a new stream starts.
    pub fn clear_sse_frames(&self, request_id: &str) {
        let mut logs = self.sse_logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.remove(request_id);
    }

    /// Starts the HTTP pipeline on the tokio runtime and returns the event
    /// receiver the Runner must pump. Cancel via the usual oneshot channel.
    /// For SSE requests the response body is parsed into per-event
    /// [`SendEvent::Frame`] items and the final record carries the capped
    /// accumulated body.
    pub fn execute(
        &self,
        mut req_item: RequestItem,
        env: Option<Arc<Environment>>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> mpsc::UnboundedReceiver<SendEvent> {
        if req_item.protocol == Protocol::Sse {
            self.clear_sse_frames(&req_item.id);
        }
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let executor = self.executor.clone();
        let send_event_tx = event_tx.clone();

        self.tokio_rt.spawn(async move {
            // 0. Bridge the one-shot cancel into a shared flag so long-running
            //    user scripts (pre/post) can be interrupted mid-eval, even
            //    after the transport phase has already completed. The
            //    transport keeps a dedicated one-shot to preserve the
            //    existing cancel semantics (send OR sender-drop).
            let script_cancel = ScriptCancel::new();
            let (transport_cancel_tx, transport_cancel_rx) = oneshot::channel();
            tokio::spawn({
                let flag = script_cancel.clone();
                async move {
                    let cancelled = cancel_rx.await.is_ok();
                    if cancelled {
                        flag.cancel();
                    }
                    let _ = transport_cancel_tx.send(());
                }
            });

            // 1. Resolve dynamic fake variables + run the pre-request script
            let pre_output =
                run_pre_request_stage(&mut req_item, env.as_deref(), Some(&script_cancel));
            let post_script_code = req_item
                .script
                .as_ref()
                .and_then(|s| s.post_response.clone());

            let _ = send_event_tx.send(SendEvent::Started {
                request_id: req_item.id.clone(),
                initial_logs: pre_output.console_entries.clone(),
                env_mutations: pre_output.env_mutations.clone(),
                pre_script_error: pre_output.error.clone(),
            });

            // 2. Merge pre-script env mutations into the effective environment
            let effective_env = merge_env_mutations(&env, &pre_output.env_mutations);

            // 3. Resolve the wire request with the effective environment and
            //    apply pre-script header/body mutations
            let mut resolved = match ResolvedRequest::from_item(&req_item, effective_env.as_deref())
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = send_event_tx.send(SendEvent::Failed {
                        request_id: req_item.id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            };
            resolved.apply_pre_script_mutations(&pre_output);

            let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
            let progress_event_tx = send_event_tx.clone();

            tokio::spawn(async move {
                while let Some(prog) = progress_rx.recv().await {
                    let text = match prog {
                        HttpProgress::Connecting => "Connecting...".to_string(),
                        HttpProgress::Sending => "Sending...".to_string(),
                        HttpProgress::WaitingResponse => "Waiting for response...".to_string(),
                        HttpProgress::Downloading {
                            received_bytes,
                            total_bytes,
                        } => {
                            if let Some(total) = total_bytes {
                                format!("Downloading ({}/{} B)...", received_bytes, total)
                            } else {
                                format!("Downloading ({} B)...", received_bytes)
                            }
                        }
                    };
                    let _ = progress_event_tx.send(SendEvent::Progress(text));
                }
            });

            let eff_env_for_post = effective_env.or(env);
            let pre_console_entries = pre_output.console_entries.clone();
            let request_id = req_item.id.clone();

            let result = if req_item.protocol == Protocol::Sse {
                Self::run_sse_stream(
                    executor,
                    resolved,
                    progress_tx,
                    send_event_tx.clone(),
                    transport_cancel_rx,
                )
                .await
            } else {
                tokio::select! {
                    _ = transport_cancel_rx => Err(RequestError::Cancelled),
                    res = executor.send(resolved, Some(progress_tx)) => res,
                }
            };

            match result {
                Ok(mut record) => {
                    // 4. Post-response / Test Script
                    let post_output = if let Some(code) = post_script_code {
                        let post_ctx = PostResponseContext::from_response_and_env(
                            &record,
                            eff_env_for_post.as_deref(),
                        );
                        ScriptSandbox::run_post_response(&code, post_ctx, Some(&script_cancel))
                    } else {
                        PostResponseOutput::default()
                    };

                    record.test_results = post_output.test_results.clone();
                    let mut combined_console_logs = pre_console_entries;
                    combined_console_logs.extend(post_output.console_entries.clone());
                    record.console_logs = combined_console_logs;

                    let _ = send_event_tx.send(SendEvent::HttpSuccess {
                        record: Arc::new(record),
                        post_output: Box::new(post_output),
                    });
                }
                Err(RequestError::Cancelled) => {
                    let _ = send_event_tx.send(SendEvent::Cancelled);
                }
                Err(e) => {
                    let _ = send_event_tx.send(SendEvent::Failed {
                        request_id,
                        error: e.to_string(),
                    });
                }
            }
        });

        event_rx
    }

    /// Streams an SSE response: forwards every parsed event/keep-alive as
    /// [`SendEvent::Frame`] (with periodic progress counts), then resolves
    /// with the final record. Cancel is handled inside `send_streaming`.
    async fn run_sse_stream(
        executor: Arc<ReqwestExecutor>,
        resolved: ResolvedRequest,
        progress_tx: mpsc::UnboundedSender<HttpProgress>,
        send_event_tx: mpsc::UnboundedSender<SendEvent>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> Result<zest_core::models::HttpResponseRecord, RequestError> {
        let (sse_tx, mut sse_rx) = mpsc::unbounded_channel::<SseOutput>();
        let request_id = resolved.request_id.clone();

        // Forward parsed SSE outputs to the unified channel as frames,
        // assigning frame ids and emitting throttled progress updates.
        let frame_pump = tokio::spawn(async move {
            let mut next_id = 0u64;
            let mut event_count = 0usize;
            while let Some(out) = sse_rx.recv().await {
                let (mut frame, event_name) = match out {
                    SseOutput::Event(event) => {
                        event_count += 1;
                        (
                            TransportFrame::new(
                                next_id,
                                chrono::Utc::now().timestamp_millis(),
                                FrameDirection::Inbound,
                                FrameKind::Text,
                                event.data.into_bytes(),
                            ),
                            event.event,
                        )
                    }
                    // Keep-alive comments surface as pings, like WS.
                    SseOutput::Comment(text) => (
                        TransportFrame::new(
                            next_id,
                            chrono::Utc::now().timestamp_millis(),
                            FrameDirection::Inbound,
                            FrameKind::Ping,
                            text.into_bytes(),
                        ),
                        None,
                    ),
                };
                frame.label = event_name;
                next_id += 1;
                let _ = send_event_tx.send(SendEvent::Frame {
                    request_id: request_id.clone(),
                    frame,
                });
                if event_count > 0 && event_count.is_multiple_of(10) {
                    let _ = send_event_tx.send(SendEvent::Progress(format!(
                        "Streaming... {event_count} events"
                    )));
                }
            }
        });

        let result = executor
            .send_streaming(resolved, Some(progress_tx), sse_tx, cancel_rx)
            .await;
        // sse_tx is dropped inside send_streaming; drain the pump so frame
        // events are ordered before the terminal event.
        let _ = frame_pump.await;
        result
    }

    /// Fetches the GraphQL schema via an introspection query sent to the
    /// request's endpoint (same URL, headers and auth; POST JSON body).
    /// Runs on the tokio runtime; the result arrives on a oneshot channel.
    pub fn fetch_graphql_schema(
        &self,
        mut req_item: RequestItem,
        env: Option<Arc<Environment>>,
    ) -> oneshot::Receiver<Result<GraphQLSchemaSummary, String>> {
        let (tx, rx) = oneshot::channel();
        let executor = self.executor.clone();
        self.tokio_rt.spawn(async move {
            let pre_output = run_pre_request_stage(&mut req_item, env.as_deref(), None);
            let effective_env = merge_env_mutations(&env, &pre_output.env_mutations);
            req_item.method = HttpMethod::Post;
            req_item.body =
                RequestBody::json(serde_json::json!({ "query": INTROSPECTION_QUERY }).to_string());
            req_item.update_content_type_header(Some("application/json"));
            let resolved = match ResolvedRequest::from_item(&req_item, effective_env.as_deref()) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
            };
            match executor.send(resolved, None).await {
                Ok(record) => {
                    let _ = tx.send(parse_introspection(&record.body));
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zest_core::models::{Environment, HttpMethod};

    #[test]
    fn test_http_engine_invalid_url_error() {
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
        let executor = Arc::new(ReqwestExecutor::new());
        let engine = HttpEngine::new(executor, rt.clone());

        let req = RequestItem::new("test-1", "Test", HttpMethod::Get, "not a valid url");
        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let mut rx = engine.execute(req, None, cancel_rx);

        let got_error = rt.block_on(async {
            while let Some(event) = rx.recv().await {
                if let SendEvent::Failed { error, .. } = event {
                    assert!(!error.is_empty());
                    return true;
                }
            }
            false
        });
        assert!(got_error);
    }

    #[test]
    fn test_http_engine_cancel() {
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
        let executor = Arc::new(ReqwestExecutor::new());
        let engine = HttpEngine::new(executor, rt.clone());

        let req = RequestItem::new(
            "test-cancel",
            "Cancel Test",
            HttpMethod::Get,
            "http://10.255.255.1",
        );
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let mut rx = engine.execute(req, None, cancel_rx);

        let _ = cancel_tx.send(());

        let got_cancelled = rt.block_on(async {
            while let Some(event) = rx.recv().await {
                if let SendEvent::Cancelled = event {
                    return true;
                }
            }
            false
        });
        assert!(got_cancelled);
    }

    #[test]
    fn test_http_engine_started_carries_env_mutations() {
        // Regression guard for the unified pipeline: the pre-script env
        // mutations must arrive on Started (like WS/gRPC), not on Success.
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
        let executor = Arc::new(ReqwestExecutor::new());
        let engine = HttpEngine::new(executor, rt.clone());

        let mut req = RequestItem::new(
            "test-script",
            "Script Test",
            HttpMethod::Get,
            "http://127.0.0.1:1/unreachable",
        );
        req.script = Some(zest_core::models::ScriptConfig {
            pre_request: Some("z.environment.set('chained', 'yes')".to_string()),
            post_response: None,
        });
        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let mut rx = engine.execute(
            req,
            Some(Arc::new(Environment::new("dev", "Dev"))),
            cancel_rx,
        );

        let got_started_with_mutation = rt.block_on(async {
            while let Some(event) = rx.recv().await {
                if let SendEvent::Started {
                    env_mutations,
                    pre_script_error,
                    ..
                } = event
                {
                    assert!(
                        pre_script_error.is_none(),
                        "script error: {pre_script_error:?}"
                    );
                    assert_eq!(
                        env_mutations,
                        vec![("chained".to_string(), Some("yes".to_string()))]
                    );
                    return true;
                }
            }
            false
        });
        assert!(got_started_with_mutation);
    }
}
