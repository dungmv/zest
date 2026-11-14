use super::ResponseInspector;
use crate::theme::theme;
use gpui_kit::component::empty::{Empty, EmptyDescription};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::Sizable as _;
use gpui_kit::{div, px, AnyElement, App, IntoElement, ParentElement, Styled};
use zest_core::ws::{WsDirection, WsMessage, WsMessageKind, WebSocketStatus};

/// Frame log for WebSocket/gRPC requests (Messages tab).
pub(super) fn render_messages_tab(
    ri: &ResponseInspector,
    cx: &App,
    is_sending_this_request: bool,
) -> AnyElement {
    let t = theme();
    let resp_s = ri.response_state.read(cx);
    let status = resp_s.ws_status.clone();
    let messages = resp_s.ws_messages.clone();

    if is_sending_this_request {
        return div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(Spinner::new().with_size(px(20.)).color(t.primary))
            .child(
                div()
                    .text_xs()
                    .text_color(t.text_secondary)
                    .child("Connecting…"),
            )
            .into_any_element();
    }

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(t.bg_response)
        .child(
            div().flex_1().overflow_y_scrollbar().p(px(12.)).child({
                if messages.is_empty() {
                    render_messages_empty_state(&status)
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .children(messages.into_iter().map(render_message_card))
                        .into_any_element()
                }
            }),
        )
        .into_any_element()
}

fn render_messages_empty_state(status: &WebSocketStatus) -> AnyElement {
    let hint = match status {
        WebSocketStatus::Connected => "Connected — no messages yet",
        WebSocketStatus::Connecting => "Connecting…",
        WebSocketStatus::Disconnected => "Not connected",
        WebSocketStatus::Failed(_) => "Connection failed",
    };
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Empty::new().child(EmptyDescription::new().child(hint)),
        )
        .into_any_element()
}

fn render_message_card(msg: WsMessage) -> AnyElement {
    let t = theme();
    let is_inbound = msg.direction == WsDirection::Inbound;

    let (direction_symbol, direction_color) = match (is_inbound, msg.kind) {
        (true, WsMessageKind::Ping) | (true, WsMessageKind::Pong) => {
            ("←", t.text_muted)
        }
        (true, _) => ("←", t.status_2xx),
        (false, WsMessageKind::Ping) | (false, WsMessageKind::Pong) => ("→", t.text_muted),
        (false, _) => ("→", t.method_websocket),
    };
    let kind_label = match msg.kind {
        WsMessageKind::Text => "TEXT",
        WsMessageKind::Binary => "BIN",
        WsMessageKind::Ping => "PING",
        WsMessageKind::Pong => "PONG",
        WsMessageKind::Close => "CLOSE",
    };

    let time = chrono::DateTime::from_timestamp_millis(msg.timestamp_ms)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M:%S%.3f").to_string())
        .unwrap_or_default();

    div()
        .rounded(px(6.))
        .border_1()
        .border_color(t.border_subtle)
        .bg(t.bg_card)
        .p(px(8.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_xs()
                .child(
                    div()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(direction_color)
                        .child(direction_symbol),
                )
                .child(
                    div()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(direction_color)
                        .child(kind_label),
                )
                .child(
                    div()
                        .text_color(t.text_muted)
                        .child(format!("{} · {} B", time, msg.size_bytes())),
                ),
        )
        .child(render_payload(&msg))
        .into_any_element()
}

fn render_payload(msg: &WsMessage) -> AnyElement {
    let t = theme();
    if msg.kind == WsMessageKind::Binary {
        let hex: String = msg
            .payload
            .iter()
            .take(256)
            .map(|b| format!("{b:02x} "))
            .collect();
        let suffix = if msg.payload.len() > 256 { " …" } else { "" };
        return div()
            .font_family("monospace")
            .text_xs()
            .text_color(t.text_secondary)
            .child(format!("{}{}", hex.trim_end(), suffix))
            .into_any_element();
    }

    let text = String::from_utf8_lossy(&msg.payload).to_string();
    div()
        .font_family("monospace")
        .text_xs()
        .text_color(t.text_primary)
        .child(text)
        .into_any_element()
}

/// Headers tab for realtime protocols: shows the handshake response headers.
pub(super) fn render_realtime_headers_tab(ri: &ResponseInspector, cx: &App) -> AnyElement {
    let t = theme();
    let resp_s = ri.response_state.read(cx);
    let headers = resp_s.ws_handshake_headers.clone();

    div().size_full().overflow_y_scrollbar().p(px(12.)).child({
        if headers.is_empty() {
            div()
                .text_xs()
                .text_color(t.text_muted)
                .child("No handshake headers (not connected yet)")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(headers.into_iter().map(|(name, value)| {
                    div()
                        .flex()
                        .gap(px(8.))
                        .text_xs()
                        .py(px(2.))
                        .border_b_1()
                        .border_color(t.border_subtle)
                        .child(
                            div()
                                .font_family("monospace")
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .text_color(t.text_primary)
                                .min_w(px(180.))
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .font_family("monospace")
                                .text_color(t.text_secondary)
                                .child(value),
                        )
                }))
                .into_any_element()
        }
    })
    .into_any_element()
}
