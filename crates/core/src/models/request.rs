use serde::{Deserialize, Serialize};

use super::request_types::{
    ApiKeyLocation, Auth, GraphQLPayload, GrpcPayload, HttpMethod, KeyValuePair, MultipartItem,
    Protocol, RequestBody, RequestSettings, ScriptConfig, WsMessageFormat,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestItem {
    pub id: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    #[serde(default, skip_serializing_if = "Protocol::is_http")]
    pub protocol: Protocol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grpc: Option<GrpcPayload>,
    /// GraphQL query payload, present only when `protocol == GraphQL`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graphql: Option<GraphQLPayload>,
    /// Outgoing message format for WebSocket requests (omitted for Text).
    #[serde(default, skip_serializing_if = "WsMessageFormat::is_text")]
    pub ws_message_format: WsMessageFormat,
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

impl RequestItem {
    pub fn new(
        workspace_id: impl Into<String>,
        name: impl Into<String>,
        method: HttpMethod,
        url: impl Into<String>,
    ) -> Self {
        Self::new_with_protocol(workspace_id, name, method, url, Protocol::Http)
    }

    pub fn new_with_protocol(
        workspace_id: impl Into<String>,
        name: impl Into<String>,
        method: HttpMethod,
        url: impl Into<String>,
        protocol: Protocol,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: workspace_id.into(),
            folder_id: None,
            name: name.into(),
            method,
            url: url.into(),
            protocol,
            grpc: None,
            graphql: None,
            ws_message_format: WsMessageFormat::default(),
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
            let (base, parsed_params) = crate::transport::parse_query_from_url(url);
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

    /// Writes the gRPC request message JSON, creating the payload if needed.
    pub fn set_grpc_message_json(&mut self, json: String) {
        self.grpc_mut().message_json = json;
    }

    /// Mutable view of the gRPC payload, created on demand.
    pub fn grpc_mut(&mut self) -> &mut GrpcPayload {
        self.grpc.get_or_insert_with(GrpcPayload::default)
    }

    /// Mutable view of the GraphQL payload, created on demand.
    pub fn graphql_mut(&mut self) -> &mut GraphQLPayload {
        self.graphql.get_or_insert_with(GraphQLPayload::default)
    }

    /// Converts a GraphQL request into the equivalent HTTP POST item that
    /// goes over the wire: method POST, JSON body
    /// `{query, variables?, operationName?}`, `Content-Type: application/json`.
    /// Query params, headers, auth, settings and scripts are carried over
    /// unchanged so the whole HTTP pipeline (cancel, progress, response
    /// cache, tests) applies verbatim.
    ///
    /// Fails when the query is empty or `variables` is not a JSON object.
    pub fn to_graphql_http_item(&self) -> Result<RequestItem, String> {
        const EMPTY_QUERY: &str =
            "GraphQL query is empty — open the Query tab and write a query first.";
        let payload = self.graphql.as_ref().ok_or(EMPTY_QUERY)?;
        if payload.query.trim().is_empty() {
            return Err(EMPTY_QUERY.to_string());
        }
        let variables = if payload.variables.trim().is_empty() {
            None
        } else {
            let parsed: serde_json::Value = serde_json::from_str(&payload.variables)
                .map_err(|e| format!("Variables is not valid JSON: {e}"))?;
            if !parsed.is_object() {
                return Err("Variables must be a JSON object.".to_string());
            }
            Some(parsed)
        };

        let mut body = serde_json::Map::new();
        body.insert(
            "query".to_string(),
            serde_json::Value::String(payload.query.clone()),
        );
        if let Some(vars) = variables {
            body.insert("variables".to_string(), vars);
        }
        if let Some(op) = &payload.operation_name {
            if !op.trim().is_empty() {
                body.insert(
                    "operationName".to_string(),
                    serde_json::Value::String(op.clone()),
                );
            }
        }

        let mut item = self.clone();
        item.method = HttpMethod::Post;
        item.body = RequestBody::json(serde_json::Value::Object(body).to_string());
        item.update_content_type_header(Some("application/json"));
        Ok(item)
    }

    /// Sets the outgoing WebSocket message format (Text or JSON).
    pub fn set_ws_message_format(&mut self, format: WsMessageFormat) {
        self.ws_message_format = format;
    }

    /// Read-only view of the gRPC request message JSON (empty when absent).
    pub fn grpc_message_json(&self) -> &str {
        self.grpc
            .as_ref()
            .map(|g| g.message_json.as_str())
            .unwrap_or("")
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

    pub fn apply_imported_curl(&mut self, parsed: RequestItem) {
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
