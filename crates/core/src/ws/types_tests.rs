use super::*;

#[test]
fn test_ws_status_is_serde_round_trippable() {
    let status = WebSocketStatus::Failed("boom".to_string());
    let json = serde_json::to_string(&status).unwrap();
    let back: WebSocketStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, status);
}
