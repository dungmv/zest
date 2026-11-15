//! The standard introspection query sent by "Fetch Schema" and the parser
//! that turns the JSON response into a [`GraphQLSchemaSummary`].

use std::collections::HashMap;

use super::schema::{
    GraphQLFieldDef, GraphQLInputValueDef, GraphQLSchemaSummary, GraphQLTypeDef, GraphQLTypeKind,
};

/// Standard introspection document (compatibility-oriented: no
/// `includeDeprecated` arguments, so pre-2018 servers still accept it).
pub const INTROSPECTION_QUERY: &str = r#"query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types {
      kind
      name
      description
      fields {
        name
        description
        args {
          name
          description
          defaultValue
          type { ...TypeRef }
        }
        type { ...TypeRef }
      }
      inputFields {
        name
        description
        defaultValue
        type { ...TypeRef }
      }
      enumValues { name }
    }
  }
}

fragment TypeRef on __Type {
  kind
  name
  ofType {
    kind
    name
    ofType {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
            }
          }
        }
      }
    }
  }
}"#;

/// Parses an introspection HTTP response body into a schema summary.
pub fn parse_introspection(body: &[u8]) -> Result<GraphQLSchemaSummary, String> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| format!("Schema response is not valid JSON: {e}"))?;
    let schema = value
        .get("data")
        .and_then(|d| d.get("__schema"))
        .ok_or("Response has no data.__schema — is this a GraphQL endpoint?")?;

    let query_type = schema
        .get("queryType")
        .and_then(|t| t.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Query")
        .to_string();
    let mutation_type = optional_type_name(schema.get("mutationType"));
    let subscription_type = optional_type_name(schema.get("subscriptionType"));

    let mut types = HashMap::new();
    if let Some(list) = schema.get("types").and_then(|t| t.as_array()) {
        for raw in list {
            let name = raw.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let kind_str = raw.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            if name.is_empty() || name.starts_with("__") {
                continue; // built-in meta types
            }
            let Some(kind) = parse_kind(kind_str) else {
                continue;
            };
            let def = GraphQLTypeDef {
                kind,
                description: optional_string(raw.get("description")),
                fields: parse_fields(raw.get("fields")),
                input_fields: parse_input_values(raw.get("inputFields")),
                enum_values: raw
                    .get("enumValues")
                    .and_then(|v| v.as_array())
                    .map(|vals| {
                        vals.iter()
                            .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            types.insert(name.to_string(), def);
        }
    }

    Ok(GraphQLSchemaSummary {
        query_type,
        mutation_type,
        subscription_type,
        types,
    })
}

fn parse_kind(kind: &str) -> Option<GraphQLTypeKind> {
    match kind {
        "SCALAR" => Some(GraphQLTypeKind::Scalar),
        "OBJECT" => Some(GraphQLTypeKind::Object),
        "INTERFACE" => Some(GraphQLTypeKind::Interface),
        "UNION" => Some(GraphQLTypeKind::Union),
        "ENUM" => Some(GraphQLTypeKind::Enum),
        "INPUT_OBJECT" => Some(GraphQLTypeKind::InputObject),
        _ => None,
    }
}

fn optional_string(v: Option<&serde_json::Value>) -> Option<String> {
    v.and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn optional_type_name(v: Option<&serde_json::Value>) -> Option<String> {
    v.and_then(|t| t.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
}

fn parse_fields(v: Option<&serde_json::Value>) -> Vec<GraphQLFieldDef> {
    v.and_then(|v| v.as_array())
        .map(|fields| {
            fields
                .iter()
                .filter_map(|f| {
                    let name = f.get("name").and_then(|n| n.as_str())?;
                    if name.starts_with("__") {
                        return None;
                    }
                    let (type_display, type_name) = parse_type_ref(f.get("type"));
                    Some(GraphQLFieldDef {
                        name: name.to_string(),
                        description: optional_string(f.get("description")),
                        type_name,
                        type_display,
                        args: parse_input_values(f.get("args")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_input_values(v: Option<&serde_json::Value>) -> Vec<GraphQLInputValueDef> {
    v.and_then(|v| v.as_array())
        .map(|vals| {
            vals.iter()
                .filter_map(|a| {
                    let name = a.get("name").and_then(|n| n.as_str())?;
                    let (type_display, type_name) = parse_type_ref(a.get("type"));
                    Some(GraphQLInputValueDef {
                        name: name.to_string(),
                        description: optional_string(a.get("description")),
                        type_name,
                        type_display,
                        default_value: optional_string(a.get("defaultValue")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reconstructs the full type signature from the nested TypeRef chain and
/// returns `(display, base_named_type)` — e.g. `["User"!]!` / `User`.
fn parse_type_ref(v: Option<&serde_json::Value>) -> (String, String) {
    let Some(v) = v else {
        return ("?".to_string(), String::new());
    };
    let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    match kind {
        "NON_NULL" => {
            let (mut display, named) = parse_type_ref(v.get("ofType"));
            display.push('!');
            (display, named)
        }
        "LIST" => {
            let (inner, named) = parse_type_ref(v.get("ofType"));
            (format!("[{inner}]"), named)
        }
        _ => {
            let name = v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("?")
                .to_string();
            (name.clone(), name)
        }
    }
}

#[cfg(test)]
#[path = "introspection_tests.rs"]
mod tests;
