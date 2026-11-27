use super::*;
use crate::models::{HttpMethod, KeyValuePair, MultipartItem, RequestBody, RequestItem};

#[test]
fn test_parse_query_from_url() {
    let url = "https://api.example.com/search?q=rust+lang&page=1&empty=&flag";
    let (base, params) = parse_query_from_url(url);
    assert_eq!(base, "https://api.example.com/search");
    assert_eq!(params.len(), 4);
    assert_eq!(params[0].key, "q");
    assert_eq!(params[0].value, "rust lang");
    assert_eq!(params[1].key, "page");
    assert_eq!(params[1].value, "1");
    assert_eq!(params[2].key, "empty");
    assert_eq!(params[2].value, "");
    assert_eq!(params[3].key, "flag");
    assert_eq!(params[3].value, "");
}

#[test]
fn test_build_url_with_query() {
    let base = "https://api.example.com/items";
    let params = vec![
        KeyValuePair::new("type", "tools"),
        KeyValuePair {
            enabled: false,
            key: "disabled".to_string(),
            value: "val".to_string(),
            description: None,
        },
        KeyValuePair::new("limit", "25"),
    ];
    let built = build_url_with_query(base, &params);
    assert_eq!(built, "https://api.example.com/items?type=tools&limit=25");
}

#[test]
fn test_resolved_request_multipart() {
    let mut item = RequestItem::new(
        "ws-1",
        "Test Multipart",
        HttpMethod::Post,
        "http://localhost:8080/upload",
    );
    item.body = RequestBody::Multipart {
        items: vec![
            MultipartItem::new_text("description", "A test file upload"),
            MultipartItem::new_file("file", "C:/dummy/path.png"),
        ],
    };

    let resolved = ResolvedRequest::from_item(&item, None).unwrap();
    assert!(resolved.multipart.is_some());
    let parts = resolved.multipart.unwrap();
    assert_eq!(parts.len(), 2);
    assert!(!parts[0].is_file);
    assert_eq!(parts[0].key, "description");
    assert!(parts[1].is_file);
    assert_eq!(parts[1].key, "file");
    assert!(resolved.sent_info.body_preview.is_some());
}

#[test]
fn test_resolved_request_no_duplicate_query() {
    // Even if item.url has query params and item.params also has them, they shouldn't duplicate
    let mut item = RequestItem::new(
        "ws-1",
        "Test Dedupe",
        HttpMethod::Get,
        "https://api.example.com/search?q=rust",
    );
    item.params = vec![
        KeyValuePair::new("q", "rust"),
        KeyValuePair::new("page", "2"),
    ];

    let resolved = ResolvedRequest::from_item(&item, None).unwrap();
    assert_eq!(resolved.url, "https://api.example.com/search?q=rust&page=2");

    // When item.url is clean base and params are defined
    let mut item_clean = RequestItem::new(
        "ws-1",
        "Test Clean",
        HttpMethod::Get,
        "https://api.example.com/search",
    );
    item_clean.params = vec![KeyValuePair::new("q", "rust")];
    let resolved_clean = ResolvedRequest::from_item(&item_clean, None).unwrap();
    assert_eq!(resolved_clean.url, "https://api.example.com/search?q=rust");
}

#[test]
fn jsonc_request_sends_json_and_preserves_the_source() {
    use crate::models::{Environment, EnvironmentVariable};

    let mut item = RequestItem::new("ws-1", "JSONC", HttpMethod::Post, "https://example.com");
    let source = "{\n// {{name}}\n\"name\": \"{{name}}\",\n\"id\": {{id}}, /* omitted */\n}";
    item.set_body_json(source.to_owned());
    let mut env = Environment::new("ws-1", "Test");
    env.variables.extend([
        EnvironmentVariable {
            key: "name".into(),
            current_value: "https://example.com/ñ".into(),
            ..Default::default()
        },
        EnvironmentVariable {
            key: "id".into(),
            current_value: "42".into(),
            ..Default::default()
        },
    ]);

    let resolved = ResolvedRequest::from_item(&item, Some(&env)).unwrap();
    let body = resolved.body.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"name": "https://example.com/ñ", "id": 42})
    );
    assert!(resolved
        .headers
        .contains(&("Content-Type".into(), "application/json".into())));
    assert_eq!(
        resolved.sent_info.body_preview.as_deref(),
        std::str::from_utf8(&body).ok()
    );
    let curl = crate::curl::request_to_curl(&item, Some(&env));
    assert!(curl.contains(std::str::from_utf8(&body).unwrap()));
    assert_eq!(
        item.body,
        RequestBody::Json {
            content: source.into()
        }
    );
}
