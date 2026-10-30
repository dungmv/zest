use crate::state::ModalState;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};

pub fn render_network_tab(modal_state: &Entity<ModalState>, cx: &App) -> impl IntoElement {
    let t = theme();
    let modal_clone = modal_state.clone();
    let modal_ref = modal_clone.read(cx);
    let cur_timeout = modal_ref.request_timeout_secs;
    let cur_follow = modal_ref.follow_redirects;
    let cur_tls = modal_ref.validate_tls;

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(16.))
        // Section Header
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
                        .child("NETWORK & EXECUTION"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Configure global timeout, redirect policy, and TLS verification for HTTP requests."),
                ),
        )
        // 1. Timeout presets
        .child({
            let modal_set = modal_clone.clone();
            div()
                .w_full()
                .min_w_0()
                .p(px(12.))
                .rounded(px(6.))
                .bg(t.bg_app)
                .border_1()
                .border_color(t.border)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .child("Request Timeout"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_muted)
                                .child("Maximum time to wait before terminating the HTTP request."),
                        ),
                )
                .child(
                    div().flex_shrink_0().child(
                        Button::new("btn-timeout-dropdown")
                            .secondary()
                            .compact()
                            .w(px(88.))
                            .px(px(8.))
                            .py(px(4.))
                            .h(px(28.))
                            .rounded(px(5.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        Icon::new(IconName::Clock)
                                            .size(px(13.))
                                            .text_color(t.text_secondary),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                            .text_color(t.text_primary)
                                            .child(format!("{}s", cur_timeout)),
                                    ),
                            )
                            .child(
                                Icon::new(IconName::ChevronDown)
                                    .size(px(12.))
                                    .text_color(t.text_muted),
                            )
                            .dropdown_menu(move |menu, _window, _cx| {
                                [10, 30, 60, 120].into_iter().fold(menu, |m, s| {
                                    let modal_item = modal_set.clone();
                                    m.item(
                                        PopupMenuItem::new(format!("{}s", s))
                                            .checked(cur_timeout == s)
                                            .on_click(move |_, _, cx| {
                                                modal_item.update(cx, |st, cx| {
                                                    st.set_request_timeout_secs(s, cx);
                                                });
                                            }),
                                    )
                                })
                            }),
                    ),
                )
        })
        // 2. Follow Redirects toggle
        .child({
            let modal_set = modal_clone.clone();
            div()
                .w_full()
                .min_w_0()
                .p(px(12.))
                .rounded(px(6.))
                .bg(t.bg_app)
                .border_1()
                .border_color(t.border)
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
                        .gap(px(3.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .child("Follow HTTP Redirects"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_muted)
                                .child("Automatically follow 301, 302, 307, and 308 redirects."),
                        ),
                )
                .child(
                    Switch::new("toggle-follow-redirects")
                        .checked(cur_follow)
                        .on_click(move |&next_val, _, cx| {
                            modal_set.update(cx, |st, cx| {
                                st.set_follow_redirects(next_val, cx);
                            });
                        }),
                )
        })
        // 3. Validate TLS Certificates toggle
        .child({
            let modal_set = modal_clone;
            div()
                .w_full()
                .min_w_0()
                .p(px(12.))
                .rounded(px(6.))
                .bg(t.bg_app)
                .border_1()
                .border_color(t.border)
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
                        .gap(px(3.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .child("Validate SSL / TLS Certificates"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_muted)
                                .child("Verify SSL certificate chain. Disable for local development or self-signed certs."),
                        ),
                )
                .child(
                    Switch::new("toggle-validate-tls")
                        .checked(cur_tls)
                        .on_click(move |&next_val, _, cx| {
                            modal_set.update(cx, |st, cx| {
                                st.set_validate_tls(next_val, cx);
                            });
                        }),
                )
        })
}
