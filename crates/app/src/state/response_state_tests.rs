use super::*;
use zest_core::models::ConsoleLogSource;
use zest_core::models::Protocol;

#[test]
fn test_start_sending_clears_active_data() {
    let mut state = ResponseState::new();
    let rec = HttpResponseRecord {
        id: "resp-1".to_string(),
        request_id: "req-1".to_string(),
        status_code: 200,
        status_text: "OK".to_string(),
        duration_ms: 25,
        size_bytes: 10,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        cookies: vec![],
        body: b"{\"a\":1}".to_vec(),
        timing: zest_core::models::TimingBreakdown::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    };

    let arc_rec = Arc::new(rec);
    state.cache.insert("req-1".to_string(), arc_rec.clone());
    state.current_response = Some(arc_rec);
    state.show_timing_popover = true;
    state.active_example_name = Some("example-1".to_string());
    state.refresh_formatted_body();
    assert!(state.current_response.is_some());
    assert!(state.formatted_body.is_some());
    assert!(state.cache.contains_key(&"req-1".to_string()));

    state.start_sending_internal(Some("req-1"));

    assert!(state.current_response.is_none());
    assert!(state.formatted_body.is_none());
    assert!(!state.show_timing_popover);
    assert!(state.active_example_name.is_none());
    assert!(!state.cache.contains_key(&"req-1".to_string()));
}

#[test]
fn test_console_logs_filtering() {
    let mut state = ResponseState::new();
    state.test_results = vec![
        TestResult {
            name: "Status is 200".to_string(),
            passed: true,
            error_message: None,
            duration_ms: 2,
        },
        TestResult {
            name: "Has token".to_string(),
            passed: false,
            error_message: Some("Expected token".to_string()),
            duration_ms: 1,
        },
    ];
    state.console_logs = vec![
        ConsoleLogEntry {
            source: ConsoleLogSource::PreRequest,
            level: ConsoleLogLevel::Log,
            message: "Pre script log".to_string(),
            timestamp: "10:00:00".to_string(),
        },
        ConsoleLogEntry {
            source: ConsoleLogSource::PostResponse,
            level: ConsoleLogLevel::Warn,
            message: "Post script warn".to_string(),
            timestamp: "10:00:01".to_string(),
        },
        ConsoleLogEntry {
            source: ConsoleLogSource::PostResponse,
            level: ConsoleLogLevel::Error,
            message: "Post script error".to_string(),
            timestamp: "10:00:02".to_string(),
        },
    ];

    // Default filter: All
    state.console_filter = ConsoleFilter::All;
    assert_eq!(state.filtered_test_results().len(), 2);
    assert_eq!(state.filtered_console_logs().len(), 3);

    // Tests filter
    state.console_filter = ConsoleFilter::Tests;
    assert_eq!(state.filtered_test_results().len(), 2);
    assert_eq!(state.filtered_console_logs().len(), 0);

    // Logs filter
    state.console_filter = ConsoleFilter::Logs;
    assert_eq!(state.filtered_test_results().len(), 0);
    assert_eq!(state.filtered_console_logs().len(), 3);

    // Errors filter
    state.console_filter = ConsoleFilter::Errors;
    let err_tests = state.filtered_test_results();
    assert_eq!(err_tests.len(), 1);
    assert_eq!(err_tests[0].name, "Has token");
    let err_logs = state.filtered_console_logs();
    assert_eq!(err_logs.len(), 1);
    assert_eq!(err_logs[0].message, "Post script error");
}

#[test]
fn test_console_tab_kind() {
    let mut state = ResponseState::new();
    assert_eq!(state.active_tab, ResponseTabKind::Response);
    state.active_tab = ResponseTabKind::Console;
    assert_eq!(state.active_tab, ResponseTabKind::Console);
}

#[test]
fn test_large_response_body_formatting() {
    let mut state = ResponseState::new();

    // Create a 500-item JSON array
    let items: Vec<String> = (0..500)
        .map(|i| format!(r#"{{"id": {}, "title": "item {}"}}"#, i, i))
        .collect();
    let json_body = format!("[\n{}\n]", items.join(",\n"));

    let rec = HttpResponseRecord {
        id: "resp-1".to_string(),
        request_id: "req-1".to_string(),
        status_code: 200,
        status_text: "OK".to_string(),
        duration_ms: 25,
        size_bytes: json_body.len(),
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        cookies: vec![],
        body: json_body.into_bytes(),
        timing: zest_core::models::TimingBreakdown::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    };

    state.current_response = Some(Arc::new(rec));
    state.refresh_formatted_body();

    let fmt = state
        .formatted_body
        .as_ref()
        .expect("formatted body should be present");
    assert!(fmt.is_json);
    // Each item has 4 lines in pretty JSON (bracket, id, title, bracket), so > 1000 lines
    assert!(fmt.lines.len() >= 500);

    // Switch to Raw mode
    state.view_mode = ResponseViewMode::Raw;
    state.refresh_formatted_body();
    let fmt_raw = state.formatted_body.as_ref().unwrap();
    assert!(!fmt_raw.lines.is_empty());
}

#[test]
fn test_load_for_request_from_cache() {
    let temp = tempfile::tempdir().unwrap();
    let store = zest_core::storage::FsWorkspaceStore::new(temp.path());
    store.init_workspace("Test WS").unwrap();
    let (_, rel_path) = store
        .create_request(
            None,
            "Login",
            zest_core::models::HttpMethod::Post,
            "http://localhost:8000/login",
            Protocol::Http,
        )
        .unwrap();

    let loc_file = zest_core::storage::models::LocFile {
        sent_at: "2026-10-15T12:00:00Z".to_string(),
        request: zest_core::storage::models::LocRequestInfo {
            method: zest_core::models::HttpMethod::Post,
            url: "http://localhost:8000/login".to_string(),
            headers: vec![],
            body: Some(r#"{"user":"alice"}"#.to_string()),
        },
        response: zest_core::storage::models::LocResponseInfo {
            status: 200,
            duration_ms: 42,
            size: 15,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: Some(r#"{"token":"xyz"}"#.to_string()),
            truncated: false,
            binary: false,
        },
    };
    store.save_response_cache(&rel_path, &loc_file).unwrap();

    let mut state = ResponseState::new();
    assert!(state.current_response.is_none());

    state.load_for_request_internal(&rel_path, &store);
    assert!(state.current_response.is_some());
    let resp = state.current_response.as_ref().unwrap();
    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.status_text, "OK");
    assert_eq!(resp.duration_ms, 42);
    assert_eq!(String::from_utf8_lossy(&resp.body), r#"{"token":"xyz"}"#);
    assert!(state.formatted_body.is_some());
}

#[test]
fn test_xml_response_body_formatting() {
    let mut state = ResponseState::new();
    let record = HttpResponseRecord {
        id: "resp-xml".to_string(),
        request_id: "req-1".to_string(),
        status_code: 200,
        status_text: "OK".to_string(),
        headers: vec![("content-type".to_string(), "application/xml".to_string())],
        cookies: vec![],
        body: "<root><user><name>Bob</name></user></root>"
            .as_bytes()
            .to_vec(),
        duration_ms: 10,
        size_bytes: 42,
        timing: Default::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    };

    state.current_response = Some(Arc::new(record));
    state.view_mode = ResponseViewMode::Pretty;
    state.refresh_formatted_body();

    let fmt = state.formatted_body.expect("Formatted body should exist");
    assert!(fmt.is_xml, "Should detect XML response");
    assert!(fmt.display_text.contains("<root>\n"));
    assert!(fmt.display_text.contains("  <user>\n"));
}

#[test]
fn test_wrap_lines_initial_and_toggle() {
    let mut state = ResponseState::new();
    assert!(!state.wrap_lines, "wrap_lines should default to false");
    state.wrap_lines = true;
    assert!(state.wrap_lines);
}

#[test]
fn test_toolbar_icons_exist_in_all_assets() {
    use gpui_kit::assets::{AllAssets, IconName};
    use gpui_kit::AssetSource as _;

    let assets = AllAssets;
    let icons = [
        IconName::TextWrap.path(),
        IconName::Copy.path(),
        IconName::Download.path(),
        IconName::Save.path(),
    ];

    for icon_path in icons {
        let res = assets.load(icon_path.as_ref());
        assert!(
            res.is_ok() && res.unwrap().is_some(),
            "Asset {} must exist in AllAssets",
            icon_path
        );
    }
}

#[test]
fn test_view_example_preserves_http_status_without_name() {
    let mut state = ResponseState::new();
    let ex = zest_core::storage::models::RequestExampleFile {
        name: "Success 200".to_string(),
        status: 200,
        headers: vec![],
        body: Some(r#"{"message":"hello"}"#.to_string()),
    };

    state.view_example_internal(&ex);

    assert_eq!(state.active_example_name.as_deref(), Some("Success 200"));
    let resp = state
        .current_response
        .as_ref()
        .expect("Response should exist");
    assert_eq!(resp.status_code, 200);
    // Requirement 2: HTTP status is preserved like request, NOT containing example name
    assert_eq!(resp.status_text, "OK");
    assert_ne!(resp.status_text, "Example: Success 200");

    // Renaming example should update active_example_name, but status_text stays "OK"
    state.rename_example_internal("Success 200", "Renamed Success");
    assert_eq!(
        state.active_example_name.as_deref(),
        Some("Renamed Success")
    );
    let resp_after = state
        .current_response
        .as_ref()
        .expect("Response should exist");
    assert_eq!(resp_after.status_text, "OK");
}

#[test]
fn test_response_state_cache_rename_and_eviction() {
    let mut state = ResponseState::new();
    // Configure a small capacity cache for testing
    state.cache = ResponseCache::new(2, 10_000);

    let rec1 = Arc::new(HttpResponseRecord {
        id: "r1".to_string(),
        request_id: "req-1".to_string(),
        status_code: 200,
        status_text: "OK".to_string(),
        duration_ms: 10,
        size_bytes: 5,
        headers: vec![],
        cookies: vec![],
        body: b"hello".to_vec(),
        timing: Default::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    });

    let rec2 = Arc::new(HttpResponseRecord {
        id: "r2".to_string(),
        request_id: "req-2".to_string(),
        status_code: 201,
        status_text: "Created".to_string(),
        duration_ms: 15,
        size_bytes: 5,
        headers: vec![],
        cookies: vec![],
        body: b"world".to_vec(),
        timing: Default::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    });

    state.cache.insert("req-1".to_string(), rec1);
    state.cache.insert("req-2".to_string(), rec2.clone());

    // Test rename_request
    state.rename_request("req-1", "req-1-renamed");
    assert!(!state.cache.contains_key(&"req-1".to_string()));
    assert!(state.cache.contains_key(&"req-1-renamed".to_string()));

    // Insert a 3rd record, which should evict the oldest entry ("req-1-renamed")
    let rec3 = Arc::new(HttpResponseRecord {
        id: "r3".to_string(),
        request_id: "req-3".to_string(),
        status_code: 204,
        status_text: "No Content".to_string(),
        duration_ms: 5,
        size_bytes: 0,
        headers: vec![],
        cookies: vec![],
        body: vec![],
        timing: Default::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    });
    state.cache.insert("req-3".to_string(), rec3);

    assert_eq!(state.cache.len(), 2);
    assert!(!state.cache.contains_key(&"req-1-renamed".to_string()));
    assert!(state.cache.contains_key(&"req-2".to_string()));
    assert!(state.cache.contains_key(&"req-3".to_string()));
}

#[test]
fn test_formatted_body_caching_on_view_mode_toggle() {
    let mut state = ResponseState::new();
    let payload = r#"{"hello": "world", "number": 42}"#.as_bytes().to_vec();
    let record = Arc::new(HttpResponseRecord {
        body: payload,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        ..Default::default()
    });

    state.current_response = Some(record);
    state.view_mode = ResponseViewMode::Pretty;
    state.refresh_formatted_body();

    let (is_json1, display_text1, cache_ptr1) = {
        let fmt = state.formatted_body.as_ref().unwrap();
        (
            fmt.is_json,
            fmt.display_text.clone(),
            fmt.cached_format.as_ref().map(Arc::as_ptr).unwrap(),
        )
    };
    assert!(is_json1);
    assert!(display_text1.contains('\n'));

    // Toggle to Raw mode
    state.view_mode = ResponseViewMode::Raw;
    state.refresh_formatted_body();

    let (display_text2, cache_ptr2) = {
        let fmt = state.formatted_body.as_ref().unwrap();
        (
            fmt.display_text.clone(),
            fmt.cached_format.as_ref().map(Arc::as_ptr).unwrap(),
        )
    };

    // Cached format must be reused with identical Arc pointer (zero re-parsing)
    assert_eq!(cache_ptr1, cache_ptr2);
    assert_eq!(&*display_text2, r#"{"hello": "world", "number": 42}"#);

    // Toggle back to Pretty mode
    state.view_mode = ResponseViewMode::Pretty;
    state.refresh_formatted_body();

    let (display_text3, cache_ptr3) = {
        let fmt = state.formatted_body.as_ref().unwrap();
        (
            fmt.display_text.clone(),
            fmt.cached_format.as_ref().map(Arc::as_ptr).unwrap(),
        )
    };
    assert_eq!(cache_ptr1, cache_ptr3);
    assert_eq!(display_text3, display_text1);
}
