use super::*;
use crate::models::{HttpRequestItem, Protocol};

fn ws_item(url: &str) -> HttpRequestItem {
    let mut item = HttpRequestItem::new("ws", "Echo", crate::models::HttpMethod::Get, url);
    item.protocol = Protocol::WebSocket;
    item
}

#[test]
fn test_handshake_request_filters_reserved_headers() {
    let mut item = ws_item("wss://echo.example/socket");
    item.headers
        .push(crate::models::KeyValuePair::new("X-Custom", "yes"));
    item.headers
        .push(crate::models::KeyValuePair::new("Connection", "keep-alive"));
    item.headers
        .push(crate::models::KeyValuePair::new("Upgrade", "h2c"));
    let resolved = ResolvedRequest::from_item(&item, None).unwrap();

    let request = WebSocketClient::build_handshake_request(&resolved).unwrap();
    let headers = request.headers();

    assert_eq!(
        headers.get("x-custom").map(|v| v.to_str().unwrap()),
        Some("yes")
    );
    // Mandatory handshake headers must remain tungstenite-generated.
    assert_eq!(
        headers.get("connection").map(|v| v.to_str().unwrap()),
        Some("Upgrade")
    );
    assert_eq!(
        headers.get("upgrade").map(|v| v.to_str().unwrap()),
        Some("websocket")
    );
    assert!(headers.get("sec-websocket-key").is_some());
    assert_eq!(request.uri().scheme_str(), Some("wss"));
    assert_eq!(request.uri().host(), Some("echo.example"));
}

#[test]
fn test_handshake_rejects_invalid_header_value() {
    let mut item = ws_item("ws://echo.example/socket");
    item.headers
        .push(crate::models::KeyValuePair::new("X-Bad", "bad\nvalue"));
    let resolved = ResolvedRequest::from_item(&item, None).unwrap();

    let err = WebSocketClient::build_handshake_request(&resolved).unwrap_err();
    assert!(matches!(err, HttpError::InvalidHeader(_)), "got {err:?}");
}

#[test]
fn test_connect_times_out_on_unroutable_host() {
    let mut item = ws_item("ws://10.255.255.1:9999/socket");
    item.settings.timeout_ms = 300;
    let resolved = ResolvedRequest::from_item(&item, None).unwrap();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    // Unroutable addresses fail with a network error (TCP connect refused by
    // the timeout race), never a panic. Map away the non-Debug handle first.
    let result = rt.block_on(async { WebSocketClient::connect(&resolved).await.map(|_| ()) });
    match result {
        Err(HttpError::Timeout(300)) => {}
        Err(HttpError::Network(msg)) if msg.contains("handshake") => {}
        other => panic!("unexpected result: {other:?}"),
    }
}
