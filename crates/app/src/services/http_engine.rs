use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use zest_core::http::{HttpExecutor, HttpProgress, ReqwestExecutor};
use zest_core::models::{Environment, RequestItem};
use zest_core::scripting::{PostResponseContext, PostResponseOutput, ScriptSandbox};
use zest_core::transport::{RequestError, ResolvedRequest};

use super::pipeline::{merge_env_mutations, run_pre_request_stage};
use super::send_event::SendEvent;

pub struct HttpEngine {
    executor: Arc<ReqwestExecutor>,
    tokio_rt: Arc<tokio::runtime::Runtime>,
}

impl HttpEngine {
    pub fn new(executor: Arc<ReqwestExecutor>, tokio_rt: Arc<tokio::runtime::Runtime>) -> Self {
        Self { executor, tokio_rt }
    }

    /// Starts the HTTP pipeline on the tokio runtime and returns the event
    /// receiver the Runner must pump. Cancel via the usual oneshot channel.
    pub fn execute(
        &self,
        mut req_item: RequestItem,
        env: Option<Arc<Environment>>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> mpsc::UnboundedReceiver<SendEvent> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let executor = self.executor.clone();
        let send_event_tx = event_tx.clone();

        self.tokio_rt.spawn(async move {
            // 1. Resolve dynamic fake variables + run the pre-request script
            let pre_output = run_pre_request_stage(&mut req_item, env.as_deref());
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

            tokio::select! {
                _ = cancel_rx => {
                    let _ = send_event_tx.send(SendEvent::Cancelled);
                }
                res = executor.send(resolved, Some(progress_tx)) => {
                    match res {
                        Ok(mut record) => {
                            // 4. Post-response / Test Script
                            let post_output = if let Some(code) = post_script_code {
                                let post_ctx = PostResponseContext::from_response_and_env(&record, eff_env_for_post.as_deref());
                                ScriptSandbox::run_post_response(&code, post_ctx)
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
                                request_id: req_item.id.clone(),
                                error: e.to_string(),
                            });
                        }
                    }
                }
            }
        });

        event_rx
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
