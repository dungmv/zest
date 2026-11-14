use crate::theme::theme;
use gpui_kit::{div, px, Div, Styled};

/// 32px toolbar shell shared by the HTTP Body, WebSocket Message and gRPC
/// Message toolbars: full-width row with a bottom border and vertically
/// centered children.
pub fn message_toolbar() -> Div {
    let t = theme();
    div()
        .h(px(32.))
        .px(px(8.))
        .border_b_1()
        .border_color(t.border)
        .bg(t.bg_editor)
        .flex()
        .items_center()
}
