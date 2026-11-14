//! Tokio-side pipeline stages shared by the HTTP, WebSocket and gRPC
//! engines. Each engine runs the same two stages before its
//! transport-specific work: fake vars + pre-request script, then merging
//! the script's env mutations into the effective environment.

use std::sync::Arc;

use zest_core::models::{Environment, EnvironmentVariable, RequestItem};
use zest_core::scripting::{
    resolve_fake_vars_in_request, PreRequestContext, PreRequestOutput, ScriptSandbox,
};

/// Stage 1 of every engine pipeline: resolve dynamic fake variables in the
/// request, then run the pre-request script when one is configured.
pub fn run_pre_request_stage(
    req_item: &mut RequestItem,
    env: Option<&Environment>,
) -> PreRequestOutput {
    resolve_fake_vars_in_request(req_item);
    let pre_script_code = req_item.script.as_ref().and_then(|s| s.pre_request.clone());
    match pre_script_code {
        Some(code) => {
            let ctx = PreRequestContext::from_request_and_env(req_item, env);
            ScriptSandbox::run_pre_request(&code, ctx)
        }
        None => PreRequestOutput::default(),
    }
}

/// Stage 2: merges `(key, Some(value))` / `(key, None)` env mutations from
/// the pre-request script into a copy of the effective environment. Returns
/// an untouched clone of `env` when there are no mutations.
pub fn merge_env_mutations(
    env: &Option<Arc<Environment>>,
    mutations: &[(String, Option<String>)],
) -> Option<Arc<Environment>> {
    if mutations.is_empty() {
        return env.clone();
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
    Some(Arc::new(temp))
}
