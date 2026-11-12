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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyLocation {
    #[default]
    Header,
    Query,
}

impl ApiKeyLocation {
    pub fn is_header(&self) -> bool {
        matches!(self, Self::Header)
    }

    pub fn is_query(&self) -> bool {
        matches!(self, Self::Query)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Query => "query",
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
        #[serde(default)]
        add_to: ApiKeyLocation,
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

    pub fn set_api_key(&mut self, key: String, value: String, add_to: ApiKeyLocation) {
        self.auth = Auth::ApiKey { key, value, add_to };
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
