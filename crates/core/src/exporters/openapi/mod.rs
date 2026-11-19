//! OpenAPI 3.0 exporter: converts a flat list of Zest requests into an
//! OpenAPI 3.0.3 document, serializable as YAML or JSON.
//!
//! The input DTO is [`ImportedRequest`] — the same neutral exchange model the
//! importers produce — so the export direction is the natural inverse of
//! `crate::importers::openapi::parse_openapi` (see the roundtrip tests).
//! Requests whose protocol does not ride the HTTP transport (WebSocket, gRPC)
//! cannot be represented in OpenAPI and are reported as warnings instead.
//!
//! Secrets (bearer tokens, basic credentials, API key values) are deliberately
//! not emitted: only the security *scheme* shapes land in the document.

use crate::importers::models::ImportedRequest;
use crate::models::{ApiKeyLocation, Auth, Protocol, RequestBody};
use crate::storage::slug::slugify;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

/// Result of an OpenAPI export: the document tree plus non-fatal warnings.
pub struct OpenApiExport {
    /// The OpenAPI 3.0 document as a JSON value (serializable to YAML or JSON).
    pub document: Value,
    /// Non-fatal notes (skipped non-HTTP requests, duplicate operations, ...).
    pub warnings: Vec<String>,
}

/// Output serialization of an exported OpenAPI document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenApiFileFormat {
    Yaml,
    Json,
}

impl OpenApiFileFormat {
    /// Default file extension for the format (without the leading dot).
    pub fn extension(self) -> &'static str {
        match self {
            OpenApiFileFormat::Yaml => "yaml",
            OpenApiFileFormat::Json => "json",
        }
    }

    /// Detects the format from a saved file extension; YAML is the default
    /// for unknown extensions.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "json" => OpenApiFileFormat::Json,
            _ => OpenApiFileFormat::Yaml,
        }
    }
}

/// Serializes an exported document to YAML or JSON text.
pub fn serialize_openapi(
    export: &OpenApiExport,
    format: OpenApiFileFormat,
) -> anyhow::Result<String> {
    match format {
        OpenApiFileFormat::Yaml => Ok(serde_yaml::to_string(&export.document)?),
        OpenApiFileFormat::Json => Ok(serde_json::to_string_pretty(&export.document)?),
    }
}

/// Converts requests into an OpenAPI 3.0.3 document.
///
/// `title`/`description` fill `info`; folder chain names in
/// `ImportedRequest::folder_path` become operation tags. Non-HTTP-transport
/// requests produce warnings instead of operations.
pub fn requests_to_openapi(
    title: &str,
    description: Option<&str>,
    requests: &[ImportedRequest],
) -> OpenApiExport {
    let mut builder = DocBuilder::default();
    for req in requests {
        builder.push_request(req);
    }
    OpenApiExport {
        document: builder.build(title, description),
        warnings: builder.warnings,
    }
}

/// Mutable state accumulated across requests.
#[derive(Default)]
struct DocBuilder {
    paths: Map<String, Value>,
    security_schemes: Map<String, Value>,
    tags: Vec<String>,
    seen_tags: HashSet<String>,
    servers: Vec<String>,
    seen_servers: HashSet<String>,
    warnings: Vec<String>,
    used_operation_ids: HashSet<String>,
}

impl DocBuilder {
    fn build(&self, title: &str, description: Option<&str>) -> Value {
        let mut doc = Map::new();
        doc.insert("openapi".into(), json!("3.0.3"));
        let mut info = json!({
            "title": title,
            "version": "1.0.0",
        })
        .as_object()
        .cloned()
        .unwrap_or_default();
        if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
            info.insert("description".into(), json!(d));
        }
        doc.insert("info".into(), Value::Object(info));
        if !self.servers.is_empty() {
            doc.insert(
                "servers".into(),
                Value::Array(
                    self.servers
                        .iter()
                        .map(|url| json!({ "url": url }))
                        .collect(),
                ),
            );
        }
        if !self.tags.is_empty() {
            doc.insert(
                "tags".into(),
                Value::Array(
                    self.tags
                        .iter()
                        .map(|name| json!({ "name": name }))
                        .collect(),
                ),
            );
        }
        doc.insert("paths".into(), Value::Object(self.paths.clone()));
        if !self.security_schemes.is_empty() {
            doc.insert(
                "components".into(),
                json!({ "securitySchemes": Value::Object(self.security_schemes.clone()) }),
            );
        }
        Value::Object(doc)
    }

    fn push_request(&mut self, req: &ImportedRequest) {
        if !req.protocol.uses_http_transport() {
            self.warnings.push(format!(
                "Skipped '{}' ({} — not representable in OpenAPI)",
                req.name,
                req.protocol.as_str()
            ));
            return;
        }

        let (server, raw_path, url_query) = split_request_url(&req.url);
        if let Some(s) = server {
            self.add_server(s);
        }
        let path_key = var_braces_to_openapi(&raw_path);

        let method_key = req.method.as_str().to_lowercase();
        // Duplicate check up front (no borrow held while building the operation).
        let duplicate = self
            .paths
            .get(&path_key)
            .and_then(|item| item.as_object())
            .is_some_and(|obj| obj.contains_key(&method_key));
        if duplicate {
            self.warnings.push(format!(
                "Skipped '{}' (duplicate operation {} {} — kept the first definition)",
                req.name,
                req.method.as_str(),
                path_key
            ));
            return;
        }

        for tag in req.folder_path.iter().filter(|t| !t.trim().is_empty()) {
            if self.seen_tags.insert(tag.clone()) {
                self.tags.push(tag.clone());
            }
        }

        let mut op = Map::new();
        let op_tags: Vec<&String> = req
            .folder_path
            .iter()
            .filter(|t| !t.trim().is_empty())
            .collect();
        if !op_tags.is_empty() {
            op.insert("tags".into(), json!(op_tags));
        }
        op.insert("summary".into(), json!(req.name));
        if let Some(d) = req.description.as_deref().filter(|d| !d.trim().is_empty()) {
            op.insert("description".into(), json!(d));
        }
        let operation_id = self.unique_operation_id(&req.name);
        op.insert("operationId".into(), json!(operation_id));

        let parameters = self.operation_parameters(req, &path_key, &url_query);
        if !parameters.is_empty() {
            op.insert("parameters".into(), Value::Array(parameters));
        }
        if let Some(security) = self.security_for(&req.auth) {
            op.insert("security".into(), security);
        }
        if let Some(request_body) = request_body_for(req) {
            op.insert("requestBody".into(), request_body);
        }
        op.insert("responses".into(), responses_for(req));

        let entry = self.paths.entry(path_key).or_insert_with(|| json!({}));
        if let Some(path_item) = entry.as_object_mut() {
            path_item.insert(method_key, Value::Object(op));
        }
    }

    fn add_server(&mut self, server: String) {
        if self.seen_servers.insert(server.clone()) {
            self.servers.push(server);
        }
    }

    fn unique_operation_id(&mut self, name: &str) -> String {
        let base = slugify(name);
        let mut candidate = base.clone();
        let mut n = 2;
        while !self.used_operation_ids.insert(candidate.clone()) {
            candidate = format!("{base}-{n}");
            n += 1;
        }
        candidate
    }

    /// Builds path/query/header parameters for one operation.
    fn operation_parameters(
        &self,
        req: &ImportedRequest,
        path_key: &str,
        url_query: &[(String, String)],
    ) -> Vec<Value> {
        let mut parameters = Vec::new();

        // Path tokens (from {{var}} templates and pre-existing braces) must be
        // declared to keep the document spec-valid.
        for name in extract_path_param_names(path_key) {
            parameters.push(json!({
                "name": name,
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": format!("Zest variable {name} — supplied by the environment at runtime."),
            }));
        }

        // Query parameters: request params first, then any pairs left in the
        // URL query string (deduped by name+value, insertion order kept).
        let mut seen = HashSet::new();
        for (name, value, description) in req
            .params
            .iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .map(|p| (p.key.as_str(), p.value.as_str(), p.description.as_deref()))
            .chain(
                url_query
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str(), None)),
            )
        {
            if !seen.insert((name.to_string(), value.to_string())) {
                continue;
            }
            parameters.push(param_value(name, "query", description, Some(value)));
        }

        // Header parameters; Content-Type is expressed via requestBody content.
        let mut headers: Vec<(String, String, Option<&str>)> = req
            .headers
            .iter()
            .filter(|h| {
                h.enabled && !h.key.is_empty() && !h.key.eq_ignore_ascii_case("content-type")
            })
            .map(|h| (h.key.clone(), h.value.clone(), h.description.as_deref()))
            .collect();
        if req.protocol == Protocol::Sse
            && !headers
                .iter()
                .any(|(k, _, _)| k.eq_ignore_ascii_case("accept"))
        {
            headers.push(("Accept".into(), "text/event-stream".into(), None));
        }
        for (name, value, description) in headers {
            parameters.push(param_value(&name, "header", description, Some(&value)));
        }

        parameters
    }

    /// Maps a resolved auth to a per-operation `security` array, registering
    /// the matching scheme in `components.securitySchemes`. Secrets are never
    /// emitted.
    fn security_for(&mut self, auth: &Auth) -> Option<Value> {
        match auth {
            Auth::None | Auth::Inherit => None,
            Auth::Bearer { .. } => {
                self.security_schemes
                    .entry("bearerAuth")
                    .or_insert_with(|| json!({ "type": "http", "scheme": "bearer" }));
                Some(json!([{ "bearerAuth": [] }]))
            }
            Auth::Basic { .. } => {
                self.security_schemes
                    .entry("basicAuth")
                    .or_insert_with(|| json!({ "type": "http", "scheme": "basic" }));
                Some(json!([{ "basicAuth": [] }]))
            }
            Auth::ApiKey { key, add_to, .. } => {
                let scheme_id = format!("apiKey-{}-{}", add_to.as_str(), slugify(key));
                let location = if *add_to == ApiKeyLocation::Query {
                    "query"
                } else {
                    "header"
                };
                self.security_schemes
                    .entry(scheme_id.clone())
                    .or_insert_with(|| json!({ "type": "apiKey", "in": location, "name": key }));
                let mut requirement = Map::new();
                requirement.insert(scheme_id, json!([]));
                Some(Value::Array(vec![Value::Object(requirement)]))
            }
        }
    }
}

/// Builds one parameter object; `location` is `"path"`, `"query"` or `"header"`.
fn param_value(
    name: &str,
    location: &str,
    description: Option<&str>,
    example: Option<&str>,
) -> Value {
    let mut p = json!({
        "name": name,
        "in": location,
        "required": location == "path",
        "schema": { "type": "string" },
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
        p.insert("description".into(), json!(d));
    }
    if let Some(v) = example.filter(|v| !v.is_empty()) {
        p.insert("example".into(), json!(v));
    }
    Value::Object(p)
}

/// Splits a request URL into `(server, path, query pairs)`. The split is done
/// manually instead of via `url::Url` because the WHATWG parser percent-encodes
/// path braces, which would destroy `{{var}}` templates. Unparseable URLs
/// (e.g. `{{base_url}}/users`) yield no server and use the raw URL as the path.
fn split_request_url(url: &str) -> (Option<String>, String, Vec<(String, String)>) {
    let raw = url.trim();
    if raw.is_empty() {
        return (None, "/".into(), Vec::new());
    }
    // Drop the fragment: API request URLs never target one.
    let no_fragment = raw.split('#').next().unwrap_or(raw);

    if let Some((scheme, after)) = no_fragment.split_once("://") {
        let valid_scheme = !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        let authority_end = after.find(['/', '?']).unwrap_or(after.len());
        if valid_scheme && authority_end > 0 {
            let authority = &after[..authority_end];
            let rest = &after[authority_end..];
            let (path, query) = match rest.split_once('?') {
                Some((p, q)) => (if p.is_empty() { "/" } else { p }, Some(q)),
                None => (if rest.is_empty() { "/" } else { rest }, None),
            };
            let pairs = query.map(parse_query_pairs).unwrap_or_default();
            return (
                Some(format!("{}://{}", scheme, authority)),
                path.to_string(),
                pairs,
            );
        }
    }

    // Fallback: keep the raw URL as the path key and split off its query part.
    let (path, query) = match no_fragment.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (no_fragment, None),
    };
    let pairs = query.map(parse_query_pairs).unwrap_or_default();
    (None, path.to_string(), pairs)
}

/// Parses `a=1&b=2` query pairs (form-urlencoded semantics, `+` → space).
fn parse_query_pairs(query: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(query.as_bytes())
        .into_iter()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// Converts Zest `{{var}}` template tokens to OpenAPI `{var}` path tokens.
fn var_braces_to_openapi(path: &str) -> String {
    path.replace("{{", "{").replace("}}", "}")
}

/// Extracts `{name}` path tokens (deduped, first-occurrence order).
fn extract_path_param_names(path: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut rest = path;
    while let (Some(open), Some(close)) = (rest.find('{'), rest.find('}')) {
        if close < open {
            rest = &rest[close + 1..];
            continue;
        }
        let name = rest[open + 1..close].trim().to_string();
        rest = &rest[close + 1..];
        if !name.is_empty() && seen.insert(name.clone()) {
            names.push(name);
        }
    }
    names
}

/// Builds the `requestBody` object for one request, or `None` for bodyless ones.
///
/// GraphQL requests arrive already flattened to their HTTP transport shape
/// (`POST` + JSON body) by the caller, mirroring `RequestItem::to_graphql_http_item`.
fn request_body_for(req: &ImportedRequest) -> Option<Value> {
    match &req.body {
        RequestBody::None => None,
        RequestBody::Json { content } => Some(json_request_body(json_media(content))),
        RequestBody::FormUrlEncoded { params } => {
            let mut properties = Map::new();
            for p in params.iter().filter(|p| p.enabled && !p.key.is_empty()) {
                properties.insert(
                    p.key.clone(),
                    json!({ "type": "string", "example": p.value }),
                );
            }
            Some(body_value(
                "application/x-www-form-urlencoded",
                json!({ "schema": { "type": "object", "properties": Value::Object(properties) } }),
            ))
        }
        RequestBody::Multipart { items } => {
            let mut properties = Map::new();
            for item in items.iter().filter(|i| i.enabled && !i.key.is_empty()) {
                let prop = if item.is_file {
                    json!({ "type": "string", "format": "binary" })
                } else {
                    json!({ "type": "string", "example": item.value })
                };
                properties.insert(item.key.clone(), prop);
            }
            Some(body_value(
                "multipart/form-data",
                json!({ "schema": { "type": "object", "properties": Value::Object(properties) } }),
            ))
        }
        RequestBody::Raw {
            content,
            content_type,
        } => Some(body_value(
            &normalize_mime(content_type),
            json!({ "schema": { "type": "string" }, "example": content }),
        )),
        RequestBody::Binary { .. } => Some(body_value(
            "application/octet-stream",
            json!({ "schema": { "type": "string", "format": "binary" } }),
        )),
    }
}

fn json_request_body(media: Value) -> Value {
    body_value("application/json", media)
}

fn body_value(mime: &str, media: Value) -> Value {
    json!({ "required": true, "content": { mime: media } })
}

/// Media object for JSON content: parsed body as the example, with a variable
/// token substitution fallback for bodies containing `{{var}}` placeholders.
fn json_media(content: &str) -> Value {
    let example = json_example_value(content);
    let schema_type = if example.is_object() {
        "object"
    } else if example.is_array() {
        "array"
    } else {
        "string"
    };
    json!({ "schema": { "type": schema_type }, "example": example })
}

/// Parses the JSON body for use as an example; `{{var}}` tokens are replaced
/// with placeholder strings so partially-templated bodies still produce a
/// valid example. Raw text is the final fallback.
fn json_example_value(content: &str) -> Value {
    if let Ok(v) = serde_json::from_str::<Value>(content) {
        return v;
    }
    let substituted = substitute_var_tokens(content);
    if let Ok(v) = serde_json::from_str::<Value>(&substituted) {
        return v;
    }
    Value::String(content.to_string())
}

/// Replaces `{{token}}` occurrences so the substituted text stays valid JSON:
/// quoted tokens (between string delimiters) keep their quotes, bare tokens
/// gain quotes. `$`-prefixed faker macros use the bare name.
fn substitute_var_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = after[..end].trim().trim_start_matches('$').trim();
                let quoted = out.ends_with('"') && after[end + 2..].starts_with('"');
                if quoted {
                    out.push_str(name);
                } else {
                    out.push('"');
                    out.push_str(name);
                    out.push('"');
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Lowercases and trims a content type, falling back to `text/plain`.
fn normalize_mime(content_type: &str) -> String {
    let trimmed = content_type.trim();
    if trimmed.is_empty() {
        "text/plain".into()
    } else {
        trimmed.to_lowercase()
    }
}

/// Builds the `responses` object: saved examples grouped by status, or a
/// baseline `"200"` placeholder when none exist.
fn responses_for(req: &ImportedRequest) -> Value {
    let mut responses: HashMap<String, Value> = HashMap::new();
    for ex in &req.examples {
        let status = if (100..=599).contains(&ex.status) {
            ex.status.to_string()
        } else {
            "200".to_string()
        };
        let mime = example_mime(ex);
        let example = match ex.body.as_deref() {
            Some(b) if b.trim().is_empty() => Value::String(String::new()),
            Some(b) => json_example_value(b),
            None => Value::String(String::new()),
        };
        let response = json!({
            "description": ex.name,
            "content": { mime: { "example": example } },
        });
        // First example per status wins; later ones refine the description only
        // when no entry exists yet.
        responses.entry(status).or_insert(response);
    }
    if responses.is_empty() {
        responses.insert(
            "200".into(),
            json!({ "description": "Successful response" }),
        );
    }
    let mut map = Map::new();
    // Sort statuses for deterministic output.
    let mut keys: Vec<&String> = responses.keys().collect();
    keys.sort();
    for key in keys {
        map.insert(key.clone(), responses[key].clone());
    }
    Value::Object(map)
}

/// Picks the response example MIME type: the saved Content-Type header, else
/// a JSON sniff, else `text/plain`.
fn example_mime(ex: &crate::storage::models::RequestExampleFile) -> String {
    if let Some(ct) = ex
        .headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return normalize_mime(&ct);
    }
    if let Some(body) = ex.body.as_deref() {
        if serde_json::from_str::<Value>(body).is_ok() {
            return "application/json".into();
        }
        if let Some(guess) = mime_guess::from_path(&ex.name).first_raw() {
            return guess.to_string();
        }
    }
    "text/plain".into()
}

#[cfg(test)]
mod tests;
