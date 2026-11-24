use gpui_kit::{div, px, AnyElement, FontWeight, Hsla, IntoElement, ParentElement, Styled};

/// Compact tinted status pill used across the response inspector status bar:
/// HTTP status, WebSocket connection state and gRPC call result all share
/// the same chrome (padded rounded box, 12% tinted background, 30% border).
pub(super) fn status_pill(label: String, color: Hsla) -> AnyElement {
    div()
        .px(px(6.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(Hsla { a: 0.12, ..color })
        .border_1()
        .border_color(Hsla { a: 0.3, ..color })
        .text_xs()
        .font_weight(FontWeight::NORMAL)
        .text_color(color)
        .child(label)
        .into_any_element()
}
