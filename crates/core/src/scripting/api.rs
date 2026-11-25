use crate::models::{
    ConsoleLogEntry, Environment, EnvironmentVariable, HttpResponseRecord, KeyValuePair,
    RequestBody, RequestItem, TestResult,
};

#[derive(Debug, Clone, Default)]
pub struct PreRequestContext {
    pub env_vars: Vec<EnvironmentVariable>,
    pub request_headers: Vec<KeyValuePair>,
    pub request_body_json: Option<String>,
}

impl PreRequestContext {
    pub fn from_request_and_env(item: &RequestItem, env: Option<&Environment>) -> Self {
        let env_vars = env.map(|e| e.variables.clone()).unwrap_or_default();
        let request_body_json = match &item.body {
            RequestBody::Json { content: raw_json } => Some(raw_json.clone()),
            _ => None,
        };

        Self {
            env_vars,
            request_headers: item.headers.clone(),
            request_body_json,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PreRequestOutput {
    /// List of (key, Some(val)) if set, or (key, None) if unset
    pub env_mutations: Vec<(String, Option<String>)>,
    /// Added or overridden headers
    pub header_mutations: Vec<(String, String)>,
    /// Present if the script invoked z.request.body.setJson(...)
    pub body_override: Option<String>,
    /// Script execution error if any (non-fatal, will not crash the app)
    pub error: Option<String>,
    /// Logs captured from console.log(...)
    pub console_logs: Vec<String>,
    /// Structured console logs (source, level, timestamp)
    pub console_entries: Vec<ConsoleLogEntry>,
}

#[derive(Debug, Clone)]
pub struct PostResponseContext {
    pub env_vars: Vec<EnvironmentVariable>,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub response_headers: Vec<(String, String)>,
    /// Response body as UTF-8 string (safety bound: max 10MB)
    pub response_body: String,
}

impl PostResponseContext {
    pub fn from_response_and_env(record: &HttpResponseRecord, env: Option<&Environment>) -> Self {
        let env_vars = env.map(|e| e.variables.clone()).unwrap_or_default();
        const MAX_STR_LEN: usize = 10 * 1024 * 1024;
        let slice = &record.body[..record.body.len().min(MAX_STR_LEN)];
        let response_body = match std::str::from_utf8(slice) {
            Ok(s) => s.to_string(),
            Err(err) if record.body.len() > MAX_STR_LEN && err.error_len().is_none() => {
                std::str::from_utf8(&slice[..err.valid_up_to()])
                    .unwrap_or("")
                    .to_string()
            }
            Err(_) => String::new(),
        };

        Self {
            env_vars,
            status_code: record.status_code,
            response_time_ms: record.duration_ms,
            response_headers: record.headers.clone(),
            response_body,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PostResponseOutput {
    pub env_mutations: Vec<(String, Option<String>)>,
    pub test_results: Vec<TestResult>,
    pub console_logs: Vec<String>,
    pub console_entries: Vec<ConsoleLogEntry>,
    pub error: Option<String>,
}
