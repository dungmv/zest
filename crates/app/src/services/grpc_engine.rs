use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use zest_core::grpc::{
    GrpcCallSummary, GrpcClient, GrpcMethodInfo, GrpcStreamEvent, GrpcUnaryResult,
};
use zest_core::models::{Environment, RequestItem};
use zest_core::transport::{FrameDirection, FrameKind, ResolvedRequest, TransportFrame};

use super::global_tokio_runtime;
use super::pipeline::{merge_env_mutations, run_pre_request_stage};
use super::send_event::SendEvent;

/// Maximum frames retained per gRPC invocation log (oldest dropped).
const MAX_GRPC_LOG_FRAMES: usize = 1000;

/// Bounded per-request frame log + last call summary so a gRPC invocation
/// history survives request switches, mirroring `WsService`'s session
/// records.
struct GrpcLog {
    frames: Vec<TransportFrame>,
    next_frame_id: u64,
    last_call: Option<GrpcCallSummary>,
}

/// Schema summary delivered to the UI after a successful fetch. Carries the
/// full descriptor pool so the editor can autocomplete and validate the JSON
/// message against the actual schema.
#[derive(Debug, Clone, Default)]
pub struct GrpcSchemaSummary {
    /// (fully-qualified service name, methods)
    pub services: Vec<(String, Vec<GrpcMethodInfo>)>,
    pub pool: zest_core::grpc::DescriptorPool,
}

/// Dynamic gRPC invocation engine. Schema pools are cached per
/// `(endpoint, proto_path)` so reflection only happens once per target.
pub struct GrpcEngine {
    tokio_rt: Arc<tokio::runtime::Runtime>,
    schema_cache: Arc<Mutex<HashMap<String, zest_core::grpc::DescriptorPool>>>,
    logs: Mutex<HashMap<String, GrpcLog>>,
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
            logs: Mutex::new(HashMap::new()),
        }
    }

    fn lock_cache(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<String, zest_core::grpc::DescriptorPool>> {
        self.schema_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_logs(&self) -> std::sync::MutexGuard<'_, HashMap<String, GrpcLog>> {
        self.logs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Appends a frame to the bounded per-request log (Runner pump).
    pub fn register_frame(&self, request_id: &str, mut frame: TransportFrame) {
        let mut logs = self.lock_logs();
        let log = logs
            .entry(request_id.to_string())
            .or_insert_with(|| GrpcLog {
                frames: Vec::new(),
                next_frame_id: 0,
                last_call: None,
            });
        frame.id = log.next_frame_id;
        log.next_frame_id = log.next_frame_id.wrapping_add(1);
        log.frames.push(frame);
        if log.frames.len() > MAX_GRPC_LOG_FRAMES {
            let overflow = log.frames.len() - MAX_GRPC_LOG_FRAMES;
            log.frames.drain(..overflow);
        }
    }

    /// Records the summary of a finished call (Runner pump).
    pub fn register_last_call(&self, request_id: &str, summary: GrpcCallSummary) {
        let mut logs = self.lock_logs();
        let log = logs
            .entry(request_id.to_string())
            .or_insert_with(|| GrpcLog {
                frames: Vec::new(),
                next_frame_id: 0,
                last_call: None,
            });
        log.last_call = Some(summary);
    }

    /// Copy of the per-request frame log for view restoration on switch.
    pub fn snapshot_frames(&self, request_id: &str) -> Option<Vec<TransportFrame>> {
        self.lock_logs().get(request_id).map(|l| l.frames.clone())
    }

    /// Copy of the last finished call summary for view restoration on switch.
    pub fn snapshot_last_call(&self, request_id: &str) -> Option<GrpcCallSummary> {
        self.lock_logs()
            .get(request_id)
            .and_then(|l| l.last_call.clone())
    }

    /// Starts the invoke pipeline on the tokio runtime; the Runner must pump
    /// the returned events. Cancel via the usual oneshot channel.
    pub fn invoke(
        &self,
        mut req_item: RequestItem,
        env: Option<Arc<Environment>>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> UnboundedReceiver<SendEvent> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let schema_cache = self.schema_cache.clone();

        self.tokio_rt.spawn(async move {
            // 1. Fake vars + pre-request script (shared first stage)
            let pre_output = run_pre_request_stage(&mut req_item, env.as_deref(), None);
            let request_id = req_item.id.clone();

            let _ = event_tx.send(SendEvent::Started {
                request_id: request_id.clone(),
                initial_logs: pre_output.console_entries.clone(),
                env_mutations: pre_output.env_mutations.clone(),
                pre_script_error: pre_output.error.clone(),
            });

            // 2. Merge pre-script env mutations
            let effective_env = merge_env_mutations(&env, &pre_output.env_mutations);

            // 2b. Interpolate {{var}} in the authored gRPC message, mirroring
            // HTTP body treatment (fake vars were resolved by stage 1).
            if let Some(payload) = req_item.grpc.as_mut() {
                let interpolated = zest_core::env::interpolate_cow(
                    &payload.message_json,
                    effective_env.as_deref(),
                )
                .into_owned();
                payload.message_json = interpolated;
            }

            // 3. Resolve headers/auth; the URL keeps its grpc:// scheme
            let resolved = match ResolvedRequest::from_item(&req_item, effective_env.as_deref()) {
                Ok(r) => r,
                Err(e) => {
                    let _ = event_tx.send(SendEvent::Failed {
                        request_id: request_id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            };

            // A brand-new gRPC request has no payload yet; reflection/`proto`
            // defaults kick in so a bare URL + Send just works.
            let mut payload = req_item.grpc.clone().unwrap_or_default();

            // 4. Client + schema (cached per endpoint+proto path)
            let client = match GrpcClient::connect(&resolved.url, resolved.settings.timeout_ms) {
                Ok(c) => c,
                Err(e) => {
                    let _ = event_tx.send(SendEvent::Failed {
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
                            let _ = event_tx.send(SendEvent::Failed {
                                request_id: request_id.clone(),
                                error: format!(
                                    "{e} — tried {} (use grpcs:// if the server requires TLS)",
                                    resolved.url
                                ),
                            });
                            return;
                        }
                    }
                }
            };

            // 5. Auto-resolve a missing service/method selection: publish the
            // schema to the picker and, when the server exposes exactly one
            // service with one method, proceed with it automatically.
            if payload.service.is_empty() || payload.method.is_empty() {
                let summary = summarize_pool(&pool);
                let selection = auto_selection(&summary);
                let _ = event_tx.send(SendEvent::SchemaReady {
                    request_id: request_id.clone(),
                    summary: summary.clone(),
                    auto_service: selection.as_ref().map(|(svc, _)| svc.clone()),
                    auto_method: selection.as_ref().map(|(_, mth)| mth.clone()),
                });
                match selection {
                    Some((svc, mth)) => {
                        payload.service = svc;
                        payload.method = mth;
                    }
                    None => {
                        let _ = event_tx.send(SendEvent::Failed {
                            request_id: request_id.clone(),
                            error: format!(
                                "Schema loaded ({} services) — pick a service and method in the Proto tab, then send again",
                                summary.services.len()
                            ),
                        });
                        return;
                    }
                }
            }

            // 6. Locate the method descriptors
            let Some(service_desc) = pool.get_service_by_name(&payload.service) else {
                let _ = event_tx.send(SendEvent::Failed {
                    request_id: request_id.clone(),
                    error: format!("Service '{}' not found in schema", payload.service),
                });
                return;
            };
            let Some(method_desc) = service_desc
                .methods()
                .find(|m| m.name() == payload.method)
            else {
                let _ = event_tx.send(SendEvent::Failed {
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
            let _ = event_tx.send(SendEvent::Frame {
                request_id: request_id.clone(),
                frame: TransportFrame {
                    id: 0,
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                    direction: FrameDirection::Outbound,
                    kind: FrameKind::Text,
                    payload: payload.message_json.clone().into_bytes(),
                    label: None,
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
                                        let _ = event_tx.send(SendEvent::Frame {
                                            request_id: request_id.clone(),
                                            frame: log_message(&json),
                                        });
                                    }
                                    Some(GrpcStreamEvent::Completed { duration_ms }) => {
                                        let _ = event_tx.send(SendEvent::CallCompleted {
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
                                        let _ = event_tx.send(SendEvent::CallFailed {
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
                                        let _ = event_tx.send(SendEvent::Failed {
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
                        let _ = event_tx.send(SendEvent::Failed {
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
        pool: pool.clone(),
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
            // Hide only the reflection services themselves; standard
            // grpc.* services like grpc.health.v1.Health stay callable.
            .filter(|(name, _)| !name.starts_with("grpc.reflection."))
            .collect(),
    }
}

fn log_message(json: &str) -> TransportFrame {
    TransportFrame {
        id: 0,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        direction: FrameDirection::Inbound,
        kind: FrameKind::Text,
        payload: json.as_bytes().to_vec(),
        label: None,
    }
}

fn finish_unary(request_id: &str, result: GrpcUnaryResult, event_tx: UnboundedSender<SendEvent>) {
    let _ = event_tx.send(SendEvent::Frame {
        request_id: request_id.to_string(),
        frame: log_message(&result.response_json),
    });
    let _ = event_tx.send(SendEvent::CallCompleted {
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

/// Picks the service/method automatically when the schema is unambiguous
/// (exactly one non-reflection service exposing exactly one method).
fn auto_selection(summary: &GrpcSchemaSummary) -> Option<(String, String)> {
    if summary.services.len() != 1 {
        return None;
    }
    let (service, methods) = &summary.services[0];
    if methods.len() != 1 {
        return None;
    }
    Some((service.clone(), methods[0].name.clone()))
}

#[cfg(test)]
#[path = "grpc_engine_tests.rs"]
mod tests;
