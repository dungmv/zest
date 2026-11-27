use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rquickjs::function::Opt;
use rquickjs::{Context, Function, Object, Runtime, Value};

use crate::models::{ConsoleLogEntry, ConsoleLogLevel, ConsoleLogSource, TestResult};
use crate::scripting::api::{
    PostResponseContext, PostResponseOutput, PreRequestContext, PreRequestOutput,
};

#[path = "sandbox_bindings.rs"]
mod bindings;
use bindings::{
    build_crypto_object, build_environment_object, build_request_object, build_response_object,
    build_variables_object, format_js_error, inject_console,
};

type EnvMutations = Rc<RefCell<Vec<(String, Option<String>)>>>;

/// Wall-clock budget for a single script evaluation. User scripts run as
/// sandboxed CPU work on the tokio runtime; an unbounded loop must never
/// occupy a worker thread longer than this.
pub const SCRIPT_EVAL_TIMEOUT: Duration = Duration::from_secs(30);
/// Hard heap cap for the QuickJS runtime (guards allocation-bomb scripts).
pub const SCRIPT_MEMORY_LIMIT: usize = 256 * 1024 * 1024;
/// JS stack cap (guards runaway recursion).
pub const SCRIPT_STACK_LIMIT: usize = 1024 * 1024;

/// Shared, thread-safe cancellation flag for user scripts. Engines set it
/// when the user cancels an in-flight request; the QuickJS interrupt
/// handler stops script execution as soon as it is observed.
#[derive(Clone, Default)]
pub struct ScriptCancel(Arc<AtomicBool>);

impl ScriptCancel {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Marks the script as cancelled (safe to call from any thread).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Applies the sandbox resource limits (memory, stack, wall-clock deadline,
/// optional cancellation) to a freshly created runtime. Returns the eval
/// deadline so callers can attribute eval errors to a timeout.
fn configure_runtime(rt: &Runtime, timeout: Duration, cancel: Option<&ScriptCancel>) -> Instant {
    rt.set_memory_limit(SCRIPT_MEMORY_LIMIT);
    rt.set_max_stack_size(SCRIPT_STACK_LIMIT);
    let deadline = Instant::now() + timeout;
    let flag = cancel.cloned();
    // The handler runs on the JS thread on every loop back-edge; rquickjs
    // contract: returning `true` interrupts the interpreter with an
    // un-catchable "interrupted" error.
    rt.set_interrupt_handler(Some(Box::new(move || {
        if Instant::now() >= deadline {
            return true;
        }
        match &flag {
            Some(f) => f.is_cancelled(),
            None => false,
        }
    })));
    deadline
}

/// Maps an eval failure to a user-facing message, overriding the generic
/// interrupt error with an explicit timeout/cancellation reason.
fn eval_error_message(
    ctx: &rquickjs::Ctx<'_>,
    e: rquickjs::Error,
    deadline: Instant,
    cancel: Option<&ScriptCancel>,
) -> String {
    if let Some(c) = cancel {
        if c.is_cancelled() {
            return "Script cancelled by user".to_string();
        }
    }
    if Instant::now() >= deadline {
        return format!("Script timed out after {}s", SCRIPT_EVAL_TIMEOUT.as_secs());
    }
    format_js_error(ctx, e)
}

pub struct ScriptSandbox;

impl ScriptSandbox {
    /// Execute Pre-request Script in an isolated QuickJS runtime
    pub fn run_pre_request(
        code: &str,
        ctx_data: PreRequestContext,
        cancel: Option<&ScriptCancel>,
    ) -> PreRequestOutput {
        Self::run_pre_request_inner(code, ctx_data, cancel, SCRIPT_EVAL_TIMEOUT)
    }

    /// Test-only variant with an injectable deadline so the interrupt path
    /// can be exercised without waiting for the full [`SCRIPT_EVAL_TIMEOUT`].
    #[cfg(test)]
    pub(crate) fn run_pre_request_with_timeout(
        code: &str,
        ctx_data: PreRequestContext,
        timeout: Duration,
    ) -> PreRequestOutput {
        Self::run_pre_request_inner(code, ctx_data, None, timeout)
    }

    fn run_pre_request_inner(
        code: &str,
        ctx_data: PreRequestContext,
        cancel: Option<&ScriptCancel>,
        timeout: Duration,
    ) -> PreRequestOutput {
        let rt = match Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                return PreRequestOutput {
                    error: Some(format!("Failed to initialize QuickJS runtime: {}", e)),
                    ..Default::default()
                }
            }
        };
        let deadline = configure_runtime(&rt, timeout, cancel);

        let ctx = match Context::full(&rt) {
            Ok(c) => c,
            Err(e) => {
                return PreRequestOutput {
                    error: Some(format!("Failed to initialize QuickJS context: {}", e)),
                    ..Default::default()
                }
            }
        };

        let env_mutations: EnvMutations = Default::default();
        let header_mutations: Rc<RefCell<Vec<(String, String)>>> = Default::default();
        let body_override: Rc<RefCell<Option<String>>> = Default::default();
        let console_entries: Rc<RefCell<Vec<ConsoleLogEntry>>> = Default::default();

        let eval_res = ctx.with(|ctx| -> Result<(), String> {
            let env_obj = build_environment_object(&ctx, &ctx_data.env_vars, env_mutations.clone())
                .map_err(|e| format!("{:?}", e))?;
            let vars_obj =
                build_variables_object(&ctx, &ctx_data.env_vars).map_err(|e| format!("{:?}", e))?;
            let req_obj = build_request_object(
                &ctx,
                &ctx_data,
                header_mutations.clone(),
                body_override.clone(),
            )
            .map_err(|e| format!("{:?}", e))?;
            let crypto_obj = build_crypto_object(&ctx).map_err(|e| format!("{:?}", e))?;

            let client = Object::new(ctx.clone()).map_err(|e| format!("{:?}", e))?;
            client
                .set("environment", env_obj.clone())
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("global", env_obj.clone())
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("globals", env_obj.clone())
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("variables", vars_obj)
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("request", req_obj)
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("crypto", crypto_obj)
                .map_err(|e| format!("{:?}", e))?;

            // Dummy test/assert trong pre-request
            let test_fn = Function::new(ctx.clone(), |_name: String, _f: Value| {})
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("test", test_fn)
                .map_err(|e| format!("{:?}", e))?;

            let assert_fn = Function::new(
                ctx.clone(),
                |ctx: rquickjs::Ctx<'_>,
                 condition: bool,
                 msg: Opt<String>|
                 -> rquickjs::Result<()> {
                    if !condition {
                        let m = msg.0.unwrap_or_else(|| "Assertion failed".to_string());
                        let js_str = rquickjs::String::from_str(ctx.clone(), &m)?;
                        Err(ctx.throw(js_str.into()))
                    } else {
                        Ok(())
                    }
                },
            )
            .map_err(|e| format!("{:?}", e))?;
            client
                .set("assert", assert_fn)
                .map_err(|e| format!("{:?}", e))?;

            ctx.globals()
                .set("z", client)
                .map_err(|e| format!("{:?}", e))?;

            inject_console(&ctx, console_entries.clone(), ConsoleLogSource::PreRequest)
                .map_err(|e| format!("{:?}", e))?;

            let eval_result: Result<Value, _> = ctx.eval(code.as_bytes());
            if let Err(e) = eval_result {
                return Err(eval_error_message(&ctx, e, deadline, cancel));
            }

            Ok(())
        });

        let error = eval_res.err();
        let env_mutations = env_mutations.borrow().clone();
        let header_mutations = header_mutations.borrow().clone();
        let body_override = body_override.borrow().clone();
        let mut console_entries = console_entries.borrow().clone();
        if let Some(err) = &error {
            console_entries.push(ConsoleLogEntry {
                source: ConsoleLogSource::PreRequest,
                level: ConsoleLogLevel::Error,
                message: err.clone(),
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            });
        }
        let console_logs = console_entries.iter().map(|e| e.message.clone()).collect();

        PreRequestOutput {
            env_mutations,
            header_mutations,
            body_override,
            console_logs,
            console_entries,
            error,
        }
    }

    /// Execute Post-response / Test Script after server response is received
    pub fn run_post_response(
        code: &str,
        ctx_data: PostResponseContext,
        cancel: Option<&ScriptCancel>,
    ) -> PostResponseOutput {
        let rt = match Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                return PostResponseOutput {
                    error: Some(format!("Failed to initialize QuickJS runtime: {}", e)),
                    ..Default::default()
                }
            }
        };
        let deadline = configure_runtime(&rt, SCRIPT_EVAL_TIMEOUT, cancel);

        let ctx = match Context::full(&rt) {
            Ok(c) => c,
            Err(e) => {
                return PostResponseOutput {
                    error: Some(format!("Failed to initialize QuickJS context: {}", e)),
                    ..Default::default()
                }
            }
        };

        let env_mutations: EnvMutations = Default::default();
        let test_results: Rc<RefCell<Vec<TestResult>>> = Default::default();
        let console_entries: Rc<RefCell<Vec<ConsoleLogEntry>>> = Default::default();

        let eval_res = ctx.with(|ctx| -> Result<(), String> {
            let env_obj = build_environment_object(&ctx, &ctx_data.env_vars, env_mutations.clone())
                .map_err(|e| format!("{:?}", e))?;
            let vars_obj =
                build_variables_object(&ctx, &ctx_data.env_vars).map_err(|e| format!("{:?}", e))?;
            let resp_obj =
                build_response_object(&ctx, &ctx_data).map_err(|e| format!("{:?}", e))?;
            let crypto_obj = build_crypto_object(&ctx).map_err(|e| format!("{:?}", e))?;

            let client = Object::new(ctx.clone()).map_err(|e| format!("{:?}", e))?;
            client
                .set("environment", env_obj.clone())
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("global", env_obj.clone())
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("globals", env_obj.clone())
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("variables", vars_obj)
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("response", resp_obj)
                .map_err(|e| format!("{:?}", e))?;
            client
                .set("crypto", crypto_obj)
                .map_err(|e| format!("{:?}", e))?;

            // z.test(name, callback)
            let tr_store = test_results.clone();
            let test_fn = Function::new(
                ctx.clone(),
                move |ctx: rquickjs::Ctx<'_>, name: String, cb: Function<'_>| {
                    let start = Instant::now();
                    let res: rquickjs::Result<Value> = cb.call(());
                    let duration_ms = start.elapsed().as_millis() as u64;

                    let (passed, error_message) = match res {
                        Ok(_) => (true, None),
                        Err(e) => {
                            let err_msg = format_js_error(&ctx, e);
                            (false, Some(err_msg))
                        }
                    };

                    tr_store.borrow_mut().push(TestResult {
                        name,
                        passed,
                        error_message,
                        duration_ms,
                    });
                },
            )
            .map_err(|e| format!("{:?}", e))?;
            client
                .set("test", test_fn)
                .map_err(|e| format!("{:?}", e))?;

            // z.assert(condition, msg?)
            let assert_fn = Function::new(
                ctx.clone(),
                |ctx: rquickjs::Ctx<'_>,
                 condition: bool,
                 msg: Opt<String>|
                 -> rquickjs::Result<()> {
                    if !condition {
                        let m = msg.0.unwrap_or_else(|| "Assertion failed".to_string());
                        let js_str = rquickjs::String::from_str(ctx.clone(), &m)?;
                        Err(ctx.throw(js_str.into()))
                    } else {
                        Ok(())
                    }
                },
            )
            .map_err(|e| format!("{:?}", e))?;
            client
                .set("assert", assert_fn)
                .map_err(|e| format!("{:?}", e))?;

            ctx.globals()
                .set("z", client)
                .map_err(|e| format!("{:?}", e))?;

            inject_console(
                &ctx,
                console_entries.clone(),
                ConsoleLogSource::PostResponse,
            )
            .map_err(|e| format!("{:?}", e))?;

            let eval_result: Result<Value, _> = ctx.eval(code.as_bytes());
            if let Err(e) = eval_result {
                return Err(eval_error_message(&ctx, e, deadline, cancel));
            }

            Ok(())
        });

        let error = eval_res.err();
        let env_mutations = env_mutations.borrow().clone();
        let test_results = test_results.borrow().clone();
        let mut console_entries = console_entries.borrow().clone();
        if let Some(err) = &error {
            console_entries.push(ConsoleLogEntry {
                source: ConsoleLogSource::PostResponse,
                level: ConsoleLogLevel::Error,
                message: err.clone(),
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            });
        }
        let console_logs = console_entries.iter().map(|e| e.message.clone()).collect();

        PostResponseOutput {
            env_mutations,
            test_results,
            console_logs,
            console_entries,
            error,
        }
    }
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
