use super::*;
use crate::graphql::introspection::parse_introspection;

/// Small schema: Query.user(id: ID!, role: Role) -> User, Query.users -> [User!],
/// User.friends -> [User], User.name(role: Role) -> String, Role enum,
/// UserFilter input object, plus a Mutation root and a Node interface.
fn schema() -> GraphQLSchemaSummary {
    let body = serde_json::json!({
        "data": { "__schema": {
            "queryType": { "name": "Query" },
            "mutationType": { "name": "Mutation" },
            "subscriptionType": null,
            "types": [
                { "kind": "OBJECT", "name": "Query", "fields": [
                    { "name": "user", "type": { "kind": "OBJECT", "name": "User" },
                      "args": [
                        { "name": "id", "type": { "kind": "NON_NULL", "ofType": { "kind": "SCALAR", "name": "ID" } } },
                        { "name": "role", "type": { "kind": "ENUM", "name": "Role" } },
                        { "name": "filter", "type": { "kind": "INPUT_OBJECT", "name": "UserFilter" } },
                        { "name": "roles", "type": { "kind": "LIST", "ofType": { "kind": "ENUM", "name": "Role" } } }
                      ] },
                    { "name": "users", "type": { "kind": "LIST", "ofType": { "kind": "OBJECT", "name": "User" } }, "args": [] },
                    { "name": "node", "type": { "kind": "INTERFACE", "name": "Node" }, "args": [] }
                ], "inputFields": null, "enumValues": null },
                { "kind": "OBJECT", "name": "User", "fields": [
                    { "name": "name", "type": { "kind": "SCALAR", "name": "String" }, "args": [] },
                    { "name": "friends", "type": { "kind": "LIST", "ofType": { "kind": "OBJECT", "name": "User" } }, "args": [] }
                ], "inputFields": null, "enumValues": null },
                { "kind": "OBJECT", "name": "Mutation", "fields": [
                    { "name": "createUser", "type": { "kind": "OBJECT", "name": "User" },
                      "args": [ { "name": "input", "type": { "kind": "NON_NULL", "ofType": { "kind": "INPUT_OBJECT", "name": "UserFilter" } } } ] }
                ], "inputFields": null, "enumValues": null },
                { "kind": "INTERFACE", "name": "Node", "fields": [
                    { "name": "id", "type": { "kind": "SCALAR", "name": "ID" }, "args": [] }
                ], "inputFields": null, "enumValues": null },
                { "kind": "ENUM", "name": "Role", "fields": null, "inputFields": null,
                  "enumValues": [ { "name": "ADMIN" }, { "name": "MEMBER" } ] },
                { "kind": "INPUT_OBJECT", "name": "UserFilter", "fields": null,
                  "inputFields": [
                    { "name": "nameContains", "type": { "kind": "SCALAR", "name": "String" } },
                    { "name": "role", "type": { "kind": "ENUM", "name": "Role" } }
                  ], "enumValues": null },
                { "kind": "SCALAR", "name": "String", "fields": null, "inputFields": null, "enumValues": null },
                { "kind": "SCALAR", "name": "ID", "fields": null, "inputFields": null, "enumValues": null },
                { "kind": "SCALAR", "name": "Boolean", "fields": null, "inputFields": null, "enumValues": null }
            ]
        } }
    });
    parse_introspection(&serde_json::to_vec(&body).unwrap()).unwrap()
}

fn labels(text: &str, caret: usize, s: &GraphQLSchemaSummary) -> Vec<String> {
    completions_at(text, caret, s)
        .into_iter()
        .map(|c| c.label)
        .collect()
}

fn labels_at_end(text: &str, s: &GraphQLSchemaSummary) -> Vec<String> {
    labels(text, text.len(), s)
}

#[test]
fn test_root_fields_suggested() {
    let s = schema();
    let ls = labels_at_end("query { ", &s);
    assert!(ls.contains(&"user".to_string()));
    assert!(ls.contains(&"users".to_string()));
    assert!(ls.contains(&"node".to_string()));
    assert!(ls.contains(&"__typename".to_string()));
}

#[test]
fn test_prefix_filters_suggestions() {
    let s = schema();
    let ls = labels_at_end("query { us", &s);
    assert_eq!(ls, vec!["user".to_string(), "users".to_string()]);
}

#[test]
fn test_nested_field_context() {
    let s = schema();
    let ls = labels_at_end("query { user { ", &s);
    assert!(ls.contains(&"name".to_string()));
    assert!(ls.contains(&"friends".to_string()));
    assert!(
        !ls.contains(&"user".to_string()),
        "no root fields inside User"
    );
}

#[test]
fn test_deeply_nested_context() {
    let s = schema();
    let ls = labels_at_end("query { user { friends { ", &s);
    assert!(ls.contains(&"name".to_string()));
    assert!(ls.contains(&"friends".to_string()));
}

#[test]
fn test_sibling_fields_after_nested_close() {
    let s = schema();
    // Typing a sibling inside the still-open User selection.
    let ls = labels_at_end("query { user { name fr", &s);
    assert_eq!(
        ls,
        vec!["friends".to_string()],
        "prefix 'fr' filters siblings"
    );
}

#[test]
fn test_alias_field_resolved() {
    let s = schema();
    // After `u: user {` the context is User.
    let ls = labels_at_end("query { u: user { ", &s);
    assert!(ls.contains(&"name".to_string()));
    // Typing an alias prefix suggests fields (filtered by prefix).
    let ls = labels_at_end("query { u", &s);
    assert!(ls.contains(&"user".to_string()));
}

#[test]
fn test_argument_names_after_open_paren() {
    let s = schema();
    let ls = labels_at_end("query { user(", &s);
    assert!(ls.contains(&"id".to_string()));
    assert!(ls.contains(&"role".to_string()));
    assert!(ls.contains(&"filter".to_string()));
}

#[test]
fn test_enum_values_in_argument_value() {
    let s = schema();
    let ls = labels_at_end("query { user(role: ", &s);
    assert_eq!(ls, vec!["ADMIN".to_string(), "MEMBER".to_string()]);
    let ls = labels_at_end("query { user(role: A", &s);
    assert_eq!(ls, vec!["ADMIN".to_string()]);
}

#[test]
fn test_enum_values_inside_list_literal() {
    let s = schema();
    let ls = labels_at_end("query { user(roles: [", &s);
    assert!(ls.contains(&"ADMIN".to_string()));
    // Second element of the list.
    let ls = labels_at_end("query { user(roles: [ADMIN, ", &s);
    assert!(ls.contains(&"MEMBER".to_string()));
}

#[test]
fn test_input_object_fields_after_brace() {
    let s = schema();
    let ls = labels_at_end("query { user(filter: { ", &s);
    assert!(ls.contains(&"nameContains".to_string()));
    assert!(ls.contains(&"role".to_string()));
}

#[test]
fn test_nested_input_object_enum_value() {
    let s = schema();
    let ls = labels_at_end("query { user(filter: { role: ", &s);
    assert!(ls.contains(&"ADMIN".to_string()));
}

#[test]
fn test_input_object_in_mutation() {
    let s = schema();
    let ls = labels_at_end("mutation { createUser(input: { ", &s);
    assert!(ls.contains(&"nameContains".to_string()));
}

#[test]
fn test_inline_fragment_switches_context() {
    let s = schema();
    let ls = labels_at_end("query { node { ... on Node { ", &s);
    assert!(ls.contains(&"id".to_string()));
}

#[test]
fn test_type_condition_suggests_types() {
    let s = schema();
    let ls = labels_at_end("query { node { ... on ", &s);
    assert!(ls.contains(&"Node".to_string()));
    assert!(ls.contains(&"User".to_string()));
}

#[test]
fn test_fragment_definition_body_context() {
    let s = schema();
    let ls = labels_at_end("query { user { ...f } } fragment f on User { ", &s);
    assert!(ls.contains(&"name".to_string()));
}

#[test]
fn test_fragment_spread_names_suggested() {
    let s = schema();
    let ls = labels_at_end(
        "query { user { ...f } } fragment f on User { name } query2 { user { ...",
        &s,
    );
    assert!(ls.contains(&"f".to_string()));
}

#[test]
fn test_directives_after_at_sign() {
    let s = schema();
    let ls = labels_at_end("query { user @", &s);
    assert!(ls.contains(&"skip".to_string()));
    assert!(ls.contains(&"include".to_string()));
    // Typed without the @ in the prefix.
    let ls = labels_at_end("query { user @sk", &s);
    assert_eq!(ls, vec!["skip".to_string()]);
}

#[test]
fn test_siblings_after_complete_field() {
    let s = schema();
    // Caret after a complete field name followed by a space: siblings.
    let ls = labels_at_end("query { user ", &s);
    assert!(ls.contains(&"user".to_string()));
    assert!(ls.contains(&"users".to_string()));
}

#[test]
fn test_caret_inside_string_yields_nothing() {
    let s = schema();
    let text = "query { user(id: \"a{b\") }";
    let caret = text.find("a{").unwrap() + 2; // inside the string literal
    assert!(completions_at(text, caret, &s).is_empty());
}

#[test]
fn test_caret_in_comment_ignores_braces() {
    let s = schema();
    // The braces after # are comments — context is still the root selection.
    let text = "query {\n# {{{ weird\n";
    let ls = labels_at_end(text, &s);
    assert!(ls.contains(&"user".to_string()));
}

#[test]
fn test_mutation_root_context() {
    let s = schema();
    let ls = labels_at_end("mutation { ", &s);
    assert!(ls.contains(&"createUser".to_string()));
    assert!(!ls.contains(&"user".to_string()));
}

#[test]
fn test_no_suggestions_inside_var_defs() {
    let s = schema();
    let ls = labels_at_end("query Q($v: ", &s);
    assert!(ls.is_empty());
}

#[test]
fn test_no_suggestions_at_top_level() {
    let s = schema();
    assert!(labels_at_end("", &s).is_empty());
}

#[test]
fn test_caret_mid_word_uses_partial_prefix() {
    let s = schema();
    let text = "query { user { fr";
    let ls = labels(text, text.len(), &s);
    assert_eq!(ls, vec!["friends".to_string()]);
}

#[test]
fn test_caret_inside_multibyte_name_is_safe() {
    let s = schema();
    let text = "query { user { tên";
    let caret = text.len();
    // No panic; the partial name splits at the non-ASCII char and only the
    // ASCII tail completes against the schema.
    let ls = completions_at(text, caret, &s)
        .into_iter()
        .map(|c| c.label)
        .collect::<Vec<_>>();
    assert_eq!(ls, vec!["name".to_string()]);
}

#[test]
fn test_malformed_document_does_not_panic() {
    let s = schema();
    for text in ["{{{", "query", "query { user((", "}}}", "...", "fragment"] {
        let _ = completions_at(text, text.len(), &s);
    }
}
