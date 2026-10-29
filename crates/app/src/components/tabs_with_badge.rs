use crate::theme::theme;
use gpui_kit::component::tab::Tab;
use gpui_kit::component::{Selectable as _, Sizable as _, Size};
use gpui_kit::{
    div, px, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, Styled, Window,
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
        let mut tab = Tab::new()
            .underline()
            .with_size(Size::Small)
            .label(self.label)
            .selected(self.is_active);

        let mut suffix_el = div().flex().items_center().gap(px(4.));
        let mut has_suffix = false;

        if self.has_dropdown {
            suffix_el = suffix_el.child(div().text_xs().text_color(t.text_muted).child("▾"));
            has_suffix = true;
        }

        if let Some(badge_text) = self.badge {
            suffix_el = suffix_el.child(
                div()
                    .px(px(5.))
                    .py(px(0.5))
                    .rounded(px(8.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .text_color(if self.is_active {
                        t.text_primary
                    } else {
                        t.text_muted
                    })
                    .text_xs()
                    .child(badge_text),
            );
            has_suffix = true;
        }

        if has_suffix {
            tab = tab.suffix(suffix_el);
        }

        if let Some(handler) = self.on_click {
            tab = tab.on_click(move |ev, window, cx| handler(ev, window, cx));
        }

        div().id(self.id).child(tab)
    }
}
