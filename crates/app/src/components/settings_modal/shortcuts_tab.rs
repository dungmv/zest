use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, IntoElement, ParentElement, Styled};

pub fn render_shortcuts_tab() -> impl IntoElement {
    let t = theme();
    let is_macos = cfg!(target_os = "macos");

    let shortcuts: [(&'static str, &'static str, &'static str); 7] = [
        (
            "Send Active Request",
            "Execute the current HTTP request",
            if is_macos {
                "⌘ + Enter"
            } else {
                "Ctrl + Enter"
            },
        ),
        (
            "Command Palette",
            "Quickly search requests, environments, and commands",
            if is_macos { "⌘ + K" } else { "Ctrl + K" },
        ),
        (
            "Toggle Sidebar",
            "Collapse or expand the left collection tree",
            if is_macos { "⌘ + B" } else { "Ctrl + B" },
        ),
        (
            "Open Settings",
            "Open this Settings & Preferences dialog",
            if is_macos { "⌘ + ," } else { "Ctrl + ," },
        ),
        (
            "Git Source Control",
            "Open Git staging, commit, and branch manager",
            if is_macos {
                "⌘ + Shift + G"
            } else {
                "Ctrl + Shift + G"
            },
        ),
        (
            "Import Collection / Spec",
            "Import OpenAPI 3.0/3.1 or Postman collection",
            if is_macos { "⌘ + I" } else { "Ctrl + I" },
        ),
        (
            "Dismiss Modal / Cancel",
            "Close the topmost modal or palette",
            "Esc",
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
                            .child(
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
                                    .child(keybinding),
                            )
                    },
                )),
        )
}
