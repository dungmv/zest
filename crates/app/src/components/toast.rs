use crate::state::{ToastKind, ToastState};
use crate::theme::theme;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window,
};

#[derive(IntoElement)]
pub struct ToastContainer {
    toast_state: Entity<ToastState>,
}

impl ToastContainer {
    pub fn new(toast_state: Entity<ToastState>) -> Self {
        Self { toast_state }
    }
}

impl RenderOnce for ToastContainer {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let toasts = self.toast_state.read(cx).toasts.clone();

        if toasts.is_empty() {
            return div().into_any_element();
        }

        let toast_state = self.toast_state.clone();

        let mut container = div()
            .absolute()
            .bottom(px(16.))
            .right(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.));

        for item in toasts {
            let ts = toast_state.clone();
            let item_id = item.id.clone();
            let icon_char = match item.kind {
                ToastKind::Info => "ⓘ",
                ToastKind::Success => "✓",
                ToastKind::Error => "✕",
            };
            let icon_color = match item.kind {
                ToastKind::Info => t.primary,
                ToastKind::Success => t.status_2xx,
                ToastKind::Error => t.status_5xx,
            };

            let mut card = div()
                .w(px(340.))
                .p(px(12.))
                .rounded(px(8.))
                .bg(t.bg_card)
                .border_1()
                .border_color(t.border)
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(div().text_sm().text_color(icon_color).child(icon_char))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_primary)
                                        .child(item.title.clone()),
                                ),
                        )
                        .child(
                            div()
                                .id(format!("dismiss-{}", item.id))
                                .cursor_pointer()
                                .text_color(t.text_muted)
                                .hover(|s| s.text_color(t.text_primary))
                                .on_click(move |_, _, cx| {
                                    ts.update(cx, |state, cx| {
                                        state.dismiss_toast(&item_id, cx);
                                    });
                                })
                                .child("×"),
                        ),
                );

            if !item.message.is_empty() {
                card = card.child(
                    div()
                        .text_xs()
                        .text_color(t.text_secondary)
                        .child(item.message.clone()),
                );
            }

            if let Some(label) = item.action_label {
                let action_cb = item.action_fn.clone();
                let ts_close = toast_state.clone();
                let close_id = item.id.clone();

                card = card.child(
                    div().flex().gap(px(8.)).child(
                        div()
                            .id(format!("act-{}", item.id))
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(t.primary)
                            .text_color(t.primary_foreground)
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(t.primary_hover))
                            .on_click(move |_, _, cx| {
                                if let Some(cb) = &action_cb {
                                    cb();
                                }
                                ts_close.update(cx, |state, cx| {
                                    state.dismiss_toast(&close_id, cx);
                                });
                            })
                            .child(label),
                    ),
                );
            }

            container = container.child(card);
        }

        container.into_any_element()
    }
}
