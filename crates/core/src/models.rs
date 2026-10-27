use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    #[default]
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            "PATCH" => Ok(HttpMethod::Patch),
            "HEAD" => Ok(HttpMethod::Head),
            "OPTIONS" => Ok(HttpMethod::Options),
            _ => Err(format!("Unknown HTTP method: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct KeyValuePair {
    #[serde(rename = "name", alias = "key")]
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

impl KeyValuePair {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            enabled: true,
            key: key.into(),
            value: value.into(),
            description: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Auth {
    #[default]
    Inherit,
    None,
    Bearer {
        token: String,
    },
    Basic {
        username: String,
        password: String,
    },
    #[serde(alias = "api_key")]
    ApiKey {
        key: String,
        value: String,
        #[serde(default = "default_true")]
        in_header: bool,
    },
}

impl Auth {
    pub fn is_inherit(&self) -> bool {
        matches!(self, Auth::Inherit)
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Auth::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipartItem {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(rename = "name", alias = "key")]
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub is_file: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

impl Default for MultipartItem {
    fn default() -> Self {
        Self {
            enabled: true,
            key: String::new(),
            value: String::new(),
            is_file: false,
            file_path: None,
        }
    }
}

impl MultipartItem {
    pub fn new_text(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            enabled: true,
            key: key.into(),
            value: value.into(),
            is_file: false,
            file_path: None,
        }
    }

    pub fn new_file(key: impl Into<String>, file_path: impl Into<String>) -> Self {
        let path = file_path.into();
        Self {
            enabled: true,
            key: key.into(),
            value: path.clone(),
            is_file: true,
            file_path: Some(path),
        }
    }
}

impl From<KeyValuePair> for MultipartItem {
    fn from(kv: KeyValuePair) -> Self {
        Self {
            enabled: kv.enabled,
            key: kv.key,
            value: kv.value,
            is_file: false,
            file_path: None,
        }
    }
}

fn default_raw_type() -> String {
    "text/plain".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RequestBody {
    #[default]
    None,
    Json {
        #[serde(default)]
        content: String,
    },
    #[serde(rename = "form")]
    FormUrlEncoded {
        #[serde(default, rename = "params")]
        params: Vec<KeyValuePair>,
    },
    Multipart {
        #[serde(default, rename = "items")]
        items: Vec<MultipartItem>,
    },
    Raw {
        #[serde(default)]
        content: String,
        #[serde(default = "default_raw_type")]
        content_type: String,
    },
    Binary {
        #[serde(default)]
        file_path: String,
    },
}

impl RequestBody {
    pub fn json(content: impl Into<String>) -> Self {
        Self::Json {
            content: content.into(),
        }
    }

    pub fn form(params: Vec<KeyValuePair>) -> Self {
        Self::FormUrlEncoded { params }
    }

    pub fn multipart(items: Vec<MultipartItem>) -> Self {
        Self::Multipart { items }
    }

    pub fn raw(content: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self::Raw {
            content: content.into(),
            content_type: content_type.into(),
        }
    }

    pub fn binary(file_path: impl Into<String>) -> Self {
        Self::Binary {
            file_path: file_path.into(),
        }
    }
}

fn default_timeout_ms() -> u64 {
    30000
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestSettings {
    #[serde(default = "default_timeout_ms", alias = "timeout_secs")]
    pub timeout_ms: u64,
    #[serde(default = "default_true")]
    pub follow_redirects: bool,
    #[serde(default = "default_true")]
    pub validate_tls: bool,
}

impl Default for RequestSettings {
    fn default() -> Self {
        Self {
            timeout_ms: default_timeout_ms(),
            follow_redirects: true,
            validate_tls: true,
        }
    }
}

impl RequestSettings {
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_ms / 1000
    }

    pub fn set_timeout_secs(&mut self, secs: u64) {
        self.timeout_ms = secs * 1000;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ScriptConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_response: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestItem {
    pub id: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub params: Vec<KeyValuePair>,
    #[serde(default)]
    pub headers: Vec<KeyValuePair>,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub body: RequestBody,
    #[serde(default)]
    pub settings: RequestSettings,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<ScriptConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<crate::storage::models::RequestExampleFile>,
}

impl HttpRequestItem {
    pub fn new(
        workspace_id: impl Into<String>,
        name: impl Into<String>,
        method: HttpMethod,
        url: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: workspace_id.into(),
            folder_id: None,
            name: name.into(),
            method,
            url: url.into(),
            params: Vec::new(),
            headers: Vec::new(),
            auth: Auth::Inherit,
            body: RequestBody::None,
            settings: RequestSettings::default(),
            sort_order: 0,
            last_status_code: None,
            script: None,
            examples: Vec::new(),
        }
    }

    /// Sets the URL and extracts query parameters into `self.params` if query parameters exist.
    /// Returns `(has_query, has_template)`.
    pub fn apply_url(&mut self, url: &str) -> (bool, bool) {
        let has_query = url.contains('?');
        let has_template = url.contains("{{");
        if has_query {
            let (base, parsed_params) = crate::http::parse_query_from_url(url);
            self.url = base;
            if !parsed_params.is_empty() {
                let mut disabled: Vec<KeyValuePair> =
                    self.params.iter().filter(|p| !p.enabled).cloned().collect();
                let mut final_params = parsed_params;
                final_params.append(&mut disabled);
                self.params = final_params;
            }
        } else {
            self.url = url.to_string();
        }
        (has_query, has_template)
    }

    pub fn set_method(&mut self, method: HttpMethod) {
        self.method = method;
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn update_param(&mut self, index: usize, key: String, val: String, enabled: bool) {
        if index < self.params.len() {
            self.params[index].key = key;
            self.params[index].value = val;
            self.params[index].enabled = enabled;
        }
    }

    pub fn add_param(&mut self) {
        self.params.push(KeyValuePair::new("", ""));
    }

    pub fn remove_param(&mut self, index: usize) {
        if index < self.params.len() {
            self.params.remove(index);
        }
    }

    pub fn update_header(&mut self, index: usize, key: String, val: String, enabled: bool) {
        if index < self.headers.len() {
            self.headers[index].key = key;
            self.headers[index].value = val;
            self.headers[index].enabled = enabled;
        }
    }

    pub fn add_header(&mut self) {
        self.headers.push(KeyValuePair::new("", ""));
    }

    pub fn remove_header(&mut self, index: usize) {
        if index < self.headers.len() {
            self.headers.remove(index);
        }
    }

    pub fn set_body_json(&mut self, json_str: String) {
        self.body = RequestBody::json(json_str);
    }

    pub fn set_body_type(&mut self, body: RequestBody) {
        self.body = body;
    }

    pub fn update_content_type_header(&mut self, content_type: Option<&str>) {
        if let Some(ct) = content_type {
            if let Some(header) = self
                .headers
                .iter_mut()
                .find(|h| h.key.eq_ignore_ascii_case("content-type"))
            {
                header.value = ct.to_string();
                header.enabled = true;
            } else {
                self.headers.push(KeyValuePair::new("Content-Type", ct));
            }
        } else {
            self.headers
                .retain(|h| !h.key.eq_ignore_ascii_case("content-type"));
        }
    }

    pub fn update_form_param(&mut self, index: usize, key: String, val: String, enabled: bool) {
        if let RequestBody::FormUrlEncoded { params } = &mut self.body {
            if index < params.len() {
                params[index].key = key;
                params[index].value = val;
                params[index].enabled = enabled;
            }
        }
    }

    pub fn add_form_param(&mut self) {
        if let RequestBody::FormUrlEncoded { params } = &mut self.body {
            params.push(KeyValuePair::new("", ""));
        }
    }

    pub fn remove_form_param(&mut self, index: usize) {
        if let RequestBody::FormUrlEncoded { params } = &mut self.body {
            if index < params.len() {
                params.remove(index);
            }
        }
    }

    pub fn update_multipart_item(
        &mut self,
        index: usize,
        key: String,
        val: String,
        is_file: bool,
        file_path: Option<String>,
        enabled: bool,
    ) {
        if let RequestBody::Multipart { items } = &mut self.body {
            if index < items.len() {
                items[index].key = key;
                items[index].value = val;
                items[index].is_file = is_file;
                items[index].file_path = file_path;
                items[index].enabled = enabled;
            }
        }
    }

    pub fn add_multipart_item(&mut self) {
        if let RequestBody::Multipart { items } = &mut self.body {
            items.push(MultipartItem::default());
        }
    }

    pub fn remove_multipart_item(&mut self, index: usize) {
        if let RequestBody::Multipart { items } = &mut self.body {
            if index < items.len() {
                items.remove(index);
            }
        }
    }

    pub fn set_raw_body(&mut self, content: String, content_type: String) {
        self.body = RequestBody::Raw {
            content,
            content_type,
        };
    }

    pub fn set_binary_file(&mut self, file_path: String) {
        self.body = RequestBody::Binary { file_path };
    }

    pub fn set_auth(&mut self, auth: Auth) {
        self.auth = auth;
    }

    pub fn set_bearer_token(&mut self, token: String) {
        self.auth = Auth::Bearer { token };
    }

    pub fn set_basic_auth(&mut self, username: String, password: String) {
        self.auth = Auth::Basic { username, password };
    }

    pub fn set_api_key(&mut self, key: String, value: String, in_header: bool) {
        self.auth = Auth::ApiKey {
            key,
            value,
            in_header,
        };
    }

    pub fn set_timeout_secs(&mut self, secs: u64) {
        self.settings.set_timeout_secs(secs);
    }

    pub fn set_follow_redirects(&mut self, follow: bool) {
        self.settings.follow_redirects = follow;
    }

    pub fn set_validate_tls(&mut self, validate: bool) {
        self.settings.validate_tls = validate;
    }

    pub fn set_pre_script(&mut self, code: String) {
        let script = self.script.get_or_insert_with(Default::default);
        script.pre_request = if code.trim().is_empty() {
            None
        } else {
            Some(code)
        };
    }

    pub fn set_test_script(&mut self, code: String) {
        let script = self.script.get_or_insert_with(Default::default);
        script.post_response = if code.trim().is_empty() {
            None
        } else {
            Some(code)
        };
    }

    pub fn apply_imported_curl(&mut self, parsed: HttpRequestItem) {
        if self.name == "New Request" || self.name == "Untitled Request" || self.name.is_empty() {
            self.name = parsed.name;
        }
        self.method = parsed.method;
        self.url = parsed.url;
        self.params = parsed.params;
        self.headers = parsed.headers;
        self.auth = parsed.auth;
        self.body = parsed.body;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub headers: Vec<KeyValuePair>,
    #[serde(default)]
    pub auth: Auth,
}

impl Folder {
    pub fn new(workspace_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: workspace_id.into(),
            name: name.into(),
            parent_id: None,
            sort_order: 0,
            headers: Vec::new(),
            auth: Auth::Inherit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EnvironmentVariable {
    pub key: String,
    #[serde(default)]
    pub initial_value: String,
    #[serde(default)]
    pub current_value: String,
    #[serde(default)]
    pub secret: bool,
}

impl EnvironmentVariable {
    pub fn resolved_value(&self) -> &str {
        if !self.current_value.is_empty() {
            &self.current_value
        } else {
            &self.initial_value
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    #[serde(default)]
    pub variables: Vec<EnvironmentVariable>,
}

impl Environment {
    pub fn new(workspace_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: workspace_id.into(),
            name: name.into(),
            variables: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_environment_id: Option<String>,
    #[serde(default)]
    pub headers: Vec<KeyValuePair>,
    #[serde(default)]
    pub auth: Auth,
}

impl Workspace {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: String::new(),
            active_environment_id: None,
            headers: Vec::new(),
            auth: Auth::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TimingBreakdown {
    pub total_ms: u64,
    pub dns_ms: Option<u64>,
    pub connect_ms: Option<u64>,
    pub tls_ms: Option<u64>,
    pub ttfb_ms: Option<u64>,
    pub download_ms: Option<u64>,
    #[serde(default)]
    pub connection_reused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RequestSentInfo {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body_size_bytes: usize,
    pub body_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedCookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<String>,
    pub max_age: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
}

impl ParsedCookie {
    pub fn parse(raw: &str) -> Self {
        let mut parts = raw.split(';').map(|s| s.trim());
        let first = parts.next().unwrap_or("");
        let (name, value) = if let Some((n, v)) = first.split_once('=') {
            (n.trim().to_string(), v.trim().to_string())
        } else {
            (first.to_string(), String::new())
        };

        let mut domain = None;
        let mut path = None;
        let mut expires = None;
        let mut max_age = None;
        let mut secure = false;
        let mut http_only = false;
        let mut same_site = None;

        for part in parts {
            if part.eq_ignore_ascii_case("secure") {
                secure = true;
            } else if part.eq_ignore_ascii_case("httponly") {
                http_only = true;
            } else if let Some((k, v)) = part.split_once('=') {
                let k_trim = k.trim();
                let v_trim = v.trim();
                if k_trim.eq_ignore_ascii_case("domain") {
                    domain = Some(v_trim.to_string());
                } else if k_trim.eq_ignore_ascii_case("path") {
                    path = Some(v_trim.to_string());
                } else if k_trim.eq_ignore_ascii_case("expires") {
                    expires = Some(v_trim.to_string());
                } else if k_trim.eq_ignore_ascii_case("max-age") {
                    max_age = v_trim.parse::<i64>().ok();
                } else if k_trim.eq_ignore_ascii_case("samesite") {
                    same_site = Some(v_trim.to_string());
                }
            }
        }

        Self {
            name,
            value,
            domain,
            path,
            expires,
            max_age,
            secure,
            http_only,
            same_site,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleLogSource {
    PreRequest,
    PostResponse,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleLogLevel {
    Log,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleLogEntry {
    pub source: ConsoleLogSource,
    pub level: ConsoleLogLevel,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HttpResponseRecord {
    pub id: String,
    pub request_id: String,
    pub status_code: u16,
    pub status_text: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<String>,
    pub body: Vec<u8>,
    pub timing: TimingBreakdown,
    #[serde(default)]
    pub request_sent: Option<RequestSentInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_results: Vec<TestResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub console_logs: Vec<ConsoleLogEntry>,
    pub created_at: DateTime<Utc>,
}

pub fn status_code_to_reason(code: u16) -> &'static str {
    reqwest::StatusCode::from_u16(code)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("")
}

impl HttpResponseRecord {
    pub fn body_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.body)
    }

    pub fn formatted_size(&self) -> String {
        let bytes = self.size_bytes;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }

    pub fn formatted_duration(&self) -> String {
        if self.duration_ms < 1000 {
            format!("{} ms", self.duration_ms)
        } else {
            format!("{:.2} s", self.duration_ms as f64 / 1000.0)
        }
    }

    pub fn parsed_cookies(&self) -> Vec<ParsedCookie> {
        self.cookies
            .iter()
            .map(|c| ParsedCookie::parse(c))
            .collect()
    }

    pub fn is_json(&self) -> bool {
        self.headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("content-type") && v.to_lowercase().contains("json")
        })
    }

    pub fn is_xml(&self) -> bool {
        self.headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("content-type")
                && (v.to_lowercase().contains("xml")
                    || v.to_lowercase().contains("application/xml")
                    || v.to_lowercase().contains("text/xml"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_code_to_reason() {
        assert_eq!(status_code_to_reason(200), "OK");
        assert_eq!(status_code_to_reason(201), "Created");
        assert_eq!(status_code_to_reason(404), "Not Found");
        assert_eq!(status_code_to_reason(500), "Internal Server Error");
        assert_eq!(status_code_to_reason(999), "");
    }

    #[test]
    fn test_http_request_item_mutations() {
        let mut req = HttpRequestItem::new("ws1", "Untitled Request", HttpMethod::Get, "");
        assert_eq!(req.name, "Untitled Request");
        assert_eq!(req.method, HttpMethod::Get);

        // URL & query params parsing
        let (has_query, has_template) =
            req.apply_url("https://api.example.com/items?q=rust&limit=20");
        assert!(has_query);
        assert!(!has_template);
        assert_eq!(req.url, "https://api.example.com/items");
        assert_eq!(req.params.len(), 2);
        assert_eq!(req.params[0].key, "q");
        assert_eq!(req.params[0].value, "rust");

        // Method & name
        req.set_method(HttpMethod::Post);
        assert_eq!(req.method, HttpMethod::Post);
        req.set_name("Get Items".to_string());
        assert_eq!(req.name, "Get Items");

        // Params
        req.update_param(1, "limit".to_string(), "50".to_string(), false);
        assert_eq!(req.params[1].value, "50");
        assert!(!req.params[1].enabled);
        req.add_param();
        assert_eq!(req.params.len(), 3);
        req.remove_param(2);
        assert_eq!(req.params.len(), 2);

        // Headers
        req.add_header();
        req.update_header(
            0,
            "Accept".to_string(),
            "application/json".to_string(),
            true,
        );
        assert_eq!(req.headers[0].key, "Accept");
        assert_eq!(req.headers[0].value, "application/json");
        req.remove_header(0);
        assert!(req.headers.is_empty());

        // Body
        req.set_body_json(r#"{"test": true}"#.to_string());
        assert_eq!(req.body, RequestBody::json(r#"{"test": true}"#));

        req.set_body_type(RequestBody::form(vec![KeyValuePair::new("k", "v")]));
        req.update_form_param(0, "k2".to_string(), "v2".to_string(), true);
        req.add_form_param();
        assert_eq!(
            req.body,
            RequestBody::form(vec![
                KeyValuePair::new("k2", "v2"),
                KeyValuePair::new("", "")
            ])
        );
        req.remove_form_param(1);

        req.set_body_type(RequestBody::multipart(vec![MultipartItem::default()]));
        req.update_multipart_item(0, "file".to_string(), "data".to_string(), false, None, true);
        req.add_multipart_item();
        req.remove_multipart_item(1);

        req.set_raw_body("hello".to_string(), "text/plain".to_string());
        assert_eq!(
            req.body,
            RequestBody::Raw {
                content: "hello".to_string(),
                content_type: "text/plain".to_string()
            }
        );

        req.set_binary_file("/tmp/file.bin".to_string());
        assert_eq!(
            req.body,
            RequestBody::Binary {
                file_path: "/tmp/file.bin".to_string()
            }
        );

        // Auth
        req.set_bearer_token("tok123".to_string());
        assert_eq!(
            req.auth,
            Auth::Bearer {
                token: "tok123".to_string()
            }
        );
        req.set_basic_auth("u".to_string(), "p".to_string());
        assert_eq!(
            req.auth,
            Auth::Basic {
                username: "u".to_string(),
                password: "p".to_string()
            }
        );
        req.set_api_key("x-api-key".to_string(), "secret".to_string(), true);
        assert_eq!(
            req.auth,
            Auth::ApiKey {
                key: "x-api-key".to_string(),
                value: "secret".to_string(),
                in_header: true
            }
        );

        // Settings
        req.set_timeout_secs(42);
        assert_eq!(req.settings.timeout_secs(), 42);
        req.set_follow_redirects(false);
        assert!(!req.settings.follow_redirects);
        req.set_validate_tls(false);
        assert!(!req.settings.validate_tls);

        // Scripts
        req.set_pre_script("console.log(1);".to_string());
        assert_eq!(
            req.script.as_ref().unwrap().pre_request.as_deref(),
            Some("console.log(1);")
        );
        req.set_test_script("z.test('ok');".to_string());
        assert_eq!(
            req.script.as_ref().unwrap().post_response.as_deref(),
            Some("z.test('ok');")
        );

        // Content-Type update
        req.update_content_type_header(Some("application/json"));
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Content-Type");
        assert_eq!(req.headers[0].value, "application/json");

        // Overwrite existing header case-insensitively
        req.headers[0].key = "content-type".to_string();
        req.update_content_type_header(Some("multipart/form-data"));
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].value, "multipart/form-data");

        // Clear header
        req.update_content_type_header(None);
        assert!(req.headers.is_empty());
    }
}
