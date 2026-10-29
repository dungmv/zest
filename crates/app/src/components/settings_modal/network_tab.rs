use crate::state::ModalState;
use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};
use gpui_kit::component::Icon;
use gpui_kit::assets::IconName;

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
        .child(
            div()
                .w_full()
                .min_w_0()
                .p(px(12.))
                .rounded(px(6.))
                .bg(t.bg_app)
                .border_1()
                .border_color(t.border)
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .justify_between()
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
                                .font_family(".SystemMonoFont")
                                .text_color(t.primary)
                                .child(format!("{}s", cur_timeout)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Maximum time to wait before terminating the HTTP request."),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .pt(px(2.))
                        .children([10, 30, 60, 120].into_iter().map(|s| {
                            let is_cur = cur_timeout == s;
                            let modal_set = modal_clone.clone();
                            div()
                                .id(format!("timeout-{}s", s))
                                .px(px(14.))
                                .py(px(5.))
                                .rounded(px(4.))
                                .bg(if is_cur { t.primary } else { t.bg_input })
                                .border_1()
                                .border_color(if is_cur { t.primary } else { t.border })
                                .text_color(if is_cur {
                                    t.primary_foreground
                                } else {
                                    t.text_secondary
                                })
                                .cursor_pointer()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .hover(|st| {
                                    if !is_cur {
                                        st.bg(t.bg_hover).text_color(t.text_primary)
                                    } else {
                                        st
                                    }
                                })
                                .on_click(move |_, _, cx| {
                                    modal_set.update(cx, |st, cx| {
                                        st.set_request_timeout_secs(s, cx);
                                    });
                                })
                                .child(format!("{}s", s))
                        })),
                ),
        )
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
                    div()
                        .id("toggle-follow-redirects")
                        .flex_shrink_0()
                        .px(px(12.))
                        .py(px(5.))
                        .rounded(px(4.))
                        .bg(if cur_follow { t.primary } else { t.bg_input })
                        .border_1()
                        .border_color(if cur_follow { t.primary } else { t.border })
                        .text_color(if cur_follow {
                            t.primary_foreground
                        } else {
                            t.text_muted
                        })
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .hover(|s| {
                            if !cur_follow {
                                s.bg(t.bg_hover).text_color(t.text_primary)
                            } else {
                                s
                            }
                        })
                        .on_click(move |_, _, cx| {
                            modal_set.update(cx, |st, cx| {
                                st.set_follow_redirects(!cur_follow, cx);
                            });
                        })
                        .when(cur_follow, |this| {
                            this.child(Icon::new(IconName::Check).size(px(12.)))
                        })
                        .child(if cur_follow { "Enabled" } else { "Disabled" }),
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
                    div()
                        .id("toggle-validate-tls")
                        .flex_shrink_0()
                        .px(px(12.))
                        .py(px(5.))
                        .rounded(px(4.))
                        .bg(if cur_tls { t.primary } else { t.bg_input })
                        .border_1()
                        .border_color(if cur_tls { t.primary } else { t.border })
                        .text_color(if cur_tls {
                            t.primary_foreground
                        } else {
                            t.text_muted
                        })
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .hover(|s| {
                            if !cur_tls {
                                s.bg(t.bg_hover).text_color(t.text_primary)
                            } else {
                                s
                            }
                        })
                        .on_click(move |_, _, cx| {
                            modal_set.update(cx, |st, cx| {
                                st.set_validate_tls(!cur_tls, cx);
                            });
                        })
                        .when(cur_tls, |this| {
                            this.child(Icon::new(IconName::Check).size(px(12.)))
                        })
                        .child(if cur_tls { "Enabled" } else { "Disabled" }),
                )
        })
}
