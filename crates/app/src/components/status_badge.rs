use crate::theme::theme;
use gpui_kit::{div, px, App, IntoElement, ParentElement, RenderOnce, Styled, Window};
use zest_core::models::HttpMethod;

#[derive(IntoElement)]
pub struct MethodBadge {
    method: HttpMethod,
    font_weight: gpui_kit::FontWeight,
}

impl MethodBadge {
    pub fn new(method: HttpMethod) -> Self {
        Self {
            method,
            font_weight: gpui_kit::FontWeight::NORMAL,
        }
    }

    pub fn font_weight(mut self, weight: gpui_kit::FontWeight) -> Self {
        self.font_weight = weight;
        self
    }
}

impl RenderOnce for MethodBadge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let t = theme();
        let color = t.method_color(self.method);
        let bg_color = gpui_kit::Hsla { a: 0.15, ..color };

        div()
            .flex_shrink_0()
            .px(px(6.))
            .py(px(1.5))
            .rounded(px(4.))
            .bg(bg_color)
            .text_color(color)
            .text_xs()
            .font_weight(self.font_weight)
            .child(self.method.as_str())
    }
}

#[derive(IntoElement)]
pub struct StatusCodeBadge {
    status_code: u16,
    font_weight: gpui_kit::FontWeight,
}

impl StatusCodeBadge {
    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            font_weight: gpui_kit::FontWeight::NORMAL,
        }
    }

    // Builder method for future customization of status badge font weight
    #[allow(dead_code)]
    pub fn font_weight(mut self, weight: gpui_kit::FontWeight) -> Self {
        self.font_weight = weight;
        self
    }
}

impl RenderOnce for StatusCodeBadge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let t = theme();
        let color = t.status_color(self.status_code);
        let bg_color = gpui_kit::Hsla { a: 0.15, ..color };

        div()
            .flex_shrink_0()
            .px(px(5.))
            .py(px(1.))
            .rounded(px(3.))
            .bg(bg_color)
            .text_color(color)
            .text_xs()
            .font_weight(self.font_weight)
            .child(format!("{}", self.status_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_badge_font_weight_default_and_builder() {
        let badge = MethodBadge::new(HttpMethod::Get);
        assert_eq!(badge.font_weight, gpui_kit::FontWeight::NORMAL);

        let bold_badge = badge.font_weight(gpui_kit::FontWeight::SEMIBOLD);
        assert_eq!(bold_badge.font_weight, gpui_kit::FontWeight::SEMIBOLD);
    }

    #[test]
    fn test_status_code_badge_font_weight_default_and_builder() {
        let badge = StatusCodeBadge::new(200);
        assert_eq!(badge.font_weight, gpui_kit::FontWeight::NORMAL);

        let custom_badge = badge.font_weight(gpui_kit::FontWeight::MEDIUM);
        assert_eq!(custom_badge.font_weight, gpui_kit::FontWeight::MEDIUM);
    }
}
