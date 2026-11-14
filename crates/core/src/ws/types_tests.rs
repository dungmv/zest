use super::*;

#[test]
fn test_text_preview_truncates() {
    let msg = WsMessage {
        id: 1,
        timestamp_ms: 0,
        direction: WsDirection::Inbound,
        kind: WsMessageKind::Text,
        payload: "hello".as_bytes().to_vec(),
    };
    assert_eq!(msg.text_preview(10), "hello");
    assert_eq!(msg.size_bytes(), 5);

    let long = "x".repeat(300);
    let msg = WsMessage {
        payload: long.into_bytes(),
        ..msg
    };
    let preview = msg.text_preview(100);
    assert_eq!(preview.chars().count(), 101, "100 chars + ellipsis");
    assert!(preview.ends_with('…'));
}

#[test]
fn test_ws_event_and_message_are_serde_round_trippable() {
    let msg = WsMessage {
        id: 7,
        timestamp_ms: 42,
        direction: WsDirection::Outbound,
        kind: WsMessageKind::Binary,
        payload: vec![0u8, 1, 2],
    };
    let json = serde_json::to_string(&msg).unwrap();
    let back: WsMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(back, msg);

    let status = WebSocketStatus::Failed("boom".to_string());
    let json = serde_json::to_string(&status).unwrap();
    let back: WebSocketStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, status);
}
