use super::ResponseInspector;
use crate::components::status_badge::MethodBadge;
use crate::theme::theme;
use gpui_kit::component::input::Editor;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::text::TextView;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, AnyElement, App, IntoElement, ParentElement, Styled};
use std::sync::Arc;
use zest_core::models::HttpResponseRecord;

pub(super) fn render_request_sent_tab(
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
                    .child("Sending request... Outgoing details will be recorded once sent."),
            )
            .into_any_element()
    } else if let Some(resp) = &current_resp {
        if let Some(sent) = &resp.request_sent {
            let escape_md = |s: &str| -> String {
                s.replace('\\', "\\\\")
                    .replace('*', "\\*")
                    .replace('_', "\\_")
                    .replace('`', "\\`")
                    .replace('<', "\\<")
                    .replace('>', "\\>")
            };
            let url_md = escape_md(&sent.url);
            let headers_md = if sent.headers.is_empty() {
                "*(No headers sent)*".to_string()
            } else {
                sent.headers
                    .iter()
                    .map(|(k, v)| format!("**{}:** {}", escape_md(k), escape_md(v)))
                    .collect::<Vec<_>>()
                    .join("  \n")
            };

            div()
                .size_full()
                .overflow_y_scrollbar()
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(12.))
                // URL Section
                .child(
                    div()
                        .p(px(10.))
                        .bg(t.bg_card)
                        .rounded(px(6.))
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(MethodBadge::new(sent.method))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Request URL"),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .p(px(6.))
                                .bg(t.bg_editor)
                                .rounded(px(4.))
                                .border_1()
                                .border_color(t.border_subtle)
                                .font_family(".SystemMonoFont")
                                .text_xs()
                                .child(
                                    TextView::markdown("sent-request-url", url_md).selectable(true),
                                ),
                        ),
                )
                // Headers Sent Section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_muted)
                                .child(format!("Headers Sent ({}):", sent.headers.len())),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .p(px(8.))
                                .bg(t.bg_card)
                                .rounded(px(6.))
                                .border_1()
                                .border_color(t.border)
                                .font_family(".SystemMonoFont")
                                .text_xs()
                                .child(
                                    TextView::markdown("sent-headers-view", headers_md)
                                        .selectable(true),
                                ),
                        ),
                )
                // Body Sent Section
                .when_some(sent.body_preview.clone(), |this, _preview| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(t.text_muted)
                                    .child(format!("Body Sent ({} bytes):", sent.body_size_bytes)),
                            )
                            .child(
                                div()
                                    .h(px(260.))
                                    .bg(t.bg_card)
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(t.border)
                                    .overflow_hidden()
                                    .child(
                                        Editor::new(&ri.request_body_editor)
                                            .readonly(true)
                                            .appearance(false)
                                            .bordered(false)
                                            .h(gpui_kit::relative(1.)),
                                    ),
                            ),
                    )
                })
                .into_any_element()
        } else {
            div()
                .p(px(12.))
                .text_xs()
                .text_color(t.text_muted)
                .child("No request sent info available.")
                .into_any_element()
        }
    } else {
        div()
            .p(px(12.))
            .text_xs()
            .text_color(t.text_muted)
            .child("No request sent yet.")
            .into_any_element()
    }
}
