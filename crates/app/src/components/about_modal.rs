use crate::components::ModalDialog;
use crate::state::{ModalState, ToastState, UpdateStatus};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::{
    div, img, px, App, Entity, FontWeight, Image, ImageFormat, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement, RenderOnce, Styled, Window,
};
use std::sync::{Arc, OnceLock};

fn about_app_icon() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image {
            format: ImageFormat::Png,
            bytes: include_bytes!("../../resources/app_icon_128.png").to_vec(),
            id: 2,
        })
    })
    .clone()
}

#[derive(IntoElement)]
pub struct AboutModal {
    modal_state: Entity<ModalState>,
    toast_state: Entity<ToastState>,
}

impl AboutModal {
    pub fn new(modal_state: Entity<ModalState>, toast_state: Entity<ToastState>) -> Self {
        Self {
            modal_state,
            toast_state,
        }
    }
}

impl RenderOnce for AboutModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let modal_clone = self.modal_state.clone();
        let toast_clone = self.toast_state;

        let version = crate::version::APP_VERSION;
        let commit_hash = crate::version::COMMIT_HASH;
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let platform_label = format!("{os} ({arch})");
        let update_status = modal_clone.read(cx).update_status.clone();

        ModalDialog::new("about-modal")
            .w(px(460.))
            .title("About Zest")
            .icon(IconName::Info)
            .on_close({
                let modal_s = modal_clone.clone();
                move |_, cx| {
                    modal_s.update(cx, |state, cx| {
                        state.close_about_modal(cx);
                    });
                }
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .p(px(24.))
                    .gap(px(16.))
                    // App Icon and Hero Heading
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .w(px(72.))
                                    .h(px(72.))
                                    .rounded(px(16.))
                                    .bg(t.bg_app)
                                    .border_1()
                                    .border_color(t.border)
                                    .shadow_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .overflow_hidden()
                                    .child(
                                        img(about_app_icon())
                                            .w(px(72.))
                                            .h(px(72.))
                                            .rounded(px(16.)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text_primary)
                                    .child("Zest"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child("High-performance macOS-native REST & API client"),
                            )
                            // Version Chip with Copy Action
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .px(px(10.))
                                    .py(px(3.))
                                    .rounded(px(12.))
                                    .bg(t.bg_app)
                                    .border_1()
                                    .border_color(t.border)
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(t.primary)
                                            .child(format!("v{version}")),
                                    )
                                    .child({
                                        let toast_c = toast_clone;
                                        let plat_c = platform_label.clone();
                                        Button::new("btn-copy-version")
                                            .ghost()
                                            .compact()
                                            .occlude()
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .w(px(20.))
                                            .h(px(20.))
                                            .tooltip("Copy Version")
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(
                                                    gpui_kit::ClipboardItem::new_string(format!(
                                                        "Zest v{version} ({commit_hash}) - {plat_c}"
                                                    )),
                                                );
                                                toast_c.update(cx, |t, cx| {
                                                    t.push_success(
                                                        "Copied",
                                                        "Version info copied to clipboard",
                                                        cx,
                                                    );
                                                });
                                            })
                                            .child(Icon::new(IconName::Copy).size(px(12.)))
                                    }),
                            ),
                    )
                    // Update Checker Card
                    .child(render_update_section(&modal_clone, &update_status))
                    // Details Table Card
                    .child(
                        div()
                            .w_full()
                            .rounded(px(8.))
                            .bg(t.bg_app)
                            .border_1()
                            .border_color(t.border)
                            .p(px(12.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(render_meta_row("Version", version, t.text_primary))
                            .child(render_meta_row("Commit", commit_hash, t.text_secondary))
                            .child(render_meta_row(
                                "Platform",
                                &platform_label,
                                t.text_secondary,
                            ))
                            .child(render_meta_row(
                                "Engine",
                                "GPUI (GPU-Accelerated)",
                                t.text_secondary,
                            ))
                            .child(render_meta_row(
                                "Architecture",
                                "Local-first YAML & Git VCS",
                                t.text_secondary,
                            )),
                    )
                    // Footer Actions
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pt(px(4.))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child("MIT License • Built with Rust & GPUI"),
                            )
                            .child({
                                let modal_s = modal_clone;
                                Button::new("btn-about-close-footer")
                                    .secondary()
                                    .compact()
                                    .px(px(14.))
                                    .py(px(4.))
                                    .on_click(move |_, _, cx| {
                                        modal_s.update(cx, |state, cx| {
                                            state.close_about_modal(cx);
                                        });
                                    })
                                    .child("Close")
                            }),
                    ),
            )
            .into_any_element()
    }
}

pub fn render_update_section(
    modal_state: &Entity<ModalState>,
    status: &UpdateStatus,
) -> impl IntoElement {
    let t = theme();
    let modal_s = modal_state.clone();

    div()
        .w_full()
        .rounded(px(8.))
        .bg(t.bg_app)
        .border_1()
        .border_color(t.border)
        .p(px(10.))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(10.))
        .child(match status {
            UpdateStatus::Idle => {
                let m = modal_s;
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Icon::new(IconName::CloudDownload)
                                    .size(px(14.))
                                    .text_color(t.text_secondary),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_secondary)
                                    .child("Software Updates"),
                            ),
                    )
                    .child(
                        Button::new("btn-check-updates")
                            .secondary()
                            .compact()
                            .px(px(10.))
                            .py(px(3.))
                            .h(px(24.))
                            .on_click(move |_, _, cx| {
                                m.update(cx, |state, cx| {
                                    state.check_for_updates(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text_primary)
                                    .child("Check for Updates"),
                            ),
                    )
                    .into_any_element()
            }
            UpdateStatus::Checking => div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(Spinner::new().with_size(px(12.)).color(t.primary))
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Checking GitHub for updates..."),
                )
                .into_any_element(),
            UpdateStatus::UpToDate { checked_version } => {
                let m = modal_s;
                let ver = checked_version.clone();
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Icon::new(IconName::CircleCheck)
                                    .size(px(14.))
                                    .text_color(t.status_2xx),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text_secondary)
                                    .child(format!("Zest v{ver} is up to date")),
                            ),
                    )
                    .child(
                        Button::new("btn-recheck-updates")
                            .ghost()
                            .compact()
                            .px(px(8.))
                            .py(px(2.))
                            .h(px(22.))
                            .on_click(move |_, _, cx| {
                                m.update(cx, |state, cx| {
                                    state.check_for_updates(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child("Check again"),
                            ),
                    )
                    .into_any_element()
            }
            UpdateStatus::UpdateAvailable {
                latest_version,
                release_url,
                ..
            } => {
                let url = release_url.clone();
                let tag = latest_version.clone();
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Icon::new(IconName::ArrowDown)
                                    .size(px(14.))
                                    .text_color(t.primary),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.primary)
                                    .child(format!("New version available: {tag}")),
                            ),
                    )
                    .child(
                        Button::new("btn-download-update")
                            .primary()
                            .compact()
                            .px(px(10.))
                            .py(px(3.))
                            .h(px(24.))
                            .on_click(move |_, _, _| {
                                crate::state::open_browser_url(&url);
                            })
                            .child(Icon::new(IconName::Globe).size(px(12.)))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("View Release"),
                            ),
                    )
                    .into_any_element()
            }
            UpdateStatus::Error(err) => {
                let m = modal_s;
                let err_msg = err.clone();
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Icon::new(IconName::CircleX)
                                    .size(px(14.))
                                    .text_color(t.status_5xx),
                            )
                            .child(div().text_xs().text_color(t.status_5xx).child(
                                if err_msg.len() > 30 {
                                    format!("Check failed: {}...", &err_msg[..27])
                                } else {
                                    format!("Check failed: {}", err_msg)
                                },
                            )),
                    )
                    .child(
                        Button::new("btn-retry-updates")
                            .ghost()
                            .compact()
                            .px(px(8.))
                            .py(px(2.))
                            .h(px(22.))
                            .on_click(move |_, _, cx| {
                                m.update(cx, |state, cx| {
                                    state.check_for_updates(cx);
                                });
                            })
                            .child(div().text_xs().text_color(t.text_muted).child("Retry")),
                    )
                    .into_any_element()
            }
        })
}

fn render_meta_row(
    label: &'static str,
    value: &str,
    val_color: gpui_kit::Hsla,
) -> impl IntoElement {
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
                .child(value.to_string()),
        )
}
