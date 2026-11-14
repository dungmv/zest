use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use zest_core::http::{HttpExecutor, HttpProgress, RequestError, ReqwestExecutor, ResolvedRequest};
use zest_core::models::{Environment, EnvironmentVariable, HttpResponseRecord, RequestItem};
use zest_core::scripting::{
    resolve_fake_vars_in_request, PostResponseContext, PostResponseOutput, PreRequestContext,
    PreRequestOutput, ScriptSandbox,
};

#[derive(Debug)]
pub enum ExecEvent {
    Started {
        request_id: String,
        initial_logs: Vec<zest_core::models::ConsoleLogEntry>,
        pre_script_error: Option<String>,
    },
    Progress(String),
    Success {
        record: Arc<HttpResponseRecord>,
        pre_output: Box<PreRequestOutput>,
        post_output: Box<PostResponseOutput>,
    },
    Cancelled,
    Error(String),
}

pub struct HttpEngine {
    executor: Arc<ReqwestExecutor>,
    tokio_rt: Arc<tokio::runtime::Runtime>,
}

impl HttpEngine {
    pub fn new(executor: Arc<ReqwestExecutor>, tokio_rt: Arc<tokio::runtime::Runtime>) -> Self {
        Self { executor, tokio_rt }
    }

    pub fn execute(
        &self,
        mut req_item: RequestItem,
        env: Option<Arc<Environment>>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> mpsc::UnboundedReceiver<ExecEvent> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let executor = self.executor.clone();
        let send_event_tx = event_tx.clone();

        self.tokio_rt.spawn(async move {
            // 1. Resolve Dynamic Fake Variables
            resolve_fake_vars_in_request(&mut req_item);

            // 2. Pre-request Script execution
            let pre_script_code = req_item.script.as_ref().and_then(|s| s.pre_request.clone());
            let post_script_code = req_item.script.as_ref().and_then(|s| s.post_response.clone());

            let pre_output = if let Some(code) = pre_script_code {
                let pre_ctx = PreRequestContext::from_request_and_env(&req_item, env.as_deref());
                ScriptSandbox::run_pre_request(&code, pre_ctx)
            } else {
                PreRequestOutput::default()
            };

            let initial_logs = pre_output.console_entries.clone();
            let pre_script_error = pre_output.error.clone();
            let _ = send_event_tx.send(ExecEvent::Started {
                request_id: req_item.id.clone(),
                initial_logs,
                pre_script_error,
            });

            // 3. Merge pre-script env mutations into effective environment
            let mut mutated_env = None;
            let effective_env_ref = if !pre_output.env_mutations.is_empty() {
                let mut temp_env = env
                    .as_ref()
                    .map(|e| (**e).clone())
                    .unwrap_or_else(|| Environment::new("temp", "Temp"));
                for (k, v_opt) in &pre_output.env_mutations {
                    if let Some(val) = v_opt {
                        if let Some(v_item) = temp_env.variables.iter_mut().find(|v| v.key == *k) {
                            v_item.current_value = val.clone();
                        } else {
                            temp_env.variables.push(EnvironmentVariable {
                                key: k.clone(),
                                initial_value: "".to_string(),
                                current_value: val.clone(),
                                secret: false,
                            });
                        }
                    } else {
                        temp_env.variables.retain(|v| v.key != *k);
                    }
                }
                mutated_env = Some(temp_env);
                mutated_env.as_ref()
            } else {
                env.as_deref()
            };

            // 4. Resolve Request with effective environment & apply pre-script mutations
            let mut resolved = match ResolvedRequest::from_item(&req_item, effective_env_ref) {
                Ok(r) => r,
                Err(e) => {
                    let _ = send_event_tx.send(ExecEvent::Error(e.to_string()));
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
                    let _ = progress_event_tx.send(ExecEvent::Progress(text));
                }
            });

            let eff_env_for_post = mutated_env.map(Arc::new).or(env);
            let pre_console_entries = pre_output.console_entries.clone();

            tokio::select! {
                _ = cancel_rx => {
                    let _ = send_event_tx.send(ExecEvent::Cancelled);
                }
                res = executor.send(resolved, Some(progress_tx)) => {
                    match res {
                        Ok(mut record) => {
                            // 5. Post-response / Test Script
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

                            let _ = send_event_tx.send(ExecEvent::Success {
                                record: Arc::new(record),
                                pre_output: Box::new(pre_output),
                                post_output: Box::new(post_output),
                            });
                        }
                        Err(RequestError::Cancelled) => {
                            let _ = send_event_tx.send(ExecEvent::Cancelled);
                        }
                        Err(e) => {
                            let _ = send_event_tx.send(ExecEvent::Error(e.to_string()));
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
    use zest_core::models::HttpMethod;

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
                if let ExecEvent::Error(err) = event {
                    assert!(!err.is_empty());
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
                if let ExecEvent::Cancelled = event {
                    return true;
                }
            }
            false
        });
        assert!(got_cancelled);
    }
}
