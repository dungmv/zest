use crate::theme::theme;
use gpui_kit::{
    div, px, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use std::rc::Rc;

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct TabItem {
    id: ElementId,
    label: String,
    badge: Option<String>,
    is_active: bool,
    has_dropdown: bool,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl TabItem {
    pub fn new(id: impl Into<ElementId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            badge: None,
            is_active: false,
            has_dropdown: false,
            on_click: None,
        }
    }

    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }

    pub fn badge(mut self, count_str: impl Into<String>) -> Self {
        self.badge = Some(count_str.into());
        self
    }

    #[allow(dead_code)]
    pub fn with_dropdown(mut self, has_dropdown: bool) -> Self {
        self.has_dropdown = has_dropdown;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TabItem {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let t = theme();
        let active = self.is_active;

        let fg = if active {
            t.text_primary
        } else {
            t.text_secondary
        };

        let border_bottom = if active { t.primary } else { t.transparent };

        let mut el = div()
            .id(self.id)
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .text_xs()
            .font_weight(if active {
                gpui_kit::FontWeight::SEMIBOLD
            } else {
                gpui_kit::FontWeight::NORMAL
            })
            .text_color(fg)
            .border_b_2()
            .border_color(border_bottom)
            .hover(|s| s.text_color(t.text_primary))
            .child(div().child(self.label));

        if let Some(handler) = self.on_click {
            el = el.on_click(move |ev, window, cx| handler(ev, window, cx));
        }

        if self.has_dropdown {
            el = el.child(div().text_xs().text_color(t.text_muted).child("▾"));
        }

        if let Some(badge_text) = self.badge {
            el = el.child(
                div()
                    .px(px(5.))
                    .py(px(0.5))
                    .rounded(px(8.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .text_color(if active { t.text_primary } else { t.text_muted })
                    .text_xs()
                    .child(badge_text),
            );
        }

        el
    }
}
