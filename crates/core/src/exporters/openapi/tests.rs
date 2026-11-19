//! Tests for the OpenAPI 3.0 exporter, including a roundtrip through the
//! OpenAPI importer (`crate::importers::openapi::parse_openapi`).

use super::*;
use crate::importers::models::ImportedRequest;
use crate::models::{
    ApiKeyLocation, Auth, HttpMethod, KeyValuePair, MultipartItem, Protocol, RequestBody,
};
use crate::storage::models::RequestExampleFile;

/// Minimal HTTP request fixture.
fn req(name: &str, method: HttpMethod, url: &str) -> ImportedRequest {
    ImportedRequest {
        name: name.into(),
        method,
        url: url.into(),
        protocol: Protocol::Http,
        params: vec![],
        headers: vec![],
        auth: Auth::None,
        body: RequestBody::None,
        script: None,
        examples: vec![],
        folder_path: vec![],
        description: None,
    }
}

fn with(mut r: ImportedRequest, f: impl FnOnce(&mut ImportedRequest)) -> ImportedRequest {
    f(&mut r);
    r
}

#[test]
fn basic_document_structure() {
    let export = requests_to_openapi(
        "My API",
        Some("Test collection"),
        &[req(
            "List Users",
            HttpMethod::Get,
            "https://api.example.com/users",
        )],
    );
    let doc = &export.document;
    assert!(export.warnings.is_empty());
    assert_eq!(doc["openapi"], "3.0.3");
    assert_eq!(doc["info"]["title"], "My API");
    assert_eq!(doc["info"]["version"], "1.0.0");
    assert_eq!(doc["info"]["description"], "Test collection");
    assert_eq!(doc["servers"][0]["url"], "https://api.example.com");
    assert_eq!(doc["paths"]["/users"]["get"]["summary"], "List Users");
    assert_eq!(doc["paths"]["/users"]["get"]["operationId"], "list-users");
}

#[test]
fn path_templates_and_parameters() {
    let r = with(
        req(
            "Get User Posts",
            HttpMethod::Get,
            "https://api.example.com/users/{{userId}}/posts?limit=5",
        ),
        |r| {
            r.params.push(KeyValuePair::new("page", "2"));
            r.params.push(KeyValuePair::new("page", "2")); // duplicate → deduped
            r.params[1].enabled = false; // disabled → dropped
        },
    );
    let export = requests_to_openapi("T", None, &[r]);
    let doc = &export.document;

    let path = &doc["paths"]["/users/{userId}/posts"]["get"];
    assert_eq!(path["summary"], "Get User Posts");

    let params = path["parameters"].as_array().unwrap();
    let path_param = params
        .iter()
        .find(|p| p["name"] == "userId" && p["in"] == "path")
        .expect("path param userId declared");
    assert_eq!(path_param["required"], true);

    let query_names: Vec<&str> = params
        .iter()
        .filter(|p| p["in"] == "query")
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(query_names, vec!["page", "limit"]);
    let limit = params.iter().find(|p| p["name"] == "limit").unwrap();
    assert_eq!(limit["example"], "5");
}

#[test]
fn header_parameters_skip_content_type() {
    let r = with(
        req(
            "Create User",
            HttpMethod::Post,
            "https://api.example.com/users",
        ),
        |r| {
            r.headers
                .push(KeyValuePair::new("Content-Type", "application/json"));
            r.headers.push(KeyValuePair::new("X-Request-Id", "abc-123"));
            r.headers.push(KeyValuePair::new("X-Disabled", "nope"));
            r.headers[2].enabled = false;
        },
    );
    let export = requests_to_openapi("T", None, &[r]);
    let params = export.document["paths"]["/users"]["post"]["parameters"]
        .as_array()
        .unwrap();
    let names: Vec<&str> = params.iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["X-Request-Id"]);
}

#[test]
fn auth_maps_to_security_schemes_without_secrets() {
    let requests = vec![
        with(req("Bearer Op", HttpMethod::Get, "https://a.test/x"), |r| {
            r.auth = Auth::Bearer {
                token: "super-secret".into(),
            };
        }),
        with(req("Basic Op", HttpMethod::Get, "https://a.test/y"), |r| {
            r.auth = Auth::Basic {
                username: "u".into(),
                password: "p".into(),
            };
        }),
        with(
            req("Key Header Op", HttpMethod::Get, "https://a.test/z"),
            |r| {
                r.auth = Auth::ApiKey {
                    key: "X-Api-Key".into(),
                    value: "key-secret".into(),
                    add_to: ApiKeyLocation::Header,
                };
            },
        ),
        with(
            req("Key Query Op", HttpMethod::Get, "https://a.test/w"),
            |r| {
                r.auth = Auth::ApiKey {
                    key: "api_key".into(),
                    value: "key-secret".into(),
                    add_to: ApiKeyLocation::Query,
                };
            },
        ),
    ];
    let export = requests_to_openapi("T", None, &requests);
    let doc = &export.document;
    let text = doc.to_string();

    // Secrets must never be serialized.
    assert!(!text.contains("super-secret"));
    assert!(!text.contains("key-secret"));
    assert!(!text.contains("\"password\""));

    let schemes = &doc["components"]["securitySchemes"];
    assert_eq!(schemes["bearerAuth"]["scheme"], "bearer");
    assert_eq!(schemes["basicAuth"]["scheme"], "basic");
    assert_eq!(schemes["apiKey-header-x-api-key"]["in"], "header");
    assert_eq!(schemes["apiKey-header-x-api-key"]["name"], "X-Api-Key");
    assert_eq!(schemes["apiKey-query-api-key"]["in"], "query");

    assert_eq!(
        doc["paths"]["/x"]["get"]["security"][0]["bearerAuth"],
        json!([])
    );
    assert_eq!(
        doc["paths"]["/y"]["get"]["security"][0]["basicAuth"],
        json!([])
    );
    // No auth on a plain request.
    let r = req("Plain", HttpMethod::Get, "https://a.test/plain");
    let export = requests_to_openapi("T", None, &[r]);
    assert!(export.document["paths"]["/plain"]["get"]["security"].is_null());
}

#[test]
fn body_variants() {
    // Valid JSON → parsed example.
    let json_req = with(req("Json Op", HttpMethod::Post, "https://a.test/j"), |r| {
        r.body = RequestBody::json(r#"{"name":"Zed","age":3}"#);
    });
    // Templated JSON → substituted example still parses.
    let templated = with(
        req("Templated Op", HttpMethod::Post, "https://a.test/t"),
        |r| {
            r.body =
                RequestBody::json(r#"{"id": {{user_id}}, "name": "{{name}}", "fake": {{$uuid}}}"#);
        },
    );
    // Invalid JSON → raw text example.
    let raw_json = with(
        req("Broken Op", HttpMethod::Post, "https://a.test/b"),
        |r| {
            r.body = RequestBody::json("{not json");
        },
    );
    let form = with(req("Form Op", HttpMethod::Post, "https://a.test/f"), |r| {
        r.body = RequestBody::form(vec![KeyValuePair::new("user", "zed")]);
    });
    let multipart = with(
        req("Upload Op", HttpMethod::Post, "https://a.test/m"),
        |r| {
            r.body = RequestBody::multipart(vec![
                MultipartItem::new_file("avatar", "/tmp/avatar.png"),
                MultipartItem::new_text("note", "hi"),
            ]);
        },
    );
    let raw = with(req("Raw Op", HttpMethod::Post, "https://a.test/r"), |r| {
        r.body = RequestBody::raw("hello", "text/csv");
    });
    let binary = with(req("Bin Op", HttpMethod::Post, "https://a.test/bin"), |r| {
        r.body = RequestBody::binary("/tmp/blob");
    });
    let export = requests_to_openapi(
        "T",
        None,
        &[json_req, templated, raw_json, form, multipart, raw, binary],
    );
    let doc = &export.document;

    let j = &doc["paths"]["/j"]["post"]["requestBody"]["content"]["application/json"];
    assert_eq!(j["schema"]["type"], "object");
    assert_eq!(j["example"]["name"], "Zed");

    let t = &doc["paths"]["/t"]["post"]["requestBody"]["content"]["application/json"];
    assert_eq!(t["example"]["id"], "user_id");
    assert_eq!(t["example"]["name"], "name");
    assert_eq!(t["example"]["fake"], "uuid");

    let b = &doc["paths"]["/b"]["post"]["requestBody"]["content"]["application/json"];
    assert_eq!(b["schema"]["type"], "string");
    assert_eq!(b["example"], "{not json");

    let f =
        &doc["paths"]["/f"]["post"]["requestBody"]["content"]["application/x-www-form-urlencoded"];
    assert_eq!(f["schema"]["properties"]["user"]["example"], "zed");

    let m = &doc["paths"]["/m"]["post"]["requestBody"]["content"]["multipart/form-data"];
    assert_eq!(m["schema"]["properties"]["avatar"]["format"], "binary");
    assert_eq!(m["schema"]["properties"]["note"]["example"], "hi");

    let r = &doc["paths"]["/r"]["post"]["requestBody"]["content"]["text/csv"];
    assert_eq!(r["example"], "hello");

    let bin = &doc["paths"]["/bin"]["post"]["requestBody"]["content"]["application/octet-stream"];
    assert_eq!(bin["schema"]["format"], "binary");
}

#[test]
fn examples_become_responses() {
    let r = with(
        req("Delete User", HttpMethod::Delete, "https://a.test/users/1"),
        |r| {
            r.examples.push(RequestExampleFile {
                name: "Not found".into(),
                status: 404,
                headers: vec![KeyValuePair::new("Content-Type", "application/json")],
                body: Some(r#"{"error":"missing"}"#.into()),
            });
            r.examples.push(RequestExampleFile {
                name: "Server error".into(),
                status: 500,
                headers: vec![],
                body: Some("oops".into()),
            });
        },
    );
    let export = requests_to_openapi("T", None, &[r]);
    let responses = &export.document["paths"]["/users/1"]["delete"]["responses"];
    assert_eq!(
        responses["404"]["content"]["application/json"]["example"]["error"],
        "missing"
    );
    assert_eq!(responses["500"]["content"]["text/plain"]["example"], "oops");
    assert!(responses["200"].is_null());
}

#[test]
fn baseline_response_without_examples() {
    let export = requests_to_openapi(
        "T",
        None,
        &[req("Ping", HttpMethod::Get, "https://a.test/ping")],
    );
    let responses = &export.document["paths"]["/ping"]["get"]["responses"];
    assert_eq!(responses["200"]["description"], "Successful response");
}

#[test]
fn non_http_protocols_are_skipped_with_warnings() {
    let mut ws = req("Chat", HttpMethod::Get, "wss://echo.example");
    ws.protocol = Protocol::WebSocket;
    let mut grpc = req("SayHello", HttpMethod::Post, "grpc://svc");
    grpc.protocol = Protocol::Grpc;
    let export = requests_to_openapi(
        "T",
        None,
        &[ws, grpc, req("Plain", HttpMethod::Get, "https://a.test/p")],
    );
    assert_eq!(export.warnings.len(), 2);
    assert!(export.warnings[0].contains("Chat") && export.warnings[0].contains("WS"));
    assert!(export.warnings[1].contains("SayHello") && export.warnings[1].contains("gRPC"));
    let paths = export.document["paths"].as_object().unwrap();
    assert_eq!(paths.len(), 1);
    assert!(paths.contains_key("/p"));
}

#[test]
fn duplicate_operations_are_warned_and_dropped() {
    let a = req("List Users", HttpMethod::Get, "https://a.test/users");
    let b = req("List Users V2", HttpMethod::Get, "https://a.test/users");
    let export = requests_to_openapi("T", None, &[a, b]);
    assert_eq!(export.warnings.len(), 1);
    assert!(export.warnings[0].contains("duplicate operation"));
    let path = export.document["paths"]["/users"].as_object().unwrap();
    assert_eq!(path.len(), 1);
    assert_eq!(path["get"]["summary"], "List Users");
}

#[test]
fn graphql_and_sse_requests() {
    // GraphQL arrives pre-flattened to its HTTP transport shape (POST + JSON
    // body), mirroring `RequestItem::to_graphql_http_item`.
    let mut gql = req("Login", HttpMethod::Post, "https://api.test/graphql");
    gql.protocol = Protocol::GraphQL;
    gql.body = RequestBody::json(
        r#"{"query":"query { me { id } }","variables":{"x":1},"operationName":"Me"}"#,
    );
    let mut sse = req("Stream Events", HttpMethod::Get, "https://api.test/events");
    sse.protocol = Protocol::Sse;
    let export = requests_to_openapi("T", None, &[gql, sse]);
    let doc = &export.document;

    let gql_op = &doc["paths"]["/graphql"]["post"];
    assert_eq!(
        gql_op["requestBody"]["content"]["application/json"]["example"]["query"],
        "query { me { id } }"
    );
    assert_eq!(
        gql_op["requestBody"]["content"]["application/json"]["example"]["operationName"],
        "Me"
    );

    let sse_params = doc["paths"]["/events"]["get"]["parameters"]
        .as_array()
        .unwrap();
    let accept = sse_params.iter().find(|p| p["name"] == "Accept").unwrap();
    assert_eq!(accept["example"], "text/event-stream");
}

#[test]
fn unparseable_url_falls_back_to_raw_path_key() {
    let export = requests_to_openapi(
        "T",
        None,
        &[req("List", HttpMethod::Get, "{{base_url}}/users")],
    );
    let paths = export.document["paths"].as_object().unwrap();
    assert!(paths.contains_key("{base_url}/users"), "paths: {:?}", paths);
    // No servers when no URL parses.
    assert!(export.document["servers"].is_null());
    // The templated token is declared as a path parameter for spec validity.
    let params = paths["{base_url}/users"]["get"]["parameters"]
        .as_array()
        .unwrap();
    assert_eq!(params[0]["name"], "base_url");
    assert_eq!(params[0]["in"], "path");
}

#[test]
fn tags_from_folder_path() {
    let r = with(
        req("Login", HttpMethod::Post, "https://a.test/login"),
        |r| {
            r.folder_path = vec!["Users".into(), "Auth".into()];
        },
    );
    let export = requests_to_openapi("T", None, &[r]);
    assert_eq!(export.document["tags"][0]["name"], "Users");
    assert_eq!(export.document["tags"][1]["name"], "Auth");
    assert_eq!(
        export.document["paths"]["/login"]["post"]["tags"][0],
        "Users"
    );
}

#[test]
fn serialize_yaml_and_json() {
    let export = requests_to_openapi(
        "My API",
        None,
        &[req("List", HttpMethod::Get, "https://a.test/items")],
    );

    let yaml = serialize_openapi(&export, OpenApiFileFormat::Yaml).unwrap();
    // serde_json::Map is a BTreeMap, so top-level keys serialize in sorted
    // order (spec-valid; deterministic output).
    assert!(yaml.contains("openapi: 3.0.3"));
    assert!(yaml.contains("title: My API"));

    let json = serialize_openapi(&export, OpenApiFileFormat::Json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["paths"]["/items"]["get"]["summary"], "List");

    assert_eq!(OpenApiFileFormat::Yaml.extension(), "yaml");
    assert_eq!(OpenApiFileFormat::Json.extension(), "json");
    assert_eq!(
        OpenApiFileFormat::from_extension("JSON"),
        OpenApiFileFormat::Json
    );
    assert_eq!(
        OpenApiFileFormat::from_extension("yaml"),
        OpenApiFileFormat::Yaml
    );
    assert_eq!(
        OpenApiFileFormat::from_extension("txt"),
        OpenApiFileFormat::Yaml
    );
}

/// Export → import roundtrip: names, methods, URLs, folder paths and examples
/// survive the cycle.
#[test]
fn roundtrip_through_importer() {
    let login = with(
        req("Login", HttpMethod::Post, "https://api.test/auth/login"),
        |r| {
            r.folder_path = vec!["Auth".into()];
            r.headers.push(KeyValuePair::new("X-Client", "zest"));
            r.body = RequestBody::json(r#"{"user":"a","pass":"b"}"#);
            r.auth = Auth::Bearer {
                token: "secret".into(),
            };
        },
    );
    let mut gql = req("Me", HttpMethod::Post, "https://api.test/graphql");
    gql.protocol = Protocol::GraphQL;
    gql.folder_path = vec!["Users".into()];
    gql.body = RequestBody::json(r#"{"query":"query { me { id } }"}"#);
    let mut del = req(
        "Delete User",
        HttpMethod::Delete,
        "https://api.test/users/{{userId}}",
    );
    del.folder_path = vec!["Users".into()];
    del.examples.push(RequestExampleFile {
        name: "Not found".into(),
        status: 404,
        headers: vec![KeyValuePair::new("Content-Type", "application/json")],
        body: Some(r#"{"error":"missing"}"#.into()),
    });
    let mut ws = req("Chat", HttpMethod::Get, "wss://x");
    ws.protocol = Protocol::WebSocket;

    let export = requests_to_openapi("Roundtrip API", None, &[login, gql, del, ws]);
    assert_eq!(export.warnings.len(), 1); // WS skipped

    let yaml = serialize_openapi(&export, OpenApiFileFormat::Yaml).unwrap();
    let imported =
        crate::importers::openapi::parse_openapi(&yaml).expect("re-import exported YAML");

    assert_eq!(imported.title, "Roundtrip API");
    assert_eq!(imported.requests.len(), 3, "WS must not re-import");

    let by_name = |name: &str| {
        imported
            .requests
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("missing request {name}"))
    };
    let login = by_name("Login");
    assert_eq!(login.method, HttpMethod::Post);
    // The importer replaces the first server with a `{{baseUrl}}` environment
    // variable — the roundtrip keeps the path verbatim.
    assert_eq!(login.url, "{{baseUrl}}/auth/login");
    assert_eq!(login.folder_path, vec!["Auth".to_string()]);
    assert!(matches!(login.auth, Auth::Bearer { .. }));
    assert!(matches!(login.body, RequestBody::Json { .. }));
    let env = imported
        .environments
        .first()
        .expect("baseUrl environment extracted");
    let base = env
        .variables
        .iter()
        .find(|v| v.key == "baseUrl")
        .expect("baseUrl variable");
    assert_eq!(base.initial_value, "https://api.test");

    let gql = by_name("Me");
    // The exporter flattens GraphQL into its HTTP transport shape (POST +
    // JSON body), which is exactly how Zest executes it.
    assert_eq!(gql.url, "{{baseUrl}}/graphql");
    assert!(matches!(gql.body, RequestBody::Json { .. }));

    let del = by_name("Delete User");
    assert_eq!(del.url, "{{baseUrl}}/users/{{userId}}");
    assert_eq!(del.folder_path, vec!["Users".to_string()]);
    assert!(del.examples.iter().any(|e| e.status == 404));

    // Folders reconstructed from tags.
    let folder_names: Vec<&str> = imported.folders.iter().map(|f| f.name.as_str()).collect();
    assert!(folder_names.contains(&"Auth"));
    assert!(folder_names.contains(&"Users"));
}

#[test]
fn empty_input_produces_valid_skeleton() {
    let export = requests_to_openapi("Empty", None, &[]);
    assert!(export.warnings.is_empty());
    assert_eq!(export.document["paths"].as_object().unwrap().len(), 0);
    assert!(export.document["servers"].is_null());
    assert!(export.document["components"].is_null());
    let yaml = serialize_openapi(&export, OpenApiFileFormat::Yaml).unwrap();
    assert!(yaml.contains("openapi: 3.0.3"));
}
