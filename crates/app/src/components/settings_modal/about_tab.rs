use crate::components::about_modal::render_update_section;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::Icon;
use gpui_kit::{
    div, img, px, App, Entity, FontWeight, Image, ImageFormat, IntoElement, ParentElement, Styled,
};
use std::sync::{Arc, OnceLock};

fn settings_about_app_icon() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image {
            format: ImageFormat::Png,
            bytes: include_bytes!("../../../resources/app_icon_128.png").to_vec(),
            id: 3,
        })
    })
    .clone()
}

pub fn render_about_tab(
    modal_state: &Entity<crate::state::ModalState>,
    cx: &App,
) -> impl IntoElement {
    let t = theme();
    let modal_clone = modal_state.clone();

    let version = crate::version::APP_VERSION;
    let commit_hash = crate::version::COMMIT_HASH;
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let platform_label = format!("{os} ({arch})");
    let update_status = modal_clone.read(cx).update_status.clone();

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(20.))
        // Section: About Zest
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(t.text_secondary)
                        .child("APPLICATION INFO"),
                )
                // Card with App Info
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .p(px(16.))
                        .rounded(px(6.))
                        .bg(t.bg_app)
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(14.))
                                .child(
                                    div()
                                        .w(px(52.))
                                        .h(px(52.))
                                        .rounded(px(12.))
                                        .bg(t.bg_card)
                                        .border_1()
                                        .border_color(t.border)
                                        .overflow_hidden()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            img(settings_about_app_icon())
                                                .w(px(52.))
                                                .h(px(52.))
                                                .rounded(px(12.)),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(t.text_primary)
                                                .child("Zest"),
                                        )
                                        .child(div().text_xs().text_color(t.text_muted).child(
                                            "High-performance macOS-native REST & API client",
                                        ))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(t.primary)
                                                .child(format!("Version {version}")),
                                        ),
                                )
                                .child({
                                    let modal_s = modal_clone.clone();
                                    Button::new("btn-open-about-popup")
                                        .secondary()
                                        .compact()
                                        .px(px(10.))
                                        .py(px(4.))
                                        .h(px(28.))
                                        .rounded(px(5.))
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |state, cx| {
                                                state.close_settings_modal(cx);
                                                state.open_about_modal(cx);
                                            });
                                        })
                                        .child(
                                            Icon::new(IconName::Info)
                                                .size(px(13.))
                                                .text_color(t.text_secondary),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(t.text_primary)
                                                .child("About Popup..."),
                                        )
                                }),
                        ),
                ),
        )
        // Section: Software Updates
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(t.text_secondary)
                        .child("SOFTWARE UPDATES"),
                )
                .child(render_update_section(&modal_clone, &update_status)),
        )
        // Section: System & Engine Details
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(t.text_secondary)
                        .child("BUILD & SYSTEM SPECIFICATIONS"),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .p(px(14.))
                        .rounded(px(6.))
                        .bg(t.bg_app)
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(render_spec_row("App Version", version, t.text_primary))
                        .child(render_spec_row("Git Commit", commit_hash, t.text_secondary))
                        .child(render_spec_row(
                            "Target Platform",
                            &platform_label,
                            t.text_secondary,
                        ))
                        .child(render_spec_row(
                            "Rendering Engine",
                            "GPUI GPU-Accelerated (60/120 FPS)",
                            t.text_secondary,
                        ))
                        .child(render_spec_row(
                            "Storage Architecture",
                            "Local-first YAML with Write-behind Coalescing",
                            t.text_secondary,
                        ))
                        .child(render_spec_row(
                            "Version Control",
                            "Integrated Git Subprocess Engine",
                            t.text_secondary,
                        )),
                ),
        )
}

fn render_spec_row(label: &'static str, val: &str, val_color: gpui_kit::Hsla) -> impl IntoElement {
    let t = theme();
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .text_xs()
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_muted)
                .child(label),
        )
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .font_family(".SystemMonoFont")
                .text_color(val_color)
                .child(val.to_string()),
        )
}
