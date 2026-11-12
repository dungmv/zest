use super::*;
use crate::http::resolved::{build_url_with_query, parse_query_from_url};
use crate::models::{HttpMethod, HttpRequestItem, KeyValuePair, MultipartItem, RequestBody};

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
    let mut item = HttpRequestItem::new(
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
    let mut item = HttpRequestItem::new(
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
    let mut item_clean = HttpRequestItem::new(
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
fn test_reqwest_executor_client_caching_and_prewarm() {
    let executor = ReqwestExecutor::new();
    // Default client (true, true) is pre-warmed on creation
    assert_eq!(executor.cached_clients_count(), 1);

    // Fetching existing (true, true) does not create a new client
    let _ = executor.get_client(true, true).unwrap();
    assert_eq!(executor.cached_clients_count(), 1);

    // Fetching (false, true) creates and caches client #2
    let _ = executor.get_client(false, true).unwrap();
    assert_eq!(executor.cached_clients_count(), 2);

    // Re-fetching (false, true) uses cached client
    let _ = executor.get_client(false, true).unwrap();
    assert_eq!(executor.cached_clients_count(), 2);

    // Fetching (true, false) and (false, false)
    let _ = executor.get_client(true, false).unwrap();
    assert_eq!(executor.cached_clients_count(), 3);

    let _ = executor.get_client(false, false).unwrap();
    assert_eq!(executor.cached_clients_count(), 4);

    // Re-fetching default client still succeeds
    let _ = executor.get_client(true, true).unwrap();
    assert_eq!(executor.cached_clients_count(), 4);
}

#[tokio::test]
async fn test_reqwest_executor_send_local_http() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/test", port);

    // Spawn mock HTTP server that handles connections
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let n = match socket.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    // Simple HTTP response
                    if n > 0 {
                        let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 13\r\nConnection: keep-alive\r\n\r\nHello, World!";
                        if socket.write_all(resp.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                }
            });
        }
    });

    let executor = ReqwestExecutor::new();

    // Request 1: Cold connection
    let item1 = HttpRequestItem::new("ws-1", "Test Req 1", HttpMethod::Get, &server_url);
    let resolved1 = ResolvedRequest::from_item(&item1, None).unwrap();
    let res1 = executor
        .send(resolved1, None)
        .await
        .expect("send request 1 should succeed");
    assert_eq!(res1.status_code, 200);
    assert_eq!(res1.body, b"Hello, World!");
    assert!(!res1.timing.connection_reused);

    // Request 2: Reused keep-alive connection
    let item2 = HttpRequestItem::new("ws-2", "Test Req 2", HttpMethod::Get, &server_url);
    let resolved2 = ResolvedRequest::from_item(&item2, None).unwrap();
    let res2 = executor
        .send(resolved2, None)
        .await
        .expect("send request 2 should succeed");
    assert_eq!(res2.status_code, 200);
    assert_eq!(res2.body, b"Hello, World!");
    assert!(res2.timing.connection_reused);
    assert_eq!(res2.timing.dns_ms, Some(0));
    assert_eq!(res2.timing.connect_ms, Some(0));
}

#[tokio::test]
async fn test_reqwest_executor_connection_close_not_reused() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/close", port);

    // Spawn mock server that responds with Connection: close
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDone";
                let _ = socket.write_all(resp.as_bytes()).await;
            });
        }
    });

    let executor = ReqwestExecutor::new();

    // Request 1: Connection: close
    let item1 = HttpRequestItem::new("ws-1", "Test Req 1", HttpMethod::Get, &server_url);
    let resolved1 = ResolvedRequest::from_item(&item1, None).unwrap();
    let res1 = executor.send(resolved1, None).await.unwrap();
    assert!(!res1.timing.connection_reused);

    // Request 2: Must not be marked as reused because connection was closed
    let item2 = HttpRequestItem::new("ws-2", "Test Req 2", HttpMethod::Get, &server_url);
    let resolved2 = ResolvedRequest::from_item(&item2, None).unwrap();
    let res2 = executor.send(resolved2, None).await.unwrap();
    assert!(!res2.timing.connection_reused);
}

#[test]
fn test_cookie_store_mutex_poison_recovery() {
    let executor = ReqwestExecutor::new();
    let cookie_store = executor.cookie_store();

    // Intentionally poison the mutex
    let cs_clone = cookie_store.clone();
    let _ = std::panic::catch_unwind(move || {
        let _guard = cs_clone.lock().unwrap_or_else(|p| p.into_inner());
        panic!("Poisoning cookie store mutex");
    });

    assert!(cookie_store.lock().is_err());

    // Calling get_all_cookies, get_cookies_for_url, or clear_cookies must not panic
    let cookies = executor.get_all_cookies();
    assert!(cookies.is_empty());

    let url_cookies = executor.get_cookies_for_url("https://example.com");
    assert!(url_cookies.is_empty());

    executor.clear_cookies();
}

#[test]
fn test_stored_cookies_roundtrip_and_removal() {
    let executor = ReqwestExecutor::new();
    let sample = vec![
        StoredCookie {
            name: "session_id".to_string(),
            value: "abc123xyz".to_string(),
            domain: "api.example.com".to_string(),
            path: "/".to_string(),
            expires: None,
            secure: true,
            http_only: true,
            same_site: Some("Lax".to_string()),
        },
        StoredCookie {
            name: "theme".to_string(),
            value: "dark".to_string(),
            domain: "app.example.com".to_string(),
            path: "/dashboard".to_string(),
            expires: None,
            secure: false,
            http_only: false,
            same_site: None,
        },
    ];

    executor.restore_stored_cookies(&sample);
    let stored = executor.get_stored_cookies();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].domain, "api.example.com");
    assert_eq!(stored[0].name, "session_id");
    assert_eq!(stored[0].value, "abc123xyz");
    assert!(stored[0].secure);
    assert!(stored[0].http_only);

    executor.remove_cookie("api.example.com", "/", "session_id");
    let after_remove = executor.get_stored_cookies();
    assert_eq!(after_remove.len(), 1);
    assert_eq!(after_remove[0].name, "theme");

    executor.clear_cookies();
    assert!(executor.get_stored_cookies().is_empty());
}
