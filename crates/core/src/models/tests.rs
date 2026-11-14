use super::*;

#[test]
fn test_status_code_to_reason() {
    assert_eq!(status_code_to_reason(200), "OK");
    assert_eq!(status_code_to_reason(201), "Created");
    assert_eq!(status_code_to_reason(404), "Not Found");
    assert_eq!(status_code_to_reason(500), "Internal Server Error");
    assert_eq!(status_code_to_reason(999), "");
}

#[test]
fn test_http_request_item_mutations() {
    let mut req = RequestItem::new("ws1", "Untitled Request", HttpMethod::Get, "");
    assert_eq!(req.name, "Untitled Request");
    assert_eq!(req.method, HttpMethod::Get);

    // URL & query params parsing
    let (has_query, has_template) = req.apply_url("https://api.example.com/items?q=rust&limit=20");
    assert!(has_query);
    assert!(!has_template);
    assert_eq!(req.url, "https://api.example.com/items");
    assert_eq!(req.params.len(), 2);
    assert_eq!(req.params[0].key, "q");
    assert_eq!(req.params[0].value, "rust");

    // Method & name
    req.set_method(HttpMethod::Post);
    assert_eq!(req.method, HttpMethod::Post);
    req.set_name("Get Items".to_string());
    assert_eq!(req.name, "Get Items");

    // Params
    req.update_param(1, "limit".to_string(), "50".to_string(), false);
    assert_eq!(req.params[1].value, "50");
    assert!(!req.params[1].enabled);
    req.add_param();
    assert_eq!(req.params.len(), 3);
    req.remove_param(2);
    assert_eq!(req.params.len(), 2);

    // Headers
    req.add_header();
    req.update_header(
        0,
        "Accept".to_string(),
        "application/json".to_string(),
        true,
    );
    assert_eq!(req.headers[0].key, "Accept");
    assert_eq!(req.headers[0].value, "application/json");
    req.remove_header(0);
    assert!(req.headers.is_empty());

    // Body
    req.set_body_json(r#"{"test": true}"#.to_string());
    assert_eq!(req.body, RequestBody::json(r#"{"test": true}"#));

    req.set_body_type(RequestBody::form(vec![KeyValuePair::new("k", "v")]));
    req.update_form_param(0, "k2".to_string(), "v2".to_string(), true);
    req.add_form_param();
    assert_eq!(
        req.body,
        RequestBody::form(vec![
            KeyValuePair::new("k2", "v2"),
            KeyValuePair::new("", "")
        ])
    );
    req.remove_form_param(1);

    req.set_body_type(RequestBody::multipart(vec![MultipartItem::default()]));
    req.update_multipart_item(0, "file".to_string(), "data".to_string(), false, None, true);
    req.add_multipart_item();
    req.remove_multipart_item(1);

    req.set_raw_body("hello".to_string(), "text/plain".to_string());
    assert_eq!(
        req.body,
        RequestBody::Raw {
            content: "hello".to_string(),
            content_type: "text/plain".to_string()
        }
    );

    req.set_binary_file("/tmp/file.bin".to_string());
    assert_eq!(
        req.body,
        RequestBody::Binary {
            file_path: "/tmp/file.bin".to_string()
        }
    );

    // Auth
    req.set_bearer_token("tok123".to_string());
    assert_eq!(
        req.auth,
        Auth::Bearer {
            token: "tok123".to_string()
        }
    );
    req.set_basic_auth("u".to_string(), "p".to_string());
    assert_eq!(
        req.auth,
        Auth::Basic {
            username: "u".to_string(),
            password: "p".to_string()
        }
    );
    req.set_api_key(
        "x-api-key".to_string(),
        "secret".to_string(),
        ApiKeyLocation::Header,
    );
    assert_eq!(
        req.auth,
        Auth::ApiKey {
            key: "x-api-key".to_string(),
            value: "secret".to_string(),
            add_to: ApiKeyLocation::Header,
        }
    );

    // Settings
    req.set_timeout_secs(42);
    assert_eq!(req.settings.timeout_secs(), 42);
    req.set_follow_redirects(false);
    assert!(!req.settings.follow_redirects);
    req.set_validate_tls(false);
    assert!(!req.settings.validate_tls);

    // Scripts
    req.set_pre_script("console.log(1);".to_string());
    assert_eq!(
        req.script.as_ref().unwrap().pre_request.as_deref(),
        Some("console.log(1);")
    );
    req.set_test_script("z.test('ok');".to_string());
    assert_eq!(
        req.script.as_ref().unwrap().post_response.as_deref(),
        Some("z.test('ok');")
    );

    // Content-Type update
    req.update_content_type_header(Some("application/json"));
    assert_eq!(req.headers.len(), 1);
    assert_eq!(req.headers[0].key, "Content-Type");
    assert_eq!(req.headers[0].value, "application/json");

    // Overwrite existing header case-insensitively
    req.headers[0].key = "content-type".to_string();
    req.update_content_type_header(Some("multipart/form-data"));
    assert_eq!(req.headers.len(), 1);
    assert_eq!(req.headers[0].value, "multipart/form-data");

    // Clear header
    req.update_content_type_header(None);
    assert!(req.headers.is_empty());
}

#[test]
fn test_auth_apikey_yaml_serialization() {
    let auth_header = Auth::ApiKey {
        key: "X-API-Key".to_string(),
        value: "secret123".to_string(),
        add_to: ApiKeyLocation::Header,
    };
    let yaml_str = serde_yaml::to_string(&auth_header).expect("serialize yaml");
    assert!(yaml_str.contains("add_to: header"));
    let deserialized: Auth = serde_yaml::from_str(&yaml_str).expect("deserialize yaml");
    assert_eq!(auth_header, deserialized);

    let auth_query = Auth::ApiKey {
        key: "api_key".to_string(),
        value: "secret456".to_string(),
        add_to: ApiKeyLocation::Query,
    };
    let yaml_query_str = serde_yaml::to_string(&auth_query).expect("serialize yaml");
    assert!(yaml_query_str.contains("add_to: query"));
    let deserialized_query: Auth = serde_yaml::from_str(&yaml_query_str).expect("deserialize yaml");
    assert_eq!(auth_query, deserialized_query);
}
