use crate::theme::theme;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, IntoElement, Keystroke, ParentElement, Styled};

pub fn render_shortcuts_tab() -> impl IntoElement {
    let t = theme();

    let shortcuts: [(&'static str, &'static str, &'static str); 7] = [
        (
            "Send Active Request",
            "Execute the current HTTP request",
            "cmd-enter",
        ),
        (
            "Command Palette",
            "Quickly search requests, environments, and commands",
            "cmd-k",
        ),
        (
            "Toggle Sidebar",
            "Collapse or expand the left collection tree",
            "cmd-b",
        ),
        (
            "Open Settings",
            "Open this Settings & Preferences dialog",
            "cmd-,",
        ),
        (
            "Git Source Control",
            "Open Git staging, commit, and branch manager",
            "cmd-shift-g",
        ),
        (
            "Import Collection / Spec",
            "Import OpenAPI 3.0/3.1 or Postman collection",
            "cmd-i",
        ),
        (
            "Dismiss Modal / Cancel",
            "Close the topmost modal or palette",
            "escape",
        ),
    ];

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(t.text_secondary)
                        .child("KEYBOARD SHORTCUTS"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Quick keyboard bindings for common workspace and request actions."),
                ),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .rounded(px(6.))
                .bg(t.bg_app)
                .border_1()
                .border_color(t.border)
                .flex()
                .flex_col()
                .children(shortcuts.into_iter().enumerate().map(
                    |(idx, (title, desc, keybinding))| {
                        div()
                            .w_full()
                            .min_w_0()
                            .px(px(14.))
                            .py(px(10.))
                            .border_color(t.border_subtle)
                            .when(idx > 0, |el| el.border_t_1())
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(t.text_primary)
                                            .child(title),
                                    )
                                    .child(div().text_xs().text_color(t.text_muted).child(desc)),
                            )
                            .child(if let Ok(stroke) = Keystroke::parse(keybinding) {
                                Kbd::new(stroke).into_any_element()
                            } else {
                                div()
                                    .flex_shrink_0()
                                    .px(px(8.))
                                    .py(px(3.))
                                    .rounded(px(4.))
                                    .bg(t.bg_input)
                                    .border_1()
                                    .border_color(t.border)
                                    .text_xs()
                                    .font_family(".SystemMonoFont")
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .text_color(t.text_primary)
                                    .child(keybinding)
                                    .into_any_element()
                            })
                    },
                )),
        )
}
