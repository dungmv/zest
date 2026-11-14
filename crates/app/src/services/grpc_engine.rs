use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use zest_core::grpc::{
    GrpcCallSummary, GrpcClient, GrpcMethodInfo, GrpcStreamEvent, GrpcUnaryResult,
};
use zest_core::models::{ConsoleLogEntry, Environment, EnvironmentVariable, RequestItem};
use zest_core::scripting::{
    resolve_fake_vars_in_request, PreRequestContext, PreRequestOutput, ScriptSandbox,
};
use zest_core::ws::{WsDirection, WsMessage, WsMessageKind};

use super::global_tokio_runtime;

/// UI-delivered events for one gRPC invocation, mirroring
/// [`super::http_engine::ExecEvent`]. Consumed on the GPUI thread by the
/// Runner pump.
pub enum GrpcUiEvent {
    /// Pre-request script finished; call about to start.
    Started {
        request_id: String,
        initial_logs: Vec<ConsoleLogEntry>,
        env_mutations: Vec<(String, Option<String>)>,
        pre_script_error: Option<String>,
    },
    /// The authored request message, echoed into the frame log as outbound.
    RequestMessage {
        request_id: String,
        message: WsMessage,
    },
    /// One inbound response message (single for unary, streamed otherwise).
    ResponseMessage {
        request_id: String,
        message: WsMessage,
    },
    /// Unary call finished (either OK or a gRPC error status).
    CallCompleted {
        request_id: String,
        summary: GrpcCallSummary,
    },
    /// Server-stream finished with a non-OK status mid-stream.
    CallFailed {
        request_id: String,
        summary: GrpcCallSummary,
    },
    /// Setup/schema/transport failure before any call completed.
    Failed { request_id: String, error: String },
}

/// Schema summary delivered to the UI picker after a successful fetch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrpcSchemaSummary {
    /// (fully-qualified service name, methods)
    pub services: Vec<(String, Vec<GrpcMethodInfo>)>,
}

/// Dynamic gRPC invocation engine. Schema pools are cached per
/// `(endpoint, proto_path)` so reflection only happens once per target.
pub struct GrpcEngine {
    tokio_rt: Arc<tokio::runtime::Runtime>,
    schema_cache: Arc<Mutex<HashMap<String, zest_core::grpc::DescriptorPool>>>,
}

impl Default for GrpcEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GrpcEngine {
    pub fn new() -> Self {
        Self {
            tokio_rt: global_tokio_runtime(),
            schema_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn lock_cache(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<String, zest_core::grpc::DescriptorPool>> {
        self.schema_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Starts the invoke pipeline on the tokio runtime; the Runner must pump
    /// the returned events. Cancel via the usual oneshot channel.
    pub fn invoke(
        &self,
        mut req_item: RequestItem,
        env: Option<Arc<Environment>>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> UnboundedReceiver<GrpcUiEvent> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let schema_cache = self.schema_cache.clone();

        self.tokio_rt.spawn(async move {
            // 1. Fake vars + pre-request script (same preamble as HTTP/WS)
            resolve_fake_vars_in_request(&mut req_item);
            let pre_script_code = req_item
                .script
                .as_ref()
                .and_then(|s| s.pre_request.clone());
            let pre_output = match pre_script_code {
                Some(code) => {
                    let ctx = PreRequestContext::from_request_and_env(&req_item, env.as_deref());
                    ScriptSandbox::run_pre_request(&code, ctx)
                }
                None => PreRequestOutput::default(),
            };
            let request_id = req_item.id.clone();

            let _ = event_tx.send(GrpcUiEvent::Started {
                request_id: request_id.clone(),
                initial_logs: pre_output.console_entries.clone(),
                env_mutations: pre_output.env_mutations.clone(),
                pre_script_error: pre_output.error.clone(),
            });

            // 2. Merge pre-script env mutations
            let effective_env = merge_env_mutations(env, &pre_output.env_mutations);

            // 3. Resolve headers/auth; the URL keeps its grpc:// scheme
            let resolved = match zest_core::http::ResolvedRequest::from_item(
                &req_item,
                effective_env.as_ref(),
            ) {
                Ok(r) => r,
                Err(e) => {
                    let _ = event_tx.send(GrpcUiEvent::Failed {
                        request_id: request_id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            };

            let Some(payload) = req_item.grpc.clone() else {
                let _ = event_tx.send(GrpcUiEvent::Failed {
                    request_id: request_id.clone(),
                    error: "This request has no gRPC payload".to_string(),
                });
                return;
            };
            if payload.service.is_empty() || payload.method.is_empty() {
                let _ = event_tx.send(GrpcUiEvent::Failed {
                    request_id: request_id.clone(),
                    error: "Pick a service and method in the Proto tab first".to_string(),
                });
                return;
            }

            // 4. Client + schema (cached per endpoint+proto path)
            let client = match GrpcClient::connect(&resolved.url, resolved.settings.timeout_ms) {
                Ok(c) => c,
                Err(e) => {
                    let _ = event_tx.send(GrpcUiEvent::Failed {
                        request_id: request_id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            };

            let cache_key = format!(
                "{}|{}",
                resolved.url,
                payload.proto_path.clone().unwrap_or_default()
            );
            // Never hold the cache lock across the schema fetch await.
            let cached = {
                let cache = schema_cache
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                cache.get(&cache_key).cloned()
            };
            let pool = match cached {
                Some(pool) => pool,
                None => {
                    let proto_path = if payload.use_reflection {
                        None
                    } else {
                        payload.proto_path.as_deref().filter(|p| !p.trim().is_empty())
                    };
                    match zest_core::grpc::resolve_schema(proto_path, Some(client.channel())).await
                    {
                        Ok(pool) => {
                            schema_cache
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .insert(cache_key, pool.clone());
                            pool
                        }
                        Err(e) => {
                            let _ = event_tx.send(GrpcUiEvent::Failed {
                                request_id: request_id.clone(),
                                error: e.to_string(),
                            });
                            return;
                        }
                    }
                }
            };

            // 5. Locate the method descriptors
            let Some(service_desc) = pool.get_service_by_name(&payload.service) else {
                let _ = event_tx.send(GrpcUiEvent::Failed {
                    request_id: request_id.clone(),
                    error: format!("Service '{}' not found in schema", payload.service),
                });
                return;
            };
            let Some(method_desc) = service_desc
                .methods()
                .find(|m| m.name() == payload.method)
            else {
                let _ = event_tx.send(GrpcUiEvent::Failed {
                    request_id: request_id.clone(),
                    error: format!(
                        "Method '{}' not found on service '{}'",
                        payload.method, payload.service
                    ),
                });
                return;
            };
            let input_desc = method_desc.input();
            let output_desc = method_desc.output();

            // 6. Echo the authored request into the frame log
            let _ = event_tx.send(GrpcUiEvent::RequestMessage {
                request_id: request_id.clone(),
                message: WsMessage {
                    id: 0,
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                    direction: WsDirection::Outbound,
                    kind: WsMessageKind::Text,
                    payload: payload.message_json.clone().into_bytes(),
                },
            });

            // 7. Invoke, racing a re-usable cancellation watch so streaming
            // calls stay cancellable after the headers arrive.
            let (cancel_flag_tx, cancel_flag_rx) = tokio::sync::watch::channel(false);
            tokio::spawn(async move {
                let _ = cancel_rx.await;
                let _ = cancel_flag_tx.send(true);
            });

            let is_server_streaming = method_desc.is_server_streaming();
            tokio::select! {
                _ = cancelled(cancel_flag_rx.clone()) => {}
                res = async {
                    if is_server_streaming {
                        let mut stream_rx = client
                            .invoke_server_streaming(
                                input_desc,
                                output_desc,
                                &payload.service,
                                &payload.method,
                                &payload.message_json,
                                &resolved.headers,
                            )
                            .await
                            .map_err(|e| e.to_string())?;
                        // Pump inline so cancellation interrupts the stream.
                        loop {
                            tokio::select! {
                                _ = cancelled(cancel_flag_rx.clone()) => break,
                                ev = stream_rx.recv() => match ev {
                                    Some(GrpcStreamEvent::Message { json, .. }) => {
                                        let _ = event_tx.send(GrpcUiEvent::ResponseMessage {
                                            request_id: request_id.clone(),
                                            message: log_message(&json),
                                        });
                                    }
                                    Some(GrpcStreamEvent::Completed { duration_ms }) => {
                                        let _ = event_tx.send(GrpcUiEvent::CallCompleted {
                                            request_id: request_id.clone(),
                                            summary: GrpcCallSummary {
                                                status_code: 0,
                                                status_message: None,
                                                headers: Vec::new(),
                                                duration_ms,
                                            },
                                        });
                                        break;
                                    }
                                    Some(GrpcStreamEvent::Failed { code, message, duration_ms }) => {
                                        let _ = event_tx.send(GrpcUiEvent::CallFailed {
                                            request_id: request_id.clone(),
                                            summary: GrpcCallSummary {
                                                status_code: code,
                                                status_message: Some(message),
                                                headers: Vec::new(),
                                                duration_ms,
                                            },
                                        });
                                        break;
                                    }
                                    Some(GrpcStreamEvent::TransportError(err)) => {
                                        let _ = event_tx.send(GrpcUiEvent::Failed {
                                            request_id: request_id.clone(),
                                            error: err,
                                        });
                                        break;
                                    }
                                    None => break,
                                }
                            }
                        }
                        Ok(())
                    } else {
                        client
                            .invoke_unary(
                                input_desc,
                                output_desc,
                                &payload.service,
                                &payload.method,
                                &payload.message_json,
                                &resolved.headers,
                            )
                            .await
                            .map(|result| {
                                finish_unary(&request_id, result, event_tx.clone())
                            })
                            .map_err(|e: zest_core::grpc::GrpcError| e.to_string())
                    }
                } => {
                    if let Err(e) = res {
                        let _ = event_tx.send(GrpcUiEvent::Failed {
                            request_id: request_id.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        });

        event_rx
    }

    /// Resolves the schema for the UI picker (reflection or proto path) and
    /// reports through the returned oneshot.
    pub fn fetch_schema(
        &self,
        endpoint_url: String,
        proto_path: Option<String>,
        use_reflection: bool,
    ) -> oneshot::Receiver<Result<GrpcSchemaSummary, String>> {
        let (tx, rx) = oneshot::channel();
        let cache_key = format!(
            "{}|{}",
            endpoint_url,
            proto_path.clone().unwrap_or_default()
        );
        let cached = self.lock_cache().get(&cache_key).map(summarize_pool);

        if let Some(summary) = cached {
            let _ = tx.send(Ok(summary));
            return rx;
        }

        self.tokio_rt.spawn(async move {
            let result = async {
                let client =
                    GrpcClient::connect(&endpoint_url, 30_000).map_err(|e| e.to_string())?;
                let proto = if use_reflection {
                    None
                } else {
                    proto_path.as_deref().filter(|p| !p.trim().is_empty())
                };
                let pool = zest_core::grpc::resolve_schema(proto, Some(client.channel()))
                    .await
                    .map_err(|e| e.to_string())?;
                let summary = summarize_pool(&pool);
                Ok::<GrpcSchemaSummary, String>(summary)
            }
            .await;
            let _ = tx.send(result);
        });
        rx
    }
}

fn summarize_pool(pool: &zest_core::grpc::DescriptorPool) -> GrpcSchemaSummary {
    GrpcSchemaSummary {
        services: pool
            .services()
            .map(|svc| {
                (
                    svc.full_name().to_string(),
                    svc.methods()
                        .map(|m| GrpcMethodInfo {
                            name: m.name().to_string(),
                            is_client_streaming: m.is_client_streaming(),
                            is_server_streaming: m.is_server_streaming(),
                        })
                        .collect(),
                )
            })
            .filter(|(name, _)| !name.starts_with("grpc."))
            .collect(),
    }
}

fn merge_env_mutations(
    env: Option<Arc<Environment>>,
    mutations: &[(String, Option<String>)],
) -> Option<Environment> {
    if mutations.is_empty() {
        return env.as_deref().cloned();
    }
    let mut temp = env
        .as_ref()
        .map(|e| (**e).clone())
        .unwrap_or_else(|| Environment::new("temp", "Temp"));
    for (k, v_opt) in mutations {
        if let Some(val) = v_opt {
            if let Some(v_item) = temp.variables.iter_mut().find(|v| v.key == *k) {
                v_item.current_value = val.clone();
            } else {
                temp.variables.push(EnvironmentVariable {
                    key: k.clone(),
                    initial_value: String::new(),
                    current_value: val.clone(),
                    secret: false,
                });
            }
        } else {
            temp.variables.retain(|v| v.key != *k);
        }
    }
    Some(temp)
}

fn log_message(json: &str) -> WsMessage {
    WsMessage {
        id: 0,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        direction: WsDirection::Inbound,
        kind: WsMessageKind::Text,
        payload: json.as_bytes().to_vec(),
    }
}

fn finish_unary(request_id: &str, result: GrpcUnaryResult, event_tx: UnboundedSender<GrpcUiEvent>) {
    let _ = event_tx.send(GrpcUiEvent::ResponseMessage {
        request_id: request_id.to_string(),
        message: log_message(&result.response_json),
    });
    let _ = event_tx.send(GrpcUiEvent::CallCompleted {
        request_id: request_id.to_string(),
        summary: GrpcCallSummary {
            status_code: 0,
            status_message: None,
            headers: result.headers,
            duration_ms: result.duration_ms,
        },
    });
}

async fn cancelled(mut rx: tokio::sync::watch::Receiver<bool>) {
    let _ = rx.changed().await;
}
