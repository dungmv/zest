use super::*;

#[test]
fn test_text_preview_truncates() {
    let msg = TransportFrame {
        id: 1,
        timestamp_ms: 0,
        direction: FrameDirection::Inbound,
        kind: FrameKind::Text,
        payload: "hello".as_bytes().to_vec(),
    };
    assert_eq!(msg.text_preview(10), "hello");
    assert_eq!(msg.size_bytes(), 5);

    let long = "x".repeat(300);
    let msg = TransportFrame {
        payload: long.into_bytes(),
        ..msg
    };
    let preview = msg.text_preview(100);
    assert_eq!(preview.chars().count(), 101, "100 chars + ellipsis");
    assert!(preview.ends_with('…'));
}

#[test]
fn test_transport_frame_is_serde_round_trippable() {
    let msg = TransportFrame {
        id: 7,
        timestamp_ms: 42,
        direction: FrameDirection::Outbound,
        kind: FrameKind::Binary,
        payload: vec![0u8, 1, 2],
    };
    let json = serde_json::to_string(&msg).unwrap();
    let back: TransportFrame = serde_json::from_str(&json).unwrap();
    assert_eq!(back, msg);
}
