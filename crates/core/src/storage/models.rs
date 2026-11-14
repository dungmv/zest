use crate::models::{
    Auth, Environment, EnvironmentVariable, GrpcPayload, HttpMethod, HttpResponseRecord,
    KeyValuePair, MultipartItem, Protocol, RequestBody, RequestItem, RequestSentInfo,
    RequestSettings, ScriptConfig, TimingBreakdown,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_true() -> bool {
    true
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceMeta {
    #[serde(default = "default_version")]
    pub version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<KeyValuePair>,
    #[serde(default, skip_serializing_if = "Auth::is_none")]
    pub auth: Auth,
}

impl Default for WorkspaceMeta {
    fn default() -> Self {
        Self {
            version: 1,
            name: "My Workspace".to_string(),
            headers: Vec::new(),
            auth: Auth::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FolderFile {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<KeyValuePair>,
    #[serde(default, skip_serializing_if = "Auth::is_inherit")]
    pub auth: Auth,
}

pub type RequestKvItem = KeyValuePair;
pub type RequestAuth = Auth;
pub type MultipartItemFile = MultipartItem;
pub type RequestBodyFile = RequestBody;
pub type RequestSettingsFile = RequestSettings;
pub type RequestScriptFile = ScriptConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestExampleFile {
    pub name: String,
    pub status: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<RequestKvItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Request YAML model:
/// Fixed key order:
/// 1. name
/// 2. method
/// 3. url
/// 4. protocol (omitted for HTTP)
/// 5. grpc (omitted unless protocol is gRPC)
/// 6. params
/// 7. headers
/// 8. auth
/// 9. body
/// 10. settings
/// 11. script
/// 12. examples (ALWAYS last)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestFile {
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    #[serde(default, skip_serializing_if = "Protocol::is_http")]
    pub protocol: Protocol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grpc: Option<GrpcPayload>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<RequestKvItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<RequestKvItem>,
    #[serde(default)]
    pub auth: RequestAuth,
    #[serde(default)]
    pub body: RequestBodyFile,
    #[serde(default)]
    pub settings: RequestSettingsFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<RequestScriptFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<RequestExampleFile>,
}

impl Default for RequestFile {
    fn default() -> Self {
        Self {
            name: "New Request".to_string(),
            method: HttpMethod::Get,
            url: String::new(),
            protocol: Protocol::Http,
            grpc: None,
            params: Vec::new(),
            headers: Vec::new(),
            auth: RequestAuth::None,
            body: RequestBodyFile::None,
            settings: RequestSettingsFile::default(),
            script: None,
            examples: Vec::new(),
        }
    }
}

impl RequestFile {
    pub fn from_item(item: &RequestItem) -> Self {
        let mut file = Self::default();
        file.update_from_item(item);
        file
    }

    pub fn from_item_owned(item: RequestItem) -> Self {
        Self {
            name: item.name,
            method: item.method,
            url: item.url,
            protocol: item.protocol,
            grpc: item.grpc,
            params: item.params,
            headers: item.headers,
            auth: item.auth,
            body: item.body,
            settings: item.settings,
            script: item.script,
            examples: item.examples,
        }
    }

    pub fn into_item(
        self,
        id: impl Into<String>,
        workspace_id: impl Into<String>,
        folder_id: Option<String>,
    ) -> RequestItem {
        RequestItem {
            id: id.into(),
            workspace_id: workspace_id.into(),
            folder_id,
            name: self.name,
            method: self.method,
            url: self.url,
            protocol: self.protocol,
            grpc: self.grpc,
            params: self.params,
            headers: self.headers,
            auth: self.auth,
            body: self.body,
            settings: self.settings,
            sort_order: 0,
            last_status_code: None,
            script: self.script,
            examples: self.examples,
        }
    }

    pub fn to_item(
        &self,
        id: impl Into<String>,
        workspace_id: impl Into<String>,
        folder_id: Option<String>,
    ) -> RequestItem {
        RequestItem {
            id: id.into(),
            workspace_id: workspace_id.into(),
            folder_id,
            name: self.name.clone(),
            method: self.method,
            url: self.url.clone(),
            protocol: self.protocol,
            grpc: self.grpc.clone(),
            params: self.params.clone(),
            headers: self.headers.clone(),
            auth: self.auth.clone(),
            body: self.body.clone(),
            settings: self.settings.clone(),
            sort_order: 0,
            last_status_code: None,
            script: self.script.clone(),
            examples: self.examples.clone(),
        }
    }

    pub fn update_from_item(&mut self, item: &RequestItem) {
        self.name = item.name.clone();
        self.method = item.method;
        self.url = item.url.clone();
        self.protocol = item.protocol;
        self.grpc = item.grpc.clone();
        self.params = item.params.clone();
        self.headers = item.headers.clone();
        self.auth = item.auth.clone();
        self.body = item.body.clone();
        self.settings = item.settings.clone();
        self.script = item.script.clone();
        self.examples = item.examples.clone();
    }
}

impl From<RequestItem> for RequestFile {
    fn from(item: RequestItem) -> Self {
        Self::from_item_owned(item)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVarFile {
    #[serde(alias = "key")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default)]
    pub secret: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentFile {
    #[serde(default)]
    pub variables: Vec<EnvVarFile>,
}

impl EnvironmentFile {
    pub fn to_environment(
        &self,
        id: impl Into<String>,
        workspace_id: impl Into<String>,
        name: impl Into<String>,
        secrets: &std::collections::HashMap<String, String>,
    ) -> Environment {
        let mut env = Environment::new(workspace_id, name);
        env.id = id.into();
        for v in &self.variables {
            let (init_val, cur_val) = if v.secret {
                let init = secrets
                    .get(&format!("{}:init", v.name))
                    .cloned()
                    .or_else(|| secrets.get(&v.name).cloned())
                    .unwrap_or_default();
                let cur = secrets.get(&v.name).cloned().unwrap_or_default();
                (init, cur)
            } else {
                let init = v.value.clone().unwrap_or_default();
                let cur = secrets.get(&v.name).cloned().unwrap_or_default();
                (init, cur)
            };
            env.variables.push(EnvironmentVariable {
                key: v.name.clone(),
                initial_value: init_val,
                current_value: cur_val,
                secret: v.secret,
            });
        }
        env
    }

    pub fn from_environment(
        env: &Environment,
    ) -> (Self, std::collections::HashMap<String, String>) {
        let mut env_file = EnvironmentFile::default();
        let mut secrets = std::collections::HashMap::new();

        for v in &env.variables {
            if v.secret {
                env_file.variables.push(EnvVarFile {
                    name: v.key.clone(),
                    value: None,
                    secret: true,
                    enabled: true,
                });
                if !v.current_value.is_empty() {
                    secrets.insert(v.key.clone(), v.current_value.clone());
                    if !v.initial_value.is_empty() && v.initial_value != v.current_value {
                        secrets.insert(format!("{}:init", v.key), v.initial_value.clone());
                    }
                } else if !v.initial_value.is_empty() {
                    secrets.insert(v.key.clone(), v.initial_value.clone());
                }
            } else {
                env_file.variables.push(EnvVarFile {
                    name: v.key.clone(),
                    value: Some(v.initial_value.clone()),
                    secret: false,
                    enabled: true,
                });
                if !v.current_value.is_empty() {
                    secrets.insert(v.key.clone(), v.current_value.clone());
                }
            }
        }
        (env_file, secrets)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocRequestInfo {
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocResponseInfo {
    pub status: u16,
    pub duration_ms: u64,
    pub size: usize,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub truncated: bool,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocFile {
    pub sent_at: String,
    pub request: LocRequestInfo,
    pub response: LocResponseInfo,
}

impl LocFile {
    pub fn from_record(record: &HttpResponseRecord) -> Self {
        let sent_req = record.request_sent.as_ref();
        let (method, url, req_headers, req_body) = if let Some(sent) = sent_req {
            (
                sent.method,
                sent.url.clone(),
                sent.headers.clone(),
                sent.body_preview.clone(),
            )
        } else {
            (HttpMethod::Get, String::new(), Vec::new(), None)
        };

        let is_binary = record.body.iter().take(1024).any(|&b| b == 0);
        let max_body_bytes = 512 * 1024;
        let (body_str, truncated) = if is_binary {
            (None, false)
        } else if record.body.len() > max_body_bytes {
            (
                Some(String::from_utf8_lossy(&record.body[..max_body_bytes]).to_string()),
                true,
            )
        } else {
            (
                Some(String::from_utf8_lossy(&record.body).to_string()),
                false,
            )
        };

        Self {
            sent_at: record.created_at.to_rfc3339(),
            request: LocRequestInfo {
                method,
                url,
                headers: req_headers,
                body: req_body,
            },
            response: LocResponseInfo {
                status: record.status_code,
                duration_ms: record.duration_ms,
                size: record.size_bytes,
                headers: record.headers.clone(),
                body: body_str,
                truncated,
                binary: is_binary,
            },
        }
    }

    pub fn to_record(&self, request_id: &str) -> HttpResponseRecord {
        let body_bytes = self
            .response
            .body
            .as_deref()
            .unwrap_or("")
            .as_bytes()
            .to_vec();
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.sent_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let reason = crate::models::status_code_to_reason(self.response.status);
        let status_text = if reason.is_empty() {
            format!("HTTP {}", self.response.status)
        } else {
            reason.to_string()
        };

        HttpResponseRecord {
            id: format!("loc-{}", request_id),
            request_id: request_id.to_string(),
            status_code: self.response.status,
            status_text,
            duration_ms: self.response.duration_ms,
            size_bytes: self.response.size,
            headers: self.response.headers.clone(),
            cookies: Vec::new(),
            body: body_bytes,
            timing: TimingBreakdown {
                total_ms: self.response.duration_ms,
                ..Default::default()
            },
            request_sent: Some(RequestSentInfo {
                method: self.request.method,
                url: self.request.url.clone(),
                headers: self.request.headers.clone(),
                body_size_bytes: self.request.body.as_ref().map(|b| b.len()).unwrap_or(0),
                body_preview: self.request.body.clone(),
            }),
            test_results: Vec::new(),
            console_logs: Vec::new(),
            created_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PinsFile {
    #[serde(default)]
    pub pins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SidebarState {
    pub width: f32,
    pub collapsed: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            width: 260.0,
            collapsed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutState {
    pub split: String,
    pub request_pane: String,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            split: "horizontal".to_string(),
            request_pane: "editor".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PerRequestState {
    pub tab: String,
    pub response_scroll: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LocalState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_environment: Option<String>,
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub expanded_folders: Vec<String>,
    #[serde(default)]
    pub sidebar: SidebarState,
    #[serde(default)]
    pub layout: LayoutState,
    #[serde(default)]
    pub per_request: BTreeMap<String, PerRequestState>,
}

#[derive(Debug, Clone)]
pub struct RequestNode {
    pub slug: String,
    pub rel_path: String,
    pub file: RequestFile,
    pub last_status_code: Option<u16>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FolderNode {
    pub dir_name: String,
    pub rel_path: String,
    pub file: FolderFile,
    pub requests: Vec<RequestNode>,
    pub subfolders: Vec<FolderNode>,
}

impl FolderNode {
    /// Recursively counts all requests contained in this folder and its subfolders.
    pub fn count_requests(&self) -> usize {
        let mut count = self.requests.len();
        for sub in &self.subfolders {
            count += sub.count_requests();
        }
        count
    }

    /// Recursively counts this folder and all nested subfolders.
    pub fn count_folders(&self) -> usize {
        let mut count = 1;
        for sub in &self.subfolders {
            count += sub.count_folders();
        }
        count
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceTree {
    pub root_requests: Vec<RequestNode>,
    pub folders: Vec<FolderNode>,
}

impl WorkspaceTree {
    /// Recursively counts all requests contained across the entire workspace tree.
    pub fn count_requests(&self) -> usize {
        let mut count = self.root_requests.len();
        for f in &self.folders {
            count += f.count_requests();
        }
        count
    }

    /// Recursively counts all folders contained across the entire workspace tree.
    pub fn count_folders(&self) -> usize {
        let mut count = 0;
        for f in &self.folders {
            count += f.count_folders();
        }
        count
    }
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
