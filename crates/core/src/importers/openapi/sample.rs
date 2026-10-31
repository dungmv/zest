use super::resolve_ref;
use serde_json::{json, Map, Value};

pub fn generate_media_sample(root: &Value, media_obj: &Value) -> String {
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

pub fn generate_schema_sample(root: &Value, schema: &Value, depth: usize) -> Value {
    if depth > 4 {
        return json!({});
    }
    let res_schema = resolve_ref(root, schema);

    if let Some(ex) = res_schema.get("example") {
        return ex.clone();
    }
    if let Some(examples) = res_schema.get("examples").and_then(|e| e.as_array()) {
        if let Some(first) = examples.first() {
            return first.clone();
        }
    }
    if let Some(default_val) = res_schema.get("default") {
        return default_val.clone();
    }
    if let Some(enum_vals) = res_schema.get("enum").and_then(|e| e.as_array()) {
        if let Some(first) = enum_vals.first() {
            return first.clone();
        }
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
        Some("integer") | Some("number") => json!(0),
        Some("boolean") => json!(true),
        _ => {
            // Check allOf
            if let Some(all_of) = res_schema.get("allOf").and_then(|a| a.as_array()) {
                let mut merged = Map::new();
                for sub in all_of {
                    let sub_val = generate_schema_sample(root, sub, depth + 1);
                    if let Value::Object(sub_map) = sub_val {
                        for (k, v) in sub_map {
                            merged.insert(k, v);
                        }
                    }
                }
                return Value::Object(merged);
            }
            json!({})
        }
    }
}

pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(value_to_string).collect();
            items.join(",")
        }
        _ => v.to_string(),
    }
}
