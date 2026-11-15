use crate::app::AppView;
use crate::theme::theme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::Sizable as _;
use gpui_kit::{div, px, AnyElement, IntoElement, ParentElement, Styled};

pub(super) fn conflict_banner(av: &AppView, req_id: &str, msg: String) -> AnyElement {
    let t = theme();
    let per_rel = av.state.persistence.clone();
    let per_keep = av.state.persistence.clone();
    let ws_rel = av.state.workspace.clone();
    let ws_keep = av.state.workspace.clone();
    let cid = req_id.to_string();
    let cid_k = req_id.to_string();
    div()
        .px(px(12.))
        .py(px(6.))
        .bg(gpui_kit::Hsla {
            a: 0.15,
            ..t.status_4xx
        })
        .border_b_1()
        .border_color(t.status_4xx)
        .flex()
        .items_center()
        .justify_between()
        .text_xs()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(t.status_4xx)
                        .child("! Conflict:"),
                )
                .child(div().text_color(t.text_primary).child(msg)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    Button::new("btn-conflict-reload")
                        .primary()
                        .small()
                        .label("Reload")
                        .on_click(move |_, _, cx| {
                            per_rel.update(cx, |per, cx| {
                                per.resolve_conflict_reload(&cid, ws_rel.clone(), cx);
                            });
                        }),
                )
                .child(
                    Button::new("btn-conflict-keep")
                        .secondary()
                        .small()
                        .label("Keep mine")
                        .on_click(move |_, _, cx| {
                            per_keep.update(cx, |per, cx| {
                                per.resolve_conflict_keep(&cid_k, ws_keep.clone(), cx);
                            });
                        }),
                ),
        )
        .into_any_element()
}

pub(super) fn parse_error_banner(err: String) -> AnyElement {
    let t = theme();
    div()
        .px(px(12.))
        .py(px(6.))
        .bg(gpui_kit::Hsla {
            a: 0.12,
            ..t.status_5xx
        })
        .border_b_1()
        .border_color(t.status_5xx)
        .flex()
        .items_center()
        .gap(px(8.))
        .text_xs()
        .text_color(t.status_5xx)
        .child("! Parse error on disk (showing last good in-memory version):")
        .child(
            div()
                .font_family("monospace")
                .text_color(t.text_primary)
                .child(err),
        )
        .into_any_element()
}

pub(super) fn corrupt_secrets_banner(msg: String) -> AnyElement {
    let t = theme();
    div()
        .px(px(12.))
        .py(px(6.))
        .bg(gpui_kit::Hsla {
            a: 0.15,
            ..t.status_4xx
        })
        .border_b_1()
        .border_color(t.status_4xx)
        .flex()
        .items_center()
        .gap(px(8.))
        .text_xs()
        .text_color(t.status_4xx)
        .child(format!("! Sensitive Data Warning: {}", msg))
        .into_any_element()
}
