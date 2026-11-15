use super::*;

fn sample_introspection_body() -> Vec<u8> {
    serde_json::json!({
        "data": {
            "__schema": {
                "queryType": { "name": "Query" },
                "mutationType": { "name": "Mutation" },
                "subscriptionType": null,
                "types": [
                    {
                        "kind": "OBJECT",
                        "name": "Query",
                        "description": "Root query",
                        "fields": [
                            {
                                "name": "user",
                                "description": "Find a user",
                                "args": [
                                    {
                                        "name": "id",
                                        "description": null,
                                        "defaultValue": null,
                                        "type": { "kind": "NON_NULL", "name": null, "ofType": { "kind": "SCALAR", "name": "ID", "ofType": null } }
                                    },
                                    {
                                        "name": "role",
                                        "description": null,
                                        "defaultValue": "ADMIN",
                                        "type": { "kind": "ENUM", "name": "Role", "ofType": null }
                                    }
                                ],
                                "type": { "kind": "OBJECT", "name": "User", "ofType": null }
                            },
                            {
                                "name": "users",
                                "description": null,
                                "args": [],
                                "type": { "kind": "LIST", "name": null, "ofType": { "kind": "NON_NULL", "name": null, "ofType": { "kind": "OBJECT", "name": "User", "ofType": null } } }
                            }
                        ],
                        "inputFields": null,
                        "enumValues": null
                    },
                    {
                        "kind": "OBJECT",
                        "name": "User",
                        "description": null,
                        "fields": [
                            { "name": "name", "description": null, "args": [], "type": { "kind": "SCALAR", "name": "String", "ofType": null } },
                            { "name": "friends", "description": null, "args": [], "type": { "kind": "LIST", "name": null, "ofType": { "kind": "OBJECT", "name": "User", "ofType": null } } }
                        ],
                        "inputFields": null,
                        "enumValues": null
                    },
                    {
                        "kind": "ENUM",
                        "name": "Role",
                        "description": null,
                        "fields": null,
                        "inputFields": null,
                        "enumValues": [ { "name": "ADMIN" }, { "name": "MEMBER" } ]
                    },
                    {
                        "kind": "INPUT_OBJECT",
                        "name": "UserFilter",
                        "description": null,
                        "fields": null,
                        "inputFields": [
                            { "name": "nameContains", "description": null, "defaultValue": null, "type": { "kind": "SCALAR", "name": "String", "ofType": null } }
                        ],
                        "enumValues": null
                    },
                    {
                        "kind": "SCALAR",
                        "name": "__TypeNameShouldBeSkipped",
                        "description": null,
                        "fields": null, "inputFields": null, "enumValues": null
                    }
                ]
            }
        }
    })
    .to_string()
    .into_bytes()
}

#[test]
fn test_parse_introspection_builds_summary() {
    let summary = parse_introspection(&sample_introspection_body()).unwrap();
    assert_eq!(summary.query_type, "Query");
    assert_eq!(summary.mutation_type.as_deref(), Some("Mutation"));
    assert_eq!(summary.subscription_type, None);
    assert!(summary.types.contains_key("Query"));
    assert!(summary.types.contains_key("User"));
    assert!(summary.types.contains_key("Role"));
    assert!(summary.types.contains_key("UserFilter"));
    assert!(!summary.types.contains_key("__TypeNameShouldBeSkipped"));
}

#[test]
fn test_field_types_unwrapped_and_displayed() {
    let summary = parse_introspection(&sample_introspection_body()).unwrap();
    let user = summary.field("Query", "user").unwrap();
    assert_eq!(user.type_name, "User");
    assert_eq!(user.type_display, "User");
    let users = summary.field("Query", "users").unwrap();
    assert_eq!(users.type_name, "User");
    assert_eq!(users.type_display, "[User!]");

    let id_arg = user.args.iter().find(|a| a.name == "id").unwrap();
    assert_eq!(id_arg.type_name, "ID");
    assert_eq!(id_arg.type_display, "ID!");
}

#[test]
fn test_enum_values_and_input_fields() {
    let summary = parse_introspection(&sample_introspection_body()).unwrap();
    let role = summary.types.get("Role").unwrap();
    assert_eq!(role.kind, GraphQLTypeKind::Enum);
    assert_eq!(role.enum_values, vec!["ADMIN", "MEMBER"]);

    let filter = summary.types.get("UserFilter").unwrap();
    assert_eq!(filter.kind, GraphQLTypeKind::InputObject);
    assert_eq!(filter.input_fields.len(), 1);
    assert_eq!(filter.input_fields[0].name, "nameContains");
}

#[test]
fn test_arg_type_lookup() {
    let summary = parse_introspection(&sample_introspection_body()).unwrap();
    assert_eq!(summary.arg_type("Query", "user", "role"), Some("Role"));
    assert_eq!(summary.arg_type("Query", "user", "nope"), None);
}

#[test]
fn test_invalid_bodies_fail_with_message() {
    assert!(parse_introspection(b"not json").is_err());
    assert!(parse_introspection(b"{\"errors\": []}").is_err());
    assert!(parse_introspection(b"{\"data\": {}}").is_err());
}
