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

/// Transport protocol of a request. HTTP is the implicit default so existing
/// YAML files (which have no `protocol` key) deserialize unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Http,
    WebSocket,
    Grpc,
    GraphQL,
    Sse,
}

impl Protocol {
    pub fn is_http(&self) -> bool {
        matches!(self, Self::Http)
    }

    /// True for protocols whose exchange is a plain HTTP request/response
    /// (GraphQL rides the HTTP executor; SSE streams its HTTP response).
    pub fn uses_http_transport(&self) -> bool {
        matches!(self, Self::Http | Self::GraphQL | Self::Sse)
    }

    /// True for long-lived connection protocols (WS) or streamed calls (gRPC)
    /// that render the realtime Connect/Disconnect affordances.
    pub fn is_realtime(&self) -> bool {
        matches!(self, Self::WebSocket | Self::Grpc)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::WebSocket => "WS",
            Self::Grpc => "gRPC",
            Self::GraphQL => "GraphQL",
            Self::Sse => "SSE",
        }
    }
}

/// Composition format of the outgoing WebSocket message in the Message tab.
/// `Text` sends the editor content verbatim; `Json` validates the content
/// parses as JSON before sending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WsMessageFormat {
    #[default]
    Text,
    Json,
}

impl WsMessageFormat {
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    pub fn is_json(&self) -> bool {
        matches!(self, Self::Json)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Json => "JSON",
        }
    }
}

/// gRPC-specific request payload, present only when `protocol == Grpc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GrpcPayload {
    /// Optional path to a `.proto` file. When `None` and `use_reflection` is
    /// true, the schema is fetched via gRPC Server Reflection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proto_path: Option<String>,
    #[serde(default = "default_true")]
    pub use_reflection: bool,
    /// Fully-qualified service name, e.g. `helloworld.Greeter`.
    #[serde(default)]
    pub service: String,
    /// Method name within `service`, e.g. `SayHello`.
    #[serde(default)]
    pub method: String,
    /// Request message(s) as raw JSON. Unary/server-streaming hold a single
    /// JSON object; the editor content is verbatim (not parsed at rest).
    #[serde(default)]
    pub message_json: String,
}

/// GraphQL-specific request payload, present only when `protocol == GraphQL`.
/// The query is sent as an HTTP POST with a JSON body built by
/// [`RequestItem::to_graphql_http_item`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GraphQLPayload {
    /// The GraphQL query or mutation document (verbatim, not parsed at rest).
    #[serde(default)]
    pub query: String,
    /// Variables as raw JSON (object literal). Empty string means "no
    /// variables"; invalid JSON fails the send.
    #[serde(default)]
    pub variables: String,
    /// Operation name for documents containing multiple operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
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

    /// Human-readable display label for UI dropdowns, banners, and tabs.
    pub fn display_label(&self) -> &'static str {
        match self {
            Auth::Inherit => "Inherit",
            Auth::None => "None",
            Auth::Bearer { .. } => "Bearer Token",
            Auth::Basic { .. } => "Basic Auth",
            Auth::ApiKey { .. } => "API Key",
        }
    }

    /// Returns the HTTP header (key, value) contributed by this auth configuration,
    /// transforming secret values using the supplied `mask_fn`.
    /// Returns `None` if this auth mode does not produce an HTTP header (e.g. `None`, `Inherit`,
    /// or `ApiKey` placed in query params).
    pub fn contributed_header_preview<F>(&self, mask_fn: F) -> Option<(String, String)>
    where
        F: Fn(&str) -> String,
    {
        match self {
            Auth::Bearer { token } => Some((
                "Authorization".to_string(),
                format!("Bearer {}", mask_fn(token)),
            )),
            Auth::Basic { username, password } => Some((
                "Authorization".to_string(),
                format!("Basic {}:{}", username, mask_fn(password)),
            )),
            Auth::ApiKey {
                key,
                value,
                add_to: ApiKeyLocation::Header,
            } if !key.trim().is_empty() => Some((key.clone(), mask_fn(value))),
            _ => None,
        }
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
