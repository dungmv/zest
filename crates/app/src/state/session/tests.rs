use super::*;

#[test]
fn test_editor_state_initial_and_active_request() {
    let mut editor = RequestSessionState::new();
    assert_eq!(editor.active_request_id(), "scratch");

    editor.active_request_id = Some("req_1".to_string());
    assert_eq!(editor.active_request_id(), "req_1");
}

#[gpui_kit::test]
fn cancel_signals_the_send_without_finishing_it(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::AppContext as _;

    let session = cx.new(|_| RequestSessionState::new());
    let (cancel, mut cancellation) = tokio::sync::oneshot::channel();
    session.update(cx, |session, cx| {
        session.start_sending(Some("request-a".into()), cancel, cx);
        assert!(session.is_sending_for(Some("request-a")));
        assert!(!session.is_sending_for(Some("request-b")));

        session.cancel_send();
        assert_eq!(cancellation.try_recv(), Ok(()));
        assert!(session.is_sending());

        session.stop_sending(cx);
        assert!(!session.is_sending());
        assert_eq!(session.send_progress(), None);
    });
}
