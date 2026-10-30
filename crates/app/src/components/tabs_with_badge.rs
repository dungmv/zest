use crate::theme::AppTheme;
use gpui_kit::component::tag::Tag;
use gpui_kit::{div, px, AnyElement, IntoElement, ParentElement, Styled};

/// Renders a small badge for tab counters (e.g. `[0]`, `[15]`, `*`).
pub fn render_tab_badge(text: impl Into<String>, is_active: bool, t: &AppTheme) -> AnyElement {
    let text_color = if is_active {
        t.text_primary
    } else {
        t.text_muted
    };

    Tag::custom(t.bg_card, text_color, t.border)
        .rounded(px(8.))
        .px(px(5.))
        .py(px(0.5))
        .child(div().text_xs().child(text.into()))
        .into_any_element()
}
