use super::models::{
    ImportFormat, ImportResult, ImportedEnvironment, ImportedFolder, ImportedRequest,
};
use super::{push_folder_chain, split_folder_segments};
use crate::models::{
    ApiKeyLocation, Auth, EnvironmentVariable, HttpMethod, KeyValuePair, MultipartItem,
    RequestBody, ScriptConfig,
};
use crate::storage::models::{RequestExampleFile, RequestKvItem};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::str::FromStr;

pub fn parse_postman(content: &str) -> Result<ImportResult> {
    let root: Value = serde_json::from_str::<Value>(content)
        .map_err(|e| anyhow!("Failed to parse Postman collection as JSON: {}", e))?;

    let title = root
        .pointer("/info/name")
        .and_then(|v| v.as_str())
        .unwrap_or("Postman Collection")
        .to_string();

    let description = root
        .pointer("/info/description")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Extract collection variables into an Environment
    let mut environments = Vec::new();
    if let Some(vars) = root.get("variable").and_then(|v| v.as_array()) {
        let mut env_vars = Vec::new();
        for var in vars {
            if let Some(key) = var.get("key").and_then(|k| k.as_str()) {
                let val = var
                    .get("value")
                    .and_then(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        Value::Bool(b) => Some(b.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default();

                env_vars.push(EnvironmentVariable {
                    key: key.to_string(),
                    initial_value: val.clone(),
                    current_value: val,
                    secret: false,
                });
            }
        }

        if !env_vars.is_empty() {
            environments.push(ImportedEnvironment {
                name: format!("{} (Env)", title),
                variables: env_vars,
            });
        }
    }

    let default_auth = root.get("auth");

    let mut imported_folders = Vec::new();
    let mut seen_folder_paths: HashSet<Vec<String>> = HashSet::new();
    let mut imported_requests = Vec::new();
    let mut warnings = Vec::new();

    if let Some(items) = root.get("item").and_then(|i| i.as_array()) {
        parse_items(
            items,
            &[],
            default_auth,
            &mut imported_folders,
            &mut seen_folder_paths,
            &mut imported_requests,
            &mut warnings,
        );
    } else {
        warnings.push("No 'item' array found in Postman collection.".to_string());
    }

    Ok(ImportResult {
        title,
        description,
        format: ImportFormat::PostmanCollection,
        folders: imported_folders,
        requests: imported_requests,
        environments,
        warnings,
    })
}

fn parse_items(
    items: &[Value],
    parent_path: &[String],
    inherited_auth: Option<&Value>,
    folders: &mut Vec<ImportedFolder>,
    seen_folder_paths: &mut HashSet<Vec<String>>,
    requests: &mut Vec<ImportedRequest>,
    warnings: &mut Vec<String>,
) {
    for item in items {
        let name = item
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Untitled")
            .to_string();

        let item_auth = item.get("auth").or(inherited_auth);

        // Check if item is a folder (contains "item" array)
        if let Some(sub_items) = item.get("item").and_then(|i| i.as_array()) {
            let desc = item
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from);

            let segments = split_folder_segments(&name);
            let current_path = if segments.is_empty() {
                push_folder_chain(
                    parent_path,
                    std::slice::from_ref(&name),
                    desc,
                    folders,
                    seen_folder_paths,
                )
            } else {
                push_folder_chain(parent_path, &segments, desc, folders, seen_folder_paths)
            };

            parse_items(
                sub_items,
                &current_path,
                item_auth,
                folders,
                seen_folder_paths,
                requests,
                warnings,
            );
            continue;
        }

        // Check if item is a request
        if let Some(req_val) = item.get("request") {
            match parse_single_request(&name, req_val, item, parent_path, item_auth) {
                Ok(req) => requests.push(req),
                Err(err) => warnings.push(format!("Failed to parse request '{}': {}", name, err)),
            }
        }
    }
}

fn parse_single_request(
    name: &str,
    req_val: &Value,
    item_root: &Value,
    folder_path: &[String],
    inherited_auth: Option<&Value>,
) -> Result<ImportedRequest> {
    // If request is a raw string (Postman shorthand for GET URL)
    if let Some(url_str) = req_val.as_str() {
        return Ok(ImportedRequest {
            name: name.to_string(),
            method: HttpMethod::Get,
            url: url_str.to_string(),
            params: Vec::new(),
            headers: Vec::new(),
            auth: Auth::None,
            body: RequestBody::None,
            script: None,
            examples: Vec::new(),
            folder_path: folder_path.to_vec(),
            description: None,
        });
    }

    let req_obj = req_val
        .as_object()
        .ok_or_else(|| anyhow!("Request is not an object or string"))?;

    let method_str = req_obj
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("GET");
    let method = HttpMethod::from_str(method_str).unwrap_or(HttpMethod::Get);

    // URL & Query Params
    let (url, query_params) = extract_url_and_params(req_obj.get("url"));

    // Headers
    let mut headers = Vec::new();
    if let Some(hdr_arr) = req_obj.get("header").and_then(|h| h.as_array()) {
        for h in hdr_arr {
            if let Some(key) = h.get("key").and_then(|k| k.as_str()) {
                let val = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                let enabled = !h.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false);
                let desc = h
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from);
                headers.push(KeyValuePair {
                    enabled,
                    key: key.to_string(),
                    value: val.to_string(),
                    description: desc,
                });
            }
        }
    }

    // Auth
    let effective_auth_val = req_obj.get("auth").or(inherited_auth);
    let auth = extract_postman_auth(effective_auth_val);

    // Body
    let body = extract_postman_body(req_obj.get("body"));

    // Scripts (pre_request, test)
    let script = extract_postman_scripts(item_root.get("event"));

    // Response Examples
    let mut examples = Vec::new();
    if let Some(responses) = item_root.get("response").and_then(|r| r.as_array()) {
        for resp in responses {
            let resp_name = resp
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("Example")
                .to_string();
            let status = resp.get("code").and_then(|c| c.as_u64()).unwrap_or(200) as u16;
            let resp_body = resp.get("body").and_then(|b| b.as_str()).map(String::from);

            let mut resp_headers = Vec::new();
            if let Some(resp_hdr_arr) = resp.get("header").and_then(|h| h.as_array()) {
                for rh in resp_hdr_arr {
                    if let Some(k) = rh.get("key").and_then(|k| k.as_str()) {
                        let v = rh.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        resp_headers.push(RequestKvItem::new(k, v));
                    }
                }
            }

            examples.push(RequestExampleFile {
                name: resp_name,
                status,
                headers: resp_headers,
                body: resp_body,
            });
        }
    }

    let description = req_obj
        .get("description")
        .and_then(|d| d.as_str())
        .map(String::from);

    Ok(ImportedRequest {
        name: name.to_string(),
        method,
        url,
        params: query_params,
        headers,
        auth,
        body,
        script,
        examples,
        folder_path: folder_path.to_vec(),
        description,
    })
}

fn extract_url_and_params(url_val: Option<&Value>) -> (String, Vec<KeyValuePair>) {
    let mut query_params = Vec::new();
    let Some(val) = url_val else {
        return (String::new(), query_params);
    };

    if let Some(s) = val.as_str() {
        return (s.to_string(), query_params);
    }

    let raw_url = val
        .get("raw")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(query_arr) = val.get("query").and_then(|q| q.as_array()) {
        for q in query_arr {
            if let Some(k) = q.get("key").and_then(|k| k.as_str()) {
                let v = q.get("value").and_then(|v| v.as_str()).unwrap_or("");
                let enabled = !q.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false);
                let desc = q
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from);
                query_params.push(KeyValuePair {
                    enabled,
                    key: k.to_string(),
                    value: v.to_string(),
                    description: desc,
                });
            }
        }
    }

    // Strip existing query string from raw URL if query params array is populated
    let clean_url = if !query_params.is_empty() {
        if let Some((base, _)) = raw_url.split_once('?') {
            base.to_string()
        } else {
            raw_url
        }
    } else {
        raw_url
    };

    (clean_url, query_params)
}

fn extract_postman_auth(auth_val: Option<&Value>) -> Auth {
    let Some(val) = auth_val else {
        return Auth::None;
    };
    let auth_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("noauth");

    match auth_type {
        "bearer" => {
            let token = extract_auth_kv(val.get("bearer"), "token").unwrap_or_default();
            Auth::Bearer { token }
        }
        "basic" => {
            let username = extract_auth_kv(val.get("basic"), "username").unwrap_or_default();
            let password = extract_auth_kv(val.get("basic"), "password").unwrap_or_default();
            Auth::Basic { username, password }
        }
        "apikey" => {
            let key = extract_auth_kv(val.get("apikey"), "key")
                .unwrap_or_else(|| "X-API-Key".to_string());
            let value = extract_auth_kv(val.get("apikey"), "value").unwrap_or_default();
            let where_in =
                extract_auth_kv(val.get("apikey"), "in").unwrap_or_else(|| "header".to_string());
            Auth::ApiKey {
                key,
                value,
                add_to: if where_in.eq_ignore_ascii_case("query") {
                    ApiKeyLocation::Query
                } else {
                    ApiKeyLocation::Header
                },
            }
        }
        _ => Auth::None,
    }
}

fn extract_auth_kv(arr_val: Option<&Value>, key_name: &str) -> Option<String> {
    let arr = arr_val?.as_array()?;
    for item in arr {
        if item.get("key").and_then(|k| k.as_str()) == Some(key_name) {
            return item.get("value").and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            });
        }
    }
    None
}

fn extract_postman_body(body_val: Option<&Value>) -> RequestBody {
    let Some(b) = body_val else {
        return RequestBody::None;
    };
    let mode = b.get("mode").and_then(|m| m.as_str()).unwrap_or("");

    match mode {
        "raw" => {
            let raw_text = b.get("raw").and_then(|r| r.as_str()).unwrap_or("");
            let lang = b
                .pointer("/options/raw/language")
                .and_then(|l| l.as_str())
                .unwrap_or("");
            if lang.eq_ignore_ascii_case("json")
                || raw_text.trim_start().starts_with('{')
                || raw_text.trim_start().starts_with('[')
            {
                RequestBody::Json {
                    content: raw_text.to_string(),
                }
            } else {
                RequestBody::Raw {
                    content: raw_text.to_string(),
                    content_type: if lang.is_empty() {
                        "text/plain".to_string()
                    } else {
                        format!("application/{}", lang)
                    },
                }
            }
        }
        "urlencoded" => {
            let mut items = Vec::new();
            if let Some(arr) = b.get("urlencoded").and_then(|u| u.as_array()) {
                for item in arr {
                    if let Some(k) = item.get("key").and_then(|k| k.as_str()) {
                        let v = item.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        let enabled = !item
                            .get("disabled")
                            .and_then(|d| d.as_bool())
                            .unwrap_or(false);
                        let desc = item
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(String::from);
                        items.push(KeyValuePair {
                            enabled,
                            key: k.to_string(),
                            value: v.to_string(),
                            description: desc,
                        });
                    }
                }
            }
            RequestBody::FormUrlEncoded { params: items }
        }
        "formdata" => {
            let mut items = Vec::new();
            if let Some(arr) = b.get("formdata").and_then(|f| f.as_array()) {
                for item in arr {
                    if let Some(k) = item.get("key").and_then(|k| k.as_str()) {
                        let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                        let enabled = !item
                            .get("disabled")
                            .and_then(|d| d.as_bool())
                            .unwrap_or(false);
                        if item_type == "file" {
                            let src = item.get("src").and_then(|s| s.as_str()).unwrap_or("");
                            let mut m = MultipartItem::new_file(k, src);
                            m.enabled = enabled;
                            items.push(m);
                        } else {
                            let val = item.get("value").and_then(|v| v.as_str()).unwrap_or("");
                            let mut m = MultipartItem::new_text(k, val);
                            m.enabled = enabled;
                            items.push(m);
                        }
                    }
                }
            }
            RequestBody::Multipart { items }
        }
        _ => RequestBody::None,
    }
}

fn extract_postman_scripts(event_val: Option<&Value>) -> Option<ScriptConfig> {
    let arr = event_val?.as_array()?;
    let mut pre_request = None;
    let mut post_response = None;

    for ev in arr {
        let listen = ev.get("listen").and_then(|l| l.as_str()).unwrap_or("");
        let script_lines = ev.pointer("/script/exec").and_then(|e| e.as_array());

        let code = if let Some(lines) = script_lines {
            let joined: Vec<&str> = lines.iter().filter_map(|l| l.as_str()).collect();
            joined.join("\n")
        } else if let Some(raw) = ev.pointer("/script/exec").and_then(|e| e.as_str()) {
            raw.to_string()
        } else {
            String::new()
        };

        if !code.trim().is_empty() {
            if listen == "prerequest" {
                pre_request = Some(code);
            } else if listen == "test" {
                post_response = Some(code);
            }
        }
    }

    if pre_request.is_some() || post_response.is_some() {
        Some(ScriptConfig {
            pre_request,
            post_response,
        })
    } else {
        None
    }
}
