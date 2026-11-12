use super::ResponseInspector;
use crate::theme::theme;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};
use std::sync::Arc;
use zest_core::models::HttpResponseRecord;

pub(super) fn render_cookies_tab(
    ri: &ResponseInspector,
    cx: &App,
    is_sending_this_request: bool,
    current_resp: &Option<Arc<HttpResponseRecord>>,
) -> AnyElement {
    let t = theme();
    let resp_s = ri.response_state.read(cx);
    let ed_s = ri.editor_state.read(cx);
    let _active_id = ed_s.active_request_id.clone();
    let _is_viewing_example = resp_s.active_example_name.is_some();
    let _view_mode = resp_s.view_mode;
    let _resp_state_clone = ri.response_state.clone();
    let current_resp = current_resp.clone();
    if is_sending_this_request {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(Spinner::new().with_size(px(20.)).color(t.primary))
            .child(
                div()
                    .text_xs()
                    .text_color(t.text_secondary)
                    .child("Sending request... Response cookies will appear here."),
            )
            .into_any_element()
    } else if let Some(resp) = &current_resp {
        let parsed_cookies = resp.parsed_cookies();
        if parsed_cookies.is_empty() {
            div()
                .p(px(12.))
                .text_xs()
                .text_color(t.text_muted)
                .child("No cookies received in this response.")
                .into_any_element()
        } else {
            div()
                .size_full()
                .overflow_y_scrollbar()
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(
                    parsed_cookies
                        .into_iter()
                        .enumerate()
                        .map(|(c_idx, cookie)| {
                            let cookie_val_tip = cookie.value.clone();
                            div()
                                .p(px(8.))
                                .rounded(px(6.))
                                .bg(t.bg_card)
                                .border_1()
                                .border_color(t.border)
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(t.primary)
                                                .child(cookie.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(4.))
                                                .when(cookie.secure, |this| {
                                                    this.child(
                                                        div()
                                                            .px(px(4.))
                                                            .rounded(px(2.))
                                                            .bg(t.success)
                                                            .text_color(t.primary_foreground)
                                                            .text_xs()
                                                            .child("Secure"),
                                                    )
                                                })
                                                .when(cookie.http_only, |this| {
                                                    this.child(
                                                        div()
                                                            .px(px(4.))
                                                            .rounded(px(2.))
                                                            .bg(t.primary)
                                                            .text_color(t.primary_foreground)
                                                            .text_xs()
                                                            .child("HttpOnly"),
                                                    )
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(format!("cookie-val-{}", c_idx))
                                        .font_family(".SystemMonoFont")
                                        .text_xs()
                                        .text_color(t.text_primary)
                                        .truncate()
                                        .tooltip(move |window, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(
                                                cookie_val_tip.clone(),
                                            )
                                            .build(window, cx)
                                        })
                                        .child(cookie.value.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(12.))
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .when_some(cookie.domain, |this, d| {
                                            this.child(format!("Domain: {}", d))
                                        })
                                        .when_some(cookie.path, |this, p| {
                                            this.child(format!("Path: {}", p))
                                        })
                                        .when_some(cookie.expires, |this, exp| {
                                            this.child(format!("Expires: {}", exp))
                                        }),
                                )
                        }),
                )
                .into_any_element()
        }
    } else {
        div()
            .p(px(12.))
            .text_xs()
            .text_color(t.text_muted)
            .child("No cookies received.")
            .into_any_element()
    }
}
