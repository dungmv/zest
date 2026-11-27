use super::ResponseInspector;
use crate::theme::theme;
use gpui_kit::component::empty::{Empty, EmptyDescription};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::Sizable as _;
use gpui_kit::{div, list, px, AnyElement, App, IntoElement, ParentElement, Styled};
use zest_core::models::Protocol;
use zest_core::transport::{FrameDirection, FrameKind, TransportFrame};
use zest_core::ws::WebSocketStatus;

/// Preview cap per message card: payloads render at most this many code
/// points (wrapped), everything longer is marked with `…`.
const MESSAGE_PREVIEW_CHARS: usize = 1000;

/// Frame log for WebSocket/gRPC/SSE requests (Messages tab).
pub(super) fn render_messages_tab(
    ri: &ResponseInspector,
    cx: &App,
    is_sending_this_request: bool,
) -> AnyElement {
    let t = theme();
    let resp_state = ri.response_state.clone();
    let (status, frame_count, frames_list) = {
        let resp_s = ri.response_state.read(cx);
        (
            resp_s.realtime.ws_status.clone(),
            resp_s.realtime.frames.len(),
            resp_s.realtime.frames_list.clone(),
        )
    };
    let protocol = ri
        .workspace_state
        .read(cx)
        .find_request(ri.editor_state.read(cx).active_request_id())
        .map(|r| r.protocol)
        .unwrap_or(Protocol::Http);
    let is_sse = protocol == Protocol::Sse;

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
                    .child(if is_sse {
                        "Streaming…"
                    } else {
                        "Connecting…"
                    }),
            )
            .into_any_element();
    }

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(t.bg_response)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .p(px(12.))
                .child({
                    if frame_count == 0 {
                        render_messages_empty_state(&status, is_sse)
                    } else {
                        // Virtualized variable-height list: only visible cards
                        // are built per frame; the list tail-follows while the
                        // user is scrolled to the bottom.
                        list(frames_list, move |idx, _window, cx| {
                            let state = resp_state.read(cx);
                            match state.realtime.frames.get(idx) {
                                Some(frame) => render_message_card(frame),
                                // Stale index during a reset rebuild.
                                None => div().into_any_element(),
                            }
                        })
                        .into_any_element()
                    }
                }),
        )
        .into_any_element()
}

fn render_messages_empty_state(status: &WebSocketStatus, is_sse: bool) -> AnyElement {
    let hint = if is_sse {
        "Send to start streaming events"
    } else {
        match status {
            WebSocketStatus::Connected => "Connected — no messages yet",
            WebSocketStatus::Connecting => "Connecting…",
            WebSocketStatus::Disconnected => "Not connected",
            WebSocketStatus::Failed(_) => "Connection failed",
        }
    };
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(Empty::new().child(EmptyDescription::new().child(hint)))
        .into_any_element()
}

fn render_message_card(msg: &TransportFrame) -> AnyElement {
    let t = theme();
    let is_inbound = msg.direction == FrameDirection::Inbound;

    let (direction_symbol, direction_color) = match (is_inbound, msg.kind) {
        (true, FrameKind::Ping) | (true, FrameKind::Pong) => ("←", t.text_muted),
        (true, _) => ("←", t.status_2xx),
        (false, FrameKind::Ping) | (false, FrameKind::Pong) => ("→", t.text_muted),
        (false, _) => ("→", t.method_websocket),
    };
    let kind_label: String = msg.label.clone().unwrap_or_else(|| match msg.kind {
        FrameKind::Text => "TEXT".to_string(),
        FrameKind::Binary => "BIN".to_string(),
        FrameKind::Ping => "PING".to_string(),
        FrameKind::Pong => "PONG".to_string(),
        FrameKind::Close => "CLOSE".to_string(),
    });

    let time = chrono::DateTime::from_timestamp_millis(msg.timestamp_ms)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%H:%M:%S%.3f")
                .to_string()
        })
        .unwrap_or_default();

    div()
        .w_full()
        .mb(px(6.))
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
                .child(div().text_color(t.text_muted).child(format!(
                    "{} · {} B",
                    time,
                    msg.size_bytes()
                ))),
        )
        .child(render_payload(msg))
        .into_any_element()
}

fn render_payload(msg: &TransportFrame) -> AnyElement {
    let t = theme();
    if msg.kind == FrameKind::Binary {
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

    let text = msg.text_preview(MESSAGE_PREVIEW_CHARS);
    div()
        .font_family("monospace")
        .text_xs()
        .text_color(t.text_primary)
        .child(text)
        .into_any_element()
}

/// Headers tab for realtime protocols: WS shows the handshake response
/// headers; gRPC shows the most recent call's response headers.
pub(super) fn render_realtime_headers_tab(ri: &ResponseInspector, cx: &App) -> AnyElement {
    let t = theme();
    let resp_s = ri.response_state.read(cx);
    let headers = match &resp_s.realtime.grpc_last_call {
        Some(call) if !call.headers.is_empty() => call.headers.clone(),
        _ => resp_s.realtime.ws_handshake_headers.clone(),
    };

    div()
        .size_full()
        .overflow_y_scrollbar()
        .p(px(12.))
        .child({
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
