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

    // Auth contributed headers preview
    let mask = |s: &str| format!("[{s}]");
    assert_eq!(
        Auth::Bearer {
            token: "secret".into()
        }
        .contributed_header_preview(mask),
        Some(("Authorization".into(), "Bearer [secret]".into()))
    );
    assert_eq!(
        Auth::Basic {
            username: "admin".into(),
            password: "pw".into()
        }
        .contributed_header_preview(mask),
        Some(("Authorization".into(), "Basic admin:[pw]".into()))
    );
    assert_eq!(
        Auth::ApiKey {
            key: "X-Key".into(),
            value: "val".into(),
            add_to: ApiKeyLocation::Header
        }
        .contributed_header_preview(mask),
        Some(("X-Key".into(), "[val]".into()))
    );
    assert_eq!(
        Auth::ApiKey {
            key: "k".into(),
            value: "v".into(),
            add_to: ApiKeyLocation::Query
        }
        .contributed_header_preview(mask),
        None
    );
    assert_eq!(Auth::None.contributed_header_preview(mask), None);
    assert_eq!(Auth::Inherit.contributed_header_preview(mask), None);

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

#[test]
fn test_ws_message_format_yaml_contract() {
    use crate::storage::models::RequestFile;

    // Text is the default and is omitted from the YAML.
    let mut req = RequestItem::new("ws1", "Echo", HttpMethod::Get, "wss://example/ws");
    assert_eq!(req.ws_message_format, WsMessageFormat::Text);

    let file = RequestFile::from_item(&req);
    let yaml = serde_yaml::to_string(&file).expect("serialize yaml");
    assert!(!yaml.contains("ws_message_format"));

    // Old request files without the key deserialize as Text.
    let legacy: RequestFile =
        serde_yaml::from_str("name: Echo\nmethod: GET\nurl: wss://example/ws")
            .expect("deserialize legacy yaml");
    assert_eq!(legacy.ws_message_format, WsMessageFormat::Text);

    // JSON is persisted and round-trips.
    req.set_ws_message_format(WsMessageFormat::Json);
    assert!(req.ws_message_format.is_json());
    let file = RequestFile::from_item(&req);
    let yaml = serde_yaml::to_string(&file).expect("serialize yaml");
    assert!(yaml.contains("ws_message_format: json"));
    let round_tripped: RequestFile = serde_yaml::from_str(&yaml).expect("deserialize yaml");
    assert_eq!(round_tripped.ws_message_format, WsMessageFormat::Json);
    let item = round_tripped.into_item("req-1", "ws1", None);
    assert_eq!(item.ws_message_format, WsMessageFormat::Json);
}

#[test]
fn test_parse_content_disposition_filename() {
    // Simple unquoted & quoted
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=report.pdf"),
        Some("report.pdf".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename("inline; filename=\"my report.pdf\""),
        Some("my report.pdf".to_string())
    );

    // Semicolon and escaped quote inside quoted string
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"a;b=c.csv\"; size=42"),
        Some("a;b=c.csv".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename(r#"attachment; filename="say \"hi\".txt""#),
        Some(r#"say "hi".txt"#.to_string())
    );

    // RFC 5987 / RFC 8187 filename* (UTF-8, ISO-8859-1, literal '+')
    assert_eq!(
        parse_content_disposition_filename("attachment; filename*=UTF-8''%E2%82%AC%20rates.pdf"),
        Some("€ rates.pdf".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename(
            "attachment; filename*=utf-8'vi'b%C3%A1o%20c%C3%A1o.pdf"
        ),
        Some("báo cáo.pdf".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename("attachment; filename*=UTF-8''a+b%20c.txt"),
        Some("a+b c.txt".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename("attachment; filename*=ISO-8859-1''%A3%20rates.pdf"),
        Some("£ rates.pdf".to_string())
    );

    // Precedence: filename* overrides filename regardless of order
    assert_eq!(
        parse_content_disposition_filename(
            "attachment; filename=\"fallback.pdf\"; filename*=UTF-8''preferred.pdf"
        ),
        Some("preferred.pdf".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename(
            "attachment; FILENAME*=UTF-8''preferred.pdf; FILENAME=\"fallback.pdf\""
        ),
        Some("preferred.pdf".to_string())
    );

    // Path traversal & directory stripping
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"../../etc/passwd\""),
        Some("passwd".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename(r#"attachment; filename="C:\Users\Alice\invoice.pdf""#),
        Some("invoice.pdf".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename(
            r#"attachment; filename="C:\\Users\\Alice\\invoice.pdf""#
        ),
        Some("invoice.pdf".to_string())
    );

    // Missing or invalid filenames
    assert_eq!(parse_content_disposition_filename("attachment"), None);
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"\""),
        None
    );
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"..\""),
        None
    );
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\".\""),
        None
    );
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"   \""),
        None
    );
}

#[test]
fn test_http_response_record_suggested_filename() {
    let mut resp = HttpResponseRecord::default();
    assert_eq!(resp.content_disposition_filename(), None);
    assert_eq!(resp.suggested_filename(), "response.json");

    resp.headers.push((
        "Content-Disposition".to_string(),
        "attachment; filename=\"export.xlsx\"".to_string(),
    ));
    assert_eq!(
        resp.content_disposition_filename(),
        Some("export.xlsx".to_string())
    );
    assert_eq!(resp.suggested_filename(), "export.xlsx");

    // Case-insensitive header name
    let resp_lower = HttpResponseRecord {
        headers: vec![(
            "content-disposition".to_string(),
            "attachment; filename*=UTF-8''data%20file.csv".to_string(),
        )],
        ..Default::default()
    };
    assert_eq!(resp_lower.suggested_filename(), "data file.csv");

    // Falls back to URL endpoint when Content-Disposition is absent
    let resp_from_url = HttpResponseRecord {
        request_sent: Some(RequestSentInfo {
            url: "https://cdn.prod.website-files.com/6a020fca21245d64af2c19d8/6a020fca21245d64af2c392a_6720dff9eada99162c95b2d5_6720d2aac712d878f3a3eb97_2%252520-%25252029.10-min.jpeg".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        resp_from_url.suggested_filename(),
        "6a020fca21245d64af2c392a_6720dff9eada99162c95b2d5_6720d2aac712d878f3a3eb97_2%2520-%252029.10-min.jpeg"
    );

    // Content-Disposition takes precedence over URL endpoint
    let resp_both = HttpResponseRecord {
        headers: vec![(
            "Content-Disposition".to_string(),
            "attachment; filename=\"override.png\"".to_string(),
        )],
        request_sent: Some(RequestSentInfo {
            url: "https://example.com/files/original.jpeg".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(resp_both.suggested_filename(), "override.png");

    // Root URL without path segment falls back to response.json
    let resp_root = HttpResponseRecord {
        request_sent: Some(RequestSentInfo {
            url: "https://example.com/".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(resp_root.suggested_filename(), "response.json");
}

#[test]
fn test_extract_filename_from_url() {
    assert_eq!(
        extract_filename_from_url("https://cdn.prod.website-files.com/6a020fca21245d64af2c19d8/6a020fca21245d64af2c392a_6720dff9eada99162c95b2d5_6720d2aac712d878f3a3eb97_2%252520-%25252029.10-min.jpeg"),
        Some("6a020fca21245d64af2c392a_6720dff9eada99162c95b2d5_6720d2aac712d878f3a3eb97_2%2520-%252029.10-min.jpeg".to_string())
    );
    assert_eq!(
        extract_filename_from_url(
            "https://api.example.com/v1/files/report%202026.pdf?download=1#page=2"
        ),
        Some("report 2026.pdf".to_string())
    );
    assert_eq!(
        extract_filename_from_url("https://api.example.com/v1/users/"),
        Some("users".to_string())
    );
    assert_eq!(extract_filename_from_url("https://example.com/"), None);
    assert_eq!(extract_filename_from_url("https://example.com"), None);
    assert_eq!(extract_filename_from_url("not-a-valid-url"), None);
    assert_eq!(
        extract_filename_from_url("https://example.com/files/..%2F..%2Fetc%2Fpasswd"),
        Some("passwd".to_string())
    );
    assert_eq!(
        extract_filename_from_url("https://example.com/files/%2E%2E"),
        None
    );
}
