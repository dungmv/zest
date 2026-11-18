use super::*;
use crate::models::{HttpMethod, RequestItem};

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
    let item1 = RequestItem::new("ws-1", "Test Req 1", HttpMethod::Get, &server_url);
    let resolved1 = ResolvedRequest::from_item(&item1, None).unwrap();
    let res1 = executor
        .send(resolved1, None)
        .await
        .expect("send request 1 should succeed");
    assert_eq!(res1.status_code, 200);
    assert_eq!(res1.body, b"Hello, World!");
    assert!(!res1.timing.connection_reused);

    // Request 2: Reused keep-alive connection
    let item2 = RequestItem::new("ws-2", "Test Req 2", HttpMethod::Get, &server_url);
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
    let item1 = RequestItem::new("ws-1", "Test Req 1", HttpMethod::Get, &server_url);
    let resolved1 = ResolvedRequest::from_item(&item1, None).unwrap();
    let res1 = executor.send(resolved1, None).await.unwrap();
    assert!(!res1.timing.connection_reused);

    // Request 2: Must not be marked as reused because connection was closed
    let item2 = RequestItem::new("ws-2", "Test Req 2", HttpMethod::Get, &server_url);
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

#[tokio::test]
async fn test_send_streaming_sse_events_and_record() {
    use crate::sse::SseOutput;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/stream", port);

    // One-shot SSE server: keep-alive comment, then two events, then EOF.
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let body = ": keep-alive\ndata: {\"n\":1}\n\nevent: add\ndata: two\ndata: more\n\n";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(resp.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    let executor = ReqwestExecutor::new();
    let item = RequestItem::new("sse-1", "SSE", HttpMethod::Get, &server_url);
    let resolved = ResolvedRequest::from_item(&item, None).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();

    let record = executor
        .send_streaming(resolved, None, tx, cancel_rx)
        .await
        .expect("streaming send should succeed");
    assert_eq!(record.status_code, 200);
    assert!(
        String::from_utf8_lossy(&record.body).contains("data: {\"n\":1}"),
        "raw body is captured for the Response tab"
    );
    assert!(record.size_bytes > 0);

    // Comment first, then the two events in arrival order.
    let mut outputs = Vec::new();
    while let Ok(out) = rx.try_recv() {
        outputs.push(out);
    }
    assert_eq!(outputs.len(), 3, "comment + 2 events, got {outputs:?}");
    assert_eq!(outputs[0], SseOutput::Comment("keep-alive".to_string()));
    let SseOutput::Event(first) = &outputs[1] else {
        panic!("expected event");
    };
    assert_eq!(first.data, "{\"n\":1}");
    assert_eq!(first.event, None);
    let SseOutput::Event(second) = &outputs[2] else {
        panic!("expected event");
    };
    assert_eq!(second.event.as_deref(), Some("add"));
    assert_eq!(second.data, "two\nmore");
}

#[tokio::test]
async fn test_send_streaming_cancelled_before_connect() {
    use tokio::sync::mpsc;

    let executor = ReqwestExecutor::new();
    let item = RequestItem::new("sse-2", "SSE", HttpMethod::Get, "http://127.0.0.1:1/x");
    let resolved = ResolvedRequest::from_item(&item, None).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    drop(cancel_tx); // cancel immediately

    let result = executor.send_streaming(resolved, None, tx, cancel_rx).await;
    assert!(matches!(result, Err(RequestError::Cancelled)));
}

#[test]
fn test_multipart_request_headers_and_boundary() {
    use crate::models::{KeyValuePair, MultipartItem, RequestBody};

    let mut item = RequestItem::new(
        "mp-1",
        "MP",
        HttpMethod::Post,
        "http://127.0.0.1:8080/upload",
    );
    item.body = RequestBody::Multipart {
        items: vec![MultipartItem::new_text("username", "alice")],
    };
    item.headers
        .push(KeyValuePair::new("Content-Type", "multipart/form-data"));
    let resolved = ResolvedRequest::from_item(&item, None).unwrap();
    let executor = ReqwestExecutor::new();
    let builder = executor.build_request_builder(&resolved, false).unwrap();
    let req = builder.build().unwrap();
    let ct = req.headers().get("content-type").unwrap().to_str().unwrap();
    eprintln!("ACTUAL CONTENT-TYPE: {}", ct);
    assert!(
        ct.contains("boundary="),
        "Expected boundary in Content-Type, got: {}",
        ct
    );
}

#[tokio::test]
async fn test_multipart_form_data_wire_transfer() {
    use crate::models::{KeyValuePair, MultipartItem, RequestBody};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let file_path = temp_file.path().to_str().unwrap().to_string();
    std::fs::write(&file_path, b"dummy file content 12345").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/upload", port);

    let (captured_tx, mut captured_rx) = tokio::sync::mpsc::channel(1);

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 4096];
            let mut total_read = Vec::new();
            // Read until we get the full multipart payload
            loop {
                let n = socket.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                total_read.extend_from_slice(&buf[..n]);
                // If we've read the end boundary or enough bytes
                if total_read
                    .windows(4)
                    .any(|w| w == b"--\r\n" || w == b"--\n")
                    || total_read.len() > 500
                {
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                        .await;
                    break;
                }
            }
            let _ = captured_tx.send(total_read).await;
        }
    });

    let mut item = RequestItem::new("mp-test", "Multipart Test", HttpMethod::Post, &server_url);
    item.body = RequestBody::Multipart {
        items: vec![
            MultipartItem::new_text("title", "Greetings"),
            MultipartItem::new_file("attachment", &file_path),
        ],
    };
    item.headers
        .push(KeyValuePair::new("Content-Type", "multipart/form-data"));

    let resolved = ResolvedRequest::from_item(&item, None).unwrap();
    let executor = ReqwestExecutor::new();
    let res = executor
        .send(resolved, None)
        .await
        .expect("send multipart request");
    assert_eq!(res.status_code, 200);

    let raw_request = captured_rx.recv().await.expect("captured request");
    let req_str = String::from_utf8_lossy(&raw_request);
    eprintln!("RAW REQUEST FROM CLIENT:\n{}", req_str);

    assert!(req_str
        .to_ascii_lowercase()
        .contains("content-type: multipart/form-data; boundary="));
    assert!(req_str.contains("name=\"title\""));
    assert!(req_str.contains("Greetings"));
    assert!(req_str.contains("name=\"attachment\""));
    assert!(req_str.contains("dummy file content 12345"));
}
