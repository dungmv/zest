use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
pub use gpui_kit::component::dialog::{
    DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, ElementId, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled,
    Window,
};
use std::rc::Rc;

type CloseCallback = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// Reusable modal dialog overlay powered by gpui-kit tokens and dialog semantics.
/// Provides a consistent backdrop dimming overlay, surface elevation, Escape key handling,
/// backdrop click-to-dismiss, and semantic DialogHeader / DialogContent / DialogFooter integration.
#[derive(IntoElement)]
pub struct ModalDialog {
    id: ElementId,
    width: Pixels,
    height: Option<Pixels>,
    max_height: Option<Pixels>,
    title: Option<SharedString>,
    icon: Option<IconName>,
    description: Option<SharedString>,
    header_custom: Option<AnyElement>,
    footer: Option<AnyElement>,
    on_close: Option<CloseCallback>,
    children: Vec<AnyElement>,
}

impl ModalDialog {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            width: px(560.),
            height: None,
            max_height: None,
            title: None,
            icon: None,
            description: None,
            header_custom: None,
            footer: None,
            on_close: None,
            children: Vec::new(),
        }
    }

    pub fn w(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }

    pub fn h(mut self, height: impl Into<Pixels>) -> Self {
        self.height = Some(height.into());
        self
    }

    pub fn max_h(mut self, max_height: impl Into<Pixels>) -> Self {
        self.max_height = Some(max_height.into());
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header_custom = Some(header.into_any_element());
        self
    }

    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    pub fn on_close(mut self, on_close: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }
}

impl ParentElement for ModalDialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ModalDialog {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = theme();
        let close_cb_backdrop = self.on_close.clone();
        let close_cb_escape = self.on_close.clone();
        let close_cb_button = self.on_close.clone();

        let overlay_id = SharedString::from(format!("{}-overlay", self.id));
        let dialog_id = SharedString::from(format!("{}-dialog", self.id));

        let mut overlay = div()
            .id(overlay_id)
            .absolute()
            .inset_0()
            .bg(gpui_kit::Hsla {
                a: 0.65,
                ..t.bg_app
            })
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

        if let Some(on_close) = close_cb_backdrop {
            let cb = on_close.clone();
            overlay = overlay.on_click(move |_, window, cx| {
                cb(window, cx);
            });
        }

        if let Some(on_close) = close_cb_escape {
            let cb = on_close.clone();
            overlay = overlay.on_key_down(move |event, window, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    cb(window, cx);
                }
            });
        }

        let mut dialog_card = div()
            .id(dialog_id)
            .occlude()
            .w(self.width)
            .rounded(px(8.))
            .bg(t.bg_card)
            .border_1()
            .border_color(t.border)
            .shadow_2xl()
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_click(|_, _, cx| cx.stop_propagation());

        if let Some(h) = self.height {
            dialog_card = dialog_card.h(h);
        }
        if let Some(mh) = self.max_height {
            dialog_card = dialog_card.max_h(mh);
        }

        // Render Header: either custom or semantic DialogHeader
        if let Some(header) = self.header_custom {
            dialog_card = dialog_card.child(header);
        } else if self.title.is_some() || self.description.is_some() {
            let header_el = div()
                .h(px(46.))
                .px(px(16.))
                .border_b_1()
                .border_color(t.border)
                .bg(t.bg_titlebar)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .when_some(self.icon, |this, icon| {
                            this.child(Icon::new(icon).size(px(18.)))
                        })
                        .child(
                            DialogHeader::new()
                                .gap(px(2.))
                                .when_some(self.title, |this, title| {
                                    this.child(
                                        DialogTitle::new()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .child(title),
                                    )
                                })
                                .when_some(self.description, |this, desc| {
                                    this.child(
                                        DialogDescription::new()
                                            .text_xs()
                                            .text_color(t.text_muted)
                                            .child(desc),
                                    )
                                }),
                        ),
                )
                .child({
                    let close_btn_id = SharedString::from(format!("{}-close-btn", self.id));
                    let mut btn = Button::new(close_btn_id)
                        .with_variant(ButtonVariant::Ghost)
                        .icon(IconName::Close);
                    if let Some(on_close) = close_cb_button {
                        let cb = on_close.clone();
                        btn = btn.on_click(move |_, window, cx| {
                            cb(window, cx);
                        });
                    }
                    btn
                });
            dialog_card = dialog_card.child(header_el);
        }

        // Render Content children inside semantic DialogContent
        let content_el = DialogContent::new()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .children(self.children);
        dialog_card = dialog_card.child(content_el);

        // Render Footer if provided
        if let Some(footer) = self.footer {
            dialog_card = dialog_card.child(footer);
        }

        overlay.child(dialog_card)
    }
}
