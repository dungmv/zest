use super::models::{ImportFormat, ImportResult, ImportedEnvironment, ImportedRequest};
use super::{push_folder_chain, split_folder_segments};
use crate::models::{
    ApiKeyLocation, Auth, EnvironmentVariable, HttpMethod, KeyValuePair, MultipartItem, RequestBody,
};
use crate::storage::models::{RequestExampleFile, RequestKvItem};
use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::str::FromStr;

pub fn parse_openapi(content: &str) -> Result<ImportResult> {
    // Attempt parsing as JSON first; if it fails, parse as YAML into serde_json::Value
    let root: Value = if let Ok(val) = serde_json::from_str::<Value>(content) {
        val
    } else {
        serde_yaml::from_str::<Value>(content)
            .map_err(|e| anyhow!("Failed to parse content as JSON or YAML: {}", e))?
    };

    let openapi_ver = root
        .get("openapi")
        .and_then(|v| v.as_str())
        .or_else(|| root.get("swagger").and_then(|v| v.as_str()));

    if openapi_ver.is_none() {
        return Err(anyhow!(
            "Missing 'openapi' or 'swagger' field in document. Expected OpenAPI 3.x or 2.x"
        ));
    }

    let title = root
        .pointer("/info/title")
        .and_then(|v| v.as_str())
        .unwrap_or("OpenAPI Spec")
        .to_string();

    let description = root
        .pointer("/info/description")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Extract servers / base URL
    let mut environments = Vec::new();
    let mut base_url_var = String::new();

    if let Some(servers) = root.get("servers").and_then(|s| s.as_array()) {
        if let Some(first_server) = servers.first() {
            if let Some(url_str) = first_server.get("url").and_then(|u| u.as_str()) {
                let clean_url = url_str.trim().trim_end_matches('/');
                if !clean_url.is_empty() {
                    base_url_var = clean_url.to_string();
                }
            }
        }
    } else if let Some(host) = root.get("host").and_then(|h| h.as_str()) {
        // Swagger 2.0 fallback
        let scheme = root
            .get("schemes")
            .and_then(|s| s.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.as_str())
            .unwrap_or("https");
        let base_path = root.get("basePath").and_then(|b| b.as_str()).unwrap_or("");
        base_url_var = format!("{}://{}{}", scheme, host, base_path.trim_end_matches('/'));
    }

    let url_prefix = if !base_url_var.is_empty() {
        environments.push(ImportedEnvironment {
            name: format!("{} (Env)", title),
            variables: vec![EnvironmentVariable {
                key: "baseUrl".to_string(),
                initial_value: base_url_var.clone(),
                current_value: base_url_var.clone(),
                secret: false,
            }],
        });
        "{{baseUrl}}"
    } else {
        "http://localhost:8080"
    };

    let mut imported_requests = Vec::new();
    let mut seen_folder_paths: HashSet<Vec<String>> = HashSet::new();
    let mut imported_folders = Vec::new();
    let mut warnings = Vec::new();

    let empty_map = Map::new();
    let paths = root
        .get("paths")
        .and_then(|p| p.as_object())
        .unwrap_or(&empty_map);

    for (path_str, path_item) in paths {
        let path_obj = match path_item.as_object() {
            Some(o) => o,
            None => continue,
        };

        // Common parameters for this path
        let path_common_params = path_obj
            .get("parameters")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();

        for (method_str, op_val) in path_obj {
            let method_upper = method_str.to_uppercase();
            let http_method = match HttpMethod::from_str(&method_upper) {
                Ok(m) => m,
                Err(_) => continue, // skip non-HTTP method fields like parameters, summary, etc.
            };

            let op_obj = match op_val.as_object() {
                Some(o) => o,
                None => continue,
            };

            // Request Name: summary -> operationId -> "METHOD path"
            let req_name = op_obj
                .get("summary")
                .and_then(|s| s.as_str())
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    op_obj
                        .get("operationId")
                        .and_then(|s| s.as_str())
                        .filter(|s| !s.trim().is_empty())
                })
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{} {}", http_method.as_str(), path_str));

            let req_desc = op_obj
                .get("description")
                .and_then(|s| s.as_str())
                .map(String::from);

            // Folder from tags
            let mut folder_path = Vec::new();
            if let Some(tags) = op_obj.get("tags").and_then(|t| t.as_array()) {
                if let Some(first_tag) = tags.first().and_then(|t| t.as_str()) {
                    let segments = split_folder_segments(first_tag);
                    if !segments.is_empty() {
                        folder_path = push_folder_chain(
                            &[],
                            &segments,
                            None,
                            &mut imported_folders,
                            &mut seen_folder_paths,
                        );
                    }
                }
            }

            // Convert URL path {param} to {{param}}
            let formatted_path = format_path_params(path_str);
            let full_url = if formatted_path.starts_with('/') {
                format!("{}{}", url_prefix, formatted_path)
            } else {
                format!("{}/{}", url_prefix, formatted_path)
            };

            // Merge path parameters and operation parameters
            let mut all_params = path_common_params.clone();
            if let Some(op_params) = op_obj.get("parameters").and_then(|p| p.as_array()) {
                all_params.extend(op_params.iter().cloned());
            }

            let mut query_params = Vec::new();
            let mut header_params = Vec::new();

            for param in all_params {
                let resolved_param = resolve_ref(&root, &param);
                let p_in = resolved_param
                    .get("in")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let p_name = resolved_param
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if p_name.is_empty() {
                    continue;
                }

                let p_desc = resolved_param
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let required = resolved_param
                    .get("required")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Sample value
                let sample_val = extract_param_sample(&root, resolved_param);

                let kv = KeyValuePair {
                    enabled: required,
                    key: p_name,
                    value: sample_val,
                    description: p_desc,
                };

                match p_in {
                    "query" => query_params.push(kv),
                    "header" => header_params.push(kv),
                    _ => {}
                }
            }

            // Request Body
            let (body, content_type_header) = extract_request_body(&root, op_obj);
            if let Some(ct) = content_type_header {
                if !header_params
                    .iter()
                    .any(|h| h.key.eq_ignore_ascii_case("content-type"))
                {
                    header_params.push(KeyValuePair::new("Content-Type", ct));
                }
            }

            // Auth resolution
            let auth = extract_auth(&root, op_obj);

            // Extract Response Examples
            let examples = extract_response_examples(&root, op_obj);

            imported_requests.push(ImportedRequest {
                name: req_name,
                method: http_method,
                url: full_url,
                params: query_params,
                headers: header_params,
                auth,
                body,
                script: None,
                examples,
                folder_path,
                description: req_desc,
            });
        }
    }

    if imported_requests.is_empty() {
        warnings
            .push("No HTTP request operations found in the provided OpenAPI document.".to_string());
    }

    Ok(ImportResult {
        title,
        description,
        format: ImportFormat::OpenApi3,
        folders: imported_folders,
        requests: imported_requests,
        environments,
        warnings,
    })
}

/// Convert `/users/{userId}/posts/{postId}` to `/users/{{userId}}/posts/{{postId}}`
fn format_path_params(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 8);
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut param_name = String::new();
            let mut closed = false;
            while let Some(&next_c) = chars.peek() {
                chars.next();
                if next_c == '}' {
                    closed = true;
                    break;
                }
                param_name.push(next_c);
            }
            if closed && !param_name.is_empty() && !param_name.starts_with('{') {
                out.push_str("{{");
                out.push_str(&param_name);
                out.push_str("}}");
            } else {
                out.push('{');
                out.push_str(&param_name);
                if closed {
                    out.push('}');
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn resolve_ref<'a>(root: &'a Value, val: &'a Value) -> &'a Value {
    let mut current = val;
    let mut visited = std::collections::HashSet::new();
    let mut depth = 0;

    while depth < 32 {
        if let Some(ref_str) = current.get("$ref").and_then(|r| r.as_str()) {
            if !visited.insert(ref_str) {
                // Cycle detected, abort resolution and return current value
                break;
            }
            if let Some(pointer_path) = ref_str.strip_prefix('#') {
                if let Some(target) = root.pointer(pointer_path) {
                    current = target;
                    depth += 1;
                    continue;
                }
            }
        }
        break;
    }
    current
}

fn extract_param_sample(root: &Value, param: &Value) -> String {
    if let Some(ex) = param.get("example") {
        return value_to_string(ex);
    }
    if let Some(schema) = param.get("schema") {
        let res_schema = resolve_ref(root, schema);
        if let Some(default_val) = res_schema.get("default") {
            return value_to_string(default_val);
        }
        if let Some(ex) = res_schema.get("example") {
            return value_to_string(ex);
        }
        if let Some(type_str) = res_schema.get("type").and_then(|t| t.as_str()) {
            match type_str {
                "integer" | "number" => return "0".to_string(),
                "boolean" => return "true".to_string(),
                "string" => {
                    if let Some(fmt) = res_schema.get("format").and_then(|f| f.as_str()) {
                        match fmt {
                            "date" => return "2026-01-01".to_string(),
                            "date-time" => return "2026-01-01T00:00:00Z".to_string(),
                            "uuid" => return "123e4567-e89b-12d3-a456-426614174000".to_string(),
                            "email" => return "user@example.com".to_string(),
                            _ => {}
                        }
                    }
                    return "value".to_string();
                }
                _ => {}
            }
        }
    }
    String::new()
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => v.to_string(),
    }
}

fn extract_request_body(
    root: &Value,
    op_obj: &Map<String, Value>,
) -> (RequestBody, Option<String>) {
    // OpenAPI 3.x requestBody
    if let Some(rb) = op_obj.get("requestBody") {
        let rb_resolved = resolve_ref(root, rb);
        if let Some(content) = rb_resolved.get("content").and_then(|c| c.as_object()) {
            // Check application/json
            if let Some(json_media) = content.get("application/json") {
                let sample_json = generate_media_sample(root, json_media);
                return (
                    RequestBody::Json {
                        content: sample_json,
                    },
                    Some("application/json".to_string()),
                );
            }

            // Check form urlencoded
            if let Some(form_media) = content.get("application/x-www-form-urlencoded") {
                let mut form_items = Vec::new();
                if let Some(schema) = form_media.get("schema") {
                    let res_schema = resolve_ref(root, schema);
                    if let Some(props) = res_schema.get("properties").and_then(|p| p.as_object()) {
                        for (k, v) in props {
                            let sample = generate_schema_sample(root, v, 0);
                            form_items.push(KeyValuePair::new(k, value_to_string(&sample)));
                        }
                    }
                }
                return (
                    RequestBody::FormUrlEncoded { params: form_items },
                    Some("application/x-www-form-urlencoded".to_string()),
                );
            }

            // Check multipart/form-data
            if let Some(multipart_media) = content.get("multipart/form-data") {
                let mut multi_items = Vec::new();
                if let Some(schema) = multipart_media.get("schema") {
                    let res_schema = resolve_ref(root, schema);
                    if let Some(props) = res_schema.get("properties").and_then(|p| p.as_object()) {
                        for (k, v) in props {
                            let sample = generate_schema_sample(root, v, 0);
                            multi_items.push(MultipartItem::new_text(k, value_to_string(&sample)));
                        }
                    }
                }
                return (
                    RequestBody::Multipart { items: multi_items },
                    Some("multipart/form-data".to_string()),
                );
            }

            // Check raw / text
            if let Some((media_type, media_obj)) = content.iter().next() {
                let sample = generate_media_sample(root, media_obj);
                return (
                    RequestBody::Raw {
                        content: sample,
                        content_type: media_type.clone(),
                    },
                    Some(media_type.clone()),
                );
            }
        }
    }

    // Swagger 2.0 parameters with in: body
    if let Some(params) = op_obj.get("parameters").and_then(|p| p.as_array()) {
        for param in params {
            let res_param = resolve_ref(root, param);
            if res_param.get("in").and_then(|i| i.as_str()) == Some("body") {
                if let Some(schema) = res_param.get("schema") {
                    let sample = generate_schema_sample(root, schema, 0);
                    let pretty = serde_json::to_string_pretty(&sample).unwrap_or_default();
                    return (
                        RequestBody::Json { content: pretty },
                        Some("application/json".to_string()),
                    );
                }
            }
        }
    }

    (RequestBody::None, None)
}

fn generate_media_sample(root: &Value, media_obj: &Value) -> String {
    if let Some(ex) = media_obj.get("example") {
        if ex.is_string() {
            return ex.as_str().unwrap().to_string();
        }
        return serde_json::to_string_pretty(ex).unwrap_or_default();
    }
    if let Some(examples) = media_obj.get("examples").and_then(|e| e.as_object()) {
        if let Some((_, first_ex)) = examples.iter().next() {
            let res_ex = resolve_ref(root, first_ex);
            if let Some(val) = res_ex.get("value") {
                if val.is_string() {
                    return val.as_str().unwrap().to_string();
                }
                return serde_json::to_string_pretty(val).unwrap_or_default();
            }
        }
    }
    if let Some(schema) = media_obj.get("schema") {
        let sample = generate_schema_sample(root, schema, 0);
        return serde_json::to_string_pretty(&sample).unwrap_or_default();
    }
    String::new()
}

fn generate_schema_sample(root: &Value, schema: &Value, depth: usize) -> Value {
    if depth > 4 {
        return json!({});
    }
    let res_schema = resolve_ref(root, schema);

    if let Some(ex) = res_schema.get("example") {
        return ex.clone();
    }
    if let Some(default_val) = res_schema.get("default") {
        return default_val.clone();
    }

    let schema_type = res_schema.get("type").and_then(|t| t.as_str());
    match schema_type {
        Some("object") | None if res_schema.get("properties").is_some() => {
            let mut obj = Map::new();
            if let Some(props) = res_schema.get("properties").and_then(|p| p.as_object()) {
                for (k, v) in props {
                    obj.insert(k.clone(), generate_schema_sample(root, v, depth + 1));
                }
            }
            Value::Object(obj)
        }
        Some("array") => {
            if let Some(items) = res_schema.get("items") {
                let sample_item = generate_schema_sample(root, items, depth + 1);
                Value::Array(vec![sample_item])
            } else {
                Value::Array(Vec::new())
            }
        }
        Some("string") => {
            if let Some(format) = res_schema.get("format").and_then(|f| f.as_str()) {
                match format {
                    "date-time" => json!("2026-01-01T00:00:00Z"),
                    "date" => json!("2026-01-01"),
                    "email" => json!("user@example.com"),
                    "uuid" => json!("123e4567-e89b-12d3-a456-426614174000"),
                    _ => json!("string"),
                }
            } else {
                json!("string")
            }
        }
        Some("integer") => json!(0),
        Some("number") => json!(0.0),
        Some("boolean") => json!(true),
        _ => json!(null),
    }
}

fn extract_auth(root: &Value, op_obj: &Map<String, Value>) -> Auth {
    let sec_reqs = op_obj
        .get("security")
        .and_then(|s| s.as_array())
        .or_else(|| root.get("security").and_then(|s| s.as_array()));

    let sec_schemes = root
        .pointer("/components/securitySchemes")
        .and_then(|s| s.as_object())
        .or_else(|| root.get("securityDefinitions").and_then(|s| s.as_object()));

    if let (Some(reqs), Some(schemes)) = (sec_reqs, sec_schemes) {
        for req in reqs {
            if let Some(req_map) = req.as_object() {
                for (scheme_name, _) in req_map {
                    if let Some(scheme_val) = schemes.get(scheme_name) {
                        let res_scheme = resolve_ref(root, scheme_val);
                        let scheme_type = res_scheme
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        match scheme_type {
                            "http" => {
                                let sub_scheme = res_scheme
                                    .get("scheme")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("")
                                    .to_lowercase();
                                if sub_scheme == "bearer" {
                                    return Auth::Bearer {
                                        token: "{{bearerToken}}".to_string(),
                                    };
                                } else if sub_scheme == "basic" {
                                    return Auth::Basic {
                                        username: "{{username}}".to_string(),
                                        password: "{{password}}".to_string(),
                                    };
                                }
                            }
                            "basic" => {
                                // Swagger 2.0 basic
                                return Auth::Basic {
                                    username: "{{username}}".to_string(),
                                    password: "{{password}}".to_string(),
                                };
                            }
                            "apiKey" => {
                                let key_name = res_scheme
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("X-API-Key")
                                    .to_string();
                                let key_in = res_scheme
                                    .get("in")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("header");
                                return Auth::ApiKey {
                                    key: key_name,
                                    value: "{{apiKey}}".to_string(),
                                    add_to: if key_in.eq_ignore_ascii_case("query") {
                                        ApiKeyLocation::Query
                                    } else {
                                        ApiKeyLocation::Header
                                    },
                                };
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Auth::None
}

fn extract_response_examples(root: &Value, op_obj: &Map<String, Value>) -> Vec<RequestExampleFile> {
    let mut examples = Vec::new();
    let responses = match op_obj.get("responses").and_then(|r| r.as_object()) {
        Some(r) => r,
        None => return examples,
    };

    for (status_str, resp_val) in responses {
        let res_resp = resolve_ref(root, resp_val);
        let status_code = status_str.parse::<u16>().unwrap_or(200);
        let resp_desc = res_resp
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("Response")
            .to_string();

        let mut sample_body = None;
        let mut sample_headers = Vec::new();

        // OpenAPI 3.x content
        if let Some(content) = res_resp.get("content").and_then(|c| c.as_object()) {
            if let Some(json_media) = content.get("application/json") {
                let s = generate_media_sample(root, json_media);
                if !s.is_empty() {
                    sample_body = Some(s);
                    sample_headers.push(RequestKvItem::new("Content-Type", "application/json"));
                }
            } else if let Some((media_type, media_obj)) = content.iter().next() {
                let s = generate_media_sample(root, media_obj);
                if !s.is_empty() {
                    sample_body = Some(s);
                    sample_headers.push(RequestKvItem::new("Content-Type", media_type.clone()));
                }
            }
        }

        // Swagger 2.0 schema or examples
        if sample_body.is_none() {
            if let Some(schema) = res_resp.get("schema") {
                let sample = generate_schema_sample(root, schema, 0);
                sample_body = Some(serde_json::to_string_pretty(&sample).unwrap_or_default());
                sample_headers.push(RequestKvItem::new("Content-Type", "application/json"));
            }
        }

        if sample_body.is_some() || status_code != 200 {
            examples.push(RequestExampleFile {
                name: format!("{} - {}", status_str, resp_desc),
                status: status_code,
                headers: sample_headers,
                body: sample_body,
            });
        }
    }

    examples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_circular_ref_safety() {
        let json_spec = r##"{
            "openapi": "3.0.0",
            "info": { "title": "Circular API", "version": "1.0.0" },
            "paths": {
                "/test": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/NodeA" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "NodeA": {
                        "type": "object",
                        "properties": {
                            "next": { "$ref": "#/components/schemas/NodeB" }
                        }
                    },
                    "NodeB": {
                        "type": "object",
                        "properties": {
                            "next": { "$ref": "#/components/schemas/NodeA" }
                        }
                    }
                }
            }
        }"##;

        let res = parse_openapi(json_spec).unwrap();
        assert_eq!(res.requests.len(), 1);
    }
}
