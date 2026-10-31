use super::resolve_ref;
use super::sample::{generate_media_sample, generate_schema_sample, value_to_string};
use crate::models::{KeyValuePair, MultipartItem, RequestBody};
use serde_json::{Map, Value};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SwaggerFormDataParam {
    pub name: String,
    pub value: String,
    pub is_file: bool,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RawFormField {
    pub name: String,
    pub value: String,
    pub is_file: bool,
    pub enabled: bool,
    pub required: bool,
    pub description: Option<String>,
}

pub fn normalize_mime_type(media_type: &str) -> &str {
    media_type.split(';').next().unwrap_or("").trim()
}

pub fn extract_request_body(
    root: &Value,
    op_obj: &Map<String, Value>,
    all_params: &[Value],
    form_params: &[SwaggerFormDataParam],
) -> (RequestBody, Option<String>) {
    // 1. OpenAPI 3.x requestBody
    if let Some(rb) = op_obj.get("requestBody") {
        let rb_resolved = resolve_ref(root, rb);
        if let Some(content) = rb_resolved.get("content").and_then(|c| c.as_object()) {
            // Priority 1: application/json or +json
            for (media_type, media_obj) in content {
                let norm = normalize_mime_type(media_type);
                if norm.eq_ignore_ascii_case("application/json") || norm.ends_with("+json") {
                    let sample_json = generate_media_sample(root, media_obj);
                    return (
                        RequestBody::Json {
                            content: sample_json,
                        },
                        Some("application/json".to_string()),
                    );
                }
            }

            // Priority 2: multipart/form-data
            for (media_type, media_obj) in content {
                let norm = normalize_mime_type(media_type);
                if norm.eq_ignore_ascii_case("multipart/form-data")
                    || norm.to_ascii_lowercase().starts_with("multipart/")
                {
                    let raw_fields = collect_schema_form_fields(root, media_obj, true);
                    let items = raw_fields
                        .into_iter()
                        .map(|f| {
                            let mut item = if f.is_file {
                                MultipartItem::new_file(f.name, f.value)
                            } else {
                                MultipartItem::new_text(f.name, f.value)
                            };
                            item.enabled = f.enabled;
                            item
                        })
                        .collect();
                    return (
                        RequestBody::Multipart { items },
                        Some("multipart/form-data".to_string()),
                    );
                }
            }

            // Priority 3: application/x-www-form-urlencoded
            for (media_type, media_obj) in content {
                let norm = normalize_mime_type(media_type);
                if norm.eq_ignore_ascii_case("application/x-www-form-urlencoded")
                    || norm.eq_ignore_ascii_case("application/x-url-encoded")
                {
                    let raw_fields = collect_schema_form_fields(root, media_obj, false);
                    let params = raw_fields
                        .into_iter()
                        .map(|f| KeyValuePair {
                            enabled: f.enabled,
                            key: f.name,
                            value: f.value,
                            description: f.description,
                        })
                        .collect();
                    return (
                        RequestBody::FormUrlEncoded { params },
                        Some("application/x-www-form-urlencoded".to_string()),
                    );
                }
            }

            // Priority 4: Raw / other media type fallback
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

    // 2. Swagger 2.0 in: formData parameters
    if !form_params.is_empty() {
        let has_file = form_params.iter().any(|p| p.is_file);
        let consumes_multipart = is_consumes_type(op_obj, root, "multipart/form-data");
        let is_multipart = has_file || consumes_multipart;

        if is_multipart {
            let items = form_params
                .iter()
                .cloned()
                .map(|p| {
                    let mut item = if p.is_file {
                        MultipartItem::new_file(p.name, p.value)
                    } else {
                        MultipartItem::new_text(p.name, p.value)
                    };
                    item.enabled = true;
                    item
                })
                .collect();
            return (
                RequestBody::Multipart { items },
                Some("multipart/form-data".to_string()),
            );
        } else {
            let params = form_params
                .iter()
                .cloned()
                .map(|p| KeyValuePair {
                    enabled: true,
                    key: p.name,
                    value: p.value,
                    description: p.description,
                })
                .collect();
            return (
                RequestBody::FormUrlEncoded { params },
                Some("application/x-www-form-urlencoded".to_string()),
            );
        }
    }

    // 3. Swagger 2.0 in: body parameters (checking all merged parameters)
    for param in all_params {
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

    (RequestBody::None, None)
}

fn is_consumes_type(op_obj: &Map<String, Value>, root: &Value, expected: &str) -> bool {
    let check_arr = |val: Option<&Value>| -> bool {
        if let Some(arr) = val.and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    if normalize_mime_type(s).eq_ignore_ascii_case(expected) {
                        return true;
                    }
                }
            }
        }
        false
    };

    check_arr(op_obj.get("consumes")) || check_arr(root.get("consumes"))
}

pub fn collect_schema_form_fields(
    root: &Value,
    media_obj: &Value,
    is_multipart: bool,
) -> Vec<RawFormField> {
    let mut fields = Vec::new();
    let mut seen_keys = HashSet::new();

    // 1. Traverse schema properties (with allOf / oneOf / anyOf support)
    if let Some(schema) = media_obj.get("schema") {
        let mut visited = HashSet::new();
        collect_from_schema_recursive(
            root,
            schema,
            0,
            &mut visited,
            &mut fields,
            &mut seen_keys,
            is_multipart,
        );
    }

    // 2. Fallback to schema-level example or default if no properties were found
    if fields.is_empty() {
        if let Some(schema) = media_obj.get("schema") {
            let res_schema = resolve_ref(root, schema);
            if let Some(ex) = res_schema
                .get("example")
                .or_else(|| res_schema.get("default"))
            {
                extract_fields_from_example_value(ex, &mut fields, &mut seen_keys);
            }
        }
    }

    // 3. Fallback to media-level example
    if fields.is_empty() {
        if let Some(ex) = media_obj.get("example") {
            extract_fields_from_example_value(ex, &mut fields, &mut seen_keys);
        }
    }

    // 4. Fallback to media-level examples (take first example)
    if fields.is_empty() {
        if let Some(examples) = media_obj.get("examples").and_then(|e| e.as_object()) {
            if let Some((_, first_ex)) = examples.iter().next() {
                let res_ex = resolve_ref(root, first_ex);
                if let Some(val) = res_ex.get("value") {
                    extract_fields_from_example_value(val, &mut fields, &mut seen_keys);
                }
            }
        }
    }

    // 5. For multipart, inspect encoding object for contentType hints
    if is_multipart {
        if let Some(encoding) = media_obj.get("encoding").and_then(|e| e.as_object()) {
            for f in &mut fields {
                if let Some(enc_obj) = encoding.get(&f.name) {
                    if let Some(ct) = enc_obj.get("contentType").and_then(|c| c.as_str()) {
                        let ct_lower = ct.to_ascii_lowercase();
                        if !ct_lower.starts_with("text/") && ct_lower != "application/json" {
                            f.is_file = true;
                        }
                    }
                }
            }
        }
    }

    fields
}

fn collect_from_schema_recursive(
    root: &Value,
    schema: &Value,
    depth: usize,
    visited: &mut HashSet<String>,
    out_fields: &mut Vec<RawFormField>,
    seen_keys: &mut HashSet<String>,
    is_multipart: bool,
) {
    if depth > 16 {
        return;
    }
    let res_schema = resolve_ref(root, schema);

    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
        if !visited.insert(ref_str.to_string()) {
            return;
        }
    }

    let required_set: HashSet<String> = res_schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    // Collect direct properties
    if let Some(props) = res_schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_val) in props {
            if seen_keys.contains(prop_name) {
                continue;
            }
            let res_prop = resolve_ref(root, prop_val);
            let is_req = required_set.contains(prop_name);
            let desc = res_prop
                .get("description")
                .or_else(|| prop_val.get("description"))
                .and_then(|d| d.as_str())
                .map(String::from);

            let is_file = if is_multipart {
                is_file_schema(res_prop)
            } else {
                false
            };

            let sample = if is_file {
                extract_string_sample(root, res_prop)
            } else {
                let sample_val = generate_schema_sample(root, res_prop, 0);
                value_to_string(&sample_val)
            };

            seen_keys.insert(prop_name.clone());
            out_fields.push(RawFormField {
                name: prop_name.clone(),
                value: sample,
                is_file,
                enabled: true,
                required: is_req,
                description: desc,
            });
        }
    }

    // Traverse allOf
    if let Some(all_of) = res_schema.get("allOf").and_then(|a| a.as_array()) {
        for sub_schema in all_of {
            collect_from_schema_recursive(
                root,
                sub_schema,
                depth + 1,
                visited,
                out_fields,
                seen_keys,
                is_multipart,
            );
        }
    }

    // If no properties found, inspect oneOf / anyOf
    if out_fields.is_empty() {
        if let Some(variants) = res_schema
            .get("oneOf")
            .or_else(|| res_schema.get("anyOf"))
            .and_then(|a| a.as_array())
        {
            if let Some(first_sub) = variants.first() {
                collect_from_schema_recursive(
                    root,
                    first_sub,
                    depth + 1,
                    visited,
                    out_fields,
                    seen_keys,
                    is_multipart,
                );
            }
        }
    }
}

pub fn is_file_schema(schema: &Value) -> bool {
    let t = schema.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let f = schema.get("format").and_then(|v| v.as_str()).unwrap_or("");
    if t.eq_ignore_ascii_case("file") {
        return true;
    }
    if t.eq_ignore_ascii_case("string")
        && (f.eq_ignore_ascii_case("binary") || f.eq_ignore_ascii_case("byte"))
    {
        return true;
    }
    if t.eq_ignore_ascii_case("array") {
        if let Some(items) = schema.get("items") {
            let item_t = items.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let item_f = items.get("format").and_then(|v| v.as_str()).unwrap_or("");
            if item_t.eq_ignore_ascii_case("file")
                || (item_t.eq_ignore_ascii_case("string")
                    && (item_f.eq_ignore_ascii_case("binary")
                        || item_f.eq_ignore_ascii_case("byte")))
            {
                return true;
            }
        }
    }
    false
}

fn extract_string_sample(root: &Value, schema: &Value) -> String {
    let res = resolve_ref(root, schema);
    if let Some(ex) = res.get("example") {
        return value_to_string(ex);
    }
    if let Some(def) = res.get("default") {
        return value_to_string(def);
    }
    if let Some(examples) = res.get("examples").and_then(|e| e.as_array()) {
        if let Some(first) = examples.first() {
            return value_to_string(first);
        }
    }
    String::new()
}

fn extract_fields_from_example_value(
    val: &Value,
    fields: &mut Vec<RawFormField>,
    seen_keys: &mut HashSet<String>,
) {
    match val {
        Value::Object(map) => {
            // Check if this object contains a list of form/param items under common wrapper keys
            let wrapper_keys = [
                "form",
                "params",
                "data",
                "items",
                "body",
                "fields",
                "formData",
                "urlencoded",
                "values",
            ];
            for wk in wrapper_keys {
                if let Some(arr) = map.get(wk).and_then(|v| v.as_array()) {
                    if try_extract_from_item_list(arr, fields, seen_keys) {
                        return;
                    }
                }
            }

            // Check if single-key object wraps an array of param objects
            if map.len() == 1 {
                if let Some(arr) = map.values().next().and_then(|v| v.as_array()) {
                    if try_extract_from_item_list(arr, fields, seen_keys) {
                        return;
                    }
                }
            }

            // Check if this object itself is a single param item: { "name": "...", "value": "..." }
            if let Some(name) = map
                .get("name")
                .or_else(|| map.get("key"))
                .and_then(|v| v.as_str())
            {
                if map.contains_key("value") {
                    let k = name.trim();
                    if !k.is_empty() && seen_keys.insert(k.to_string()) {
                        let v_str = map.get("value").map(value_to_string).unwrap_or_default();
                        let is_file = is_file_param(map);
                        let desc = map
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(String::from);
                        let enabled = map.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true);
                        fields.push(RawFormField {
                            name: k.to_string(),
                            value: v_str,
                            is_file,
                            enabled,
                            required: enabled,
                            description: desc,
                        });
                        return;
                    }
                }
            }

            // Standard object mapping: { "field1": "val1", "field2": "val2" }
            for (k, v) in map {
                let trimmed_k = k.trim();
                if !trimmed_k.is_empty() && seen_keys.insert(trimmed_k.to_string()) {
                    fields.push(RawFormField {
                        name: trimmed_k.to_string(),
                        value: value_to_string(v),
                        is_file: false,
                        enabled: true,
                        required: true,
                        description: None,
                    });
                }
            }
        }
        Value::Array(arr) => {
            if try_extract_from_item_list(arr, fields, seen_keys) {
                return;
            }
            // If it's an array of strings like ["k1=v1", "k2=v2"]
            for item in arr {
                if let Value::String(s) = item {
                    extract_from_url_encoded_string(s, fields, seen_keys);
                }
            }
        }
        Value::String(s) => {
            let trimmed = s.trim();
            // If string is JSON: "{\"form\": [...]}" or "{\"id\": \"123\"}"
            if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('[') && trimmed.ends_with(']'))
            {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    extract_fields_from_example_value(&parsed, fields, seen_keys);
                    if !fields.is_empty() {
                        return;
                    }
                }
            }

            // If string contains URL-encoded query string: "k1=v1&k2=v2"
            extract_from_url_encoded_string(s, fields, seen_keys);
        }
        _ => {}
    }
}

fn try_extract_from_item_list(
    arr: &[Value],
    fields: &mut Vec<RawFormField>,
    seen_keys: &mut HashSet<String>,
) -> bool {
    let mut extracted_any = false;
    for item in arr {
        if let Some(map) = item.as_object() {
            let name_opt = map
                .get("name")
                .or_else(|| map.get("key"))
                .and_then(|v| v.as_str());
            if let Some(name) = name_opt {
                let k = name.trim();
                if k.is_empty() {
                    continue; // Skip empty rows (e.g. { "name": "", "value": "" })
                }
                if seen_keys.insert(k.to_string()) {
                    let v_str = map.get("value").map(value_to_string).unwrap_or_default();
                    let is_file = is_file_param(map);
                    let desc = map
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(String::from);
                    let enabled = map.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true);

                    fields.push(RawFormField {
                        name: k.to_string(),
                        value: v_str,
                        is_file,
                        enabled,
                        required: enabled,
                        description: desc,
                    });
                    extracted_any = true;
                }
            }
        }
    }
    extracted_any
}

fn is_file_param(map: &Map<String, Value>) -> bool {
    if let Some(is_f) = map
        .get("isFile")
        .or_else(|| map.get("is_file"))
        .or_else(|| map.get("file"))
        .and_then(|v| v.as_bool())
    {
        return is_f;
    }
    if let Some(t) = map.get("type").and_then(|v| v.as_str()) {
        if t.eq_ignore_ascii_case("file") || t.eq_ignore_ascii_case("binary") {
            return true;
        }
    }
    false
}

fn extract_from_url_encoded_string(
    s: &str,
    fields: &mut Vec<RawFormField>,
    seen_keys: &mut HashSet<String>,
) {
    if s.contains('=') {
        for pair in s.split('&') {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next().unwrap_or("").trim();
            let v = parts.next().unwrap_or("").trim();
            if !k.is_empty() && seen_keys.insert(k.to_string()) {
                let decoded_v = urlencoding::decode(v).unwrap_or(std::borrow::Cow::Borrowed(v));
                fields.push(RawFormField {
                    name: k.to_string(),
                    value: decoded_v.into_owned(),
                    is_file: false,
                    enabled: true,
                    required: true,
                    description: None,
                });
            }
        }
    }
}
