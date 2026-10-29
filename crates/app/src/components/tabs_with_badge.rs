use crate::theme::AppTheme;
use gpui_kit::{div, px, AnyElement, IntoElement, ParentElement, Styled};

/// Renders a small badge for tab counters (e.g. `[0]`, `[15]`, `*`).
pub fn render_tab_badge(text: impl Into<String>, is_active: bool, t: &AppTheme) -> AnyElement {
    div()
        .px(px(5.))
        .py(px(0.5))
        .rounded(px(8.))
        .bg(t.bg_card)
        .border_1()
        .border_color(t.border)
        .text_color(if is_active {
            t.text_primary
        } else {
            t.text_muted
        })
        .text_xs()
        .child(text.into())
        .into_any_element()
}
