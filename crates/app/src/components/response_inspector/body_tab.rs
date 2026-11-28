use super::body_tab_toolbar::{render_body_toolbar, BodyToolbarProps};
use super::{preview_image, ResponseInspector};
use crate::editor_scheme::editor_scheme;
use crate::state::ResponseViewMode;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, Input, InputState};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::AppContext as _;
use gpui_kit::{
    div, px, uniform_list, AnyElement, App, Div, Entity, IntoElement, ParentElement, Styled, Window,
};
use std::sync::Arc;
use zest_core::models::{HttpResponseRecord, ResponsePreviewKind};

pub(super) fn render_body_tab(
    ri: &ResponseInspector,
    window: &mut Window,
    cx: &mut App,
    is_sending_this_request: bool,
    current_resp: &Option<Arc<HttpResponseRecord>>,
) -> AnyElement {
    let active_id = ri.editor_state.read(cx).active_request_id.clone();

    if is_sending_this_request {
        return render_sending_state(ri, cx);
    }
    let Some(c_resp) = current_resp.clone() else {
        return render_waiting_state();
    };

    let jsonpath_input = use_jsonpath_input(ri, window, cx);
    let resp_s = ri.response_state.read(cx);
    let view_mode = resp_s.view().mode;
    let body_len = c_resp.body.len();

    let (lines, is_json, is_binary, is_truncated) = if let Some(fmt) = &resp_s.view().formatted_body
    {
        (
            fmt.lines.clone(),
            fmt.is_json,
            fmt.is_binary,
            fmt.is_truncated,
        )
    } else {
        (Arc::new(Vec::new()), false, false, false)
    };

    let is_large_payload = is_truncated || body_len > 10 * 1024 * 1024 || lines.len() > 20000;
    let preview_kind = resp_s
        .view()
        .formatted_body
        .as_ref()
        .and_then(|f| f.preview_kind)
        .or_else(|| c_resp.preview_kind());
    let is_preview_mode = matches!(view_mode, ResponseViewMode::Preview) && preview_kind.is_some();
    let is_raster_image = matches!(
        preview_kind,
        Some(ResponsePreviewKind::Image(fmt)) if fmt.is_raster()
    );

    // Query bar is offered only when the body parsed as JSON (the
    // retained parsed value in CachedResponseFormat is what gets queried).
    let jsonpath_active = !is_preview_mode
        && resp_s
            .view()
            .formatted_body
            .as_ref()
            .and_then(|f| f.cached_format.as_ref())
            .is_some_and(|cf| cf.parsed_json.is_some());
    let show_jsonpath_bar = resp_s.view().show_jsonpath_bar;
    let jsonpath_matches = resp_s
        .view()
        .formatted_body
        .as_ref()
        .and_then(|f| f.match_count);
    let is_viewing_example = resp_s.active_example_name.is_some();
    let wrap_lines = resp_s.view().wrap_lines;

    let toolbar = render_body_toolbar(
        ri,
        BodyToolbarProps {
            active_id,
            current_resp: c_resp.clone(),
            view_mode,
            preview_kind,
            is_preview_mode,
            is_viewing_example,
            is_raster_image,
            is_binary,
            is_large_payload,
            wrap_lines,
            jsonpath_active,
            show_jsonpath_bar,
            jsonpath_input: jsonpath_input.clone(),
        },
    );

    let content = render_body_content(
        ri,
        &c_resp,
        lines,
        is_json,
        is_large_payload,
        is_preview_mode,
        preview_kind,
        cx,
    );

    let t = theme();
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(toolbar)
        .when(jsonpath_active && show_jsonpath_bar, |this| {
            this.child(render_jsonpath_bar(
                &ri.response_state,
                &jsonpath_input,
                jsonpath_matches,
            ))
        })
        .when(!is_preview_mode && is_large_payload, |this| {
            this.child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .bg(t.var_unresolved.opacity(0.12))
                    .border_b_1()
                    .border_color(t.var_unresolved.opacity(0.3))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.var_unresolved)
                            .child(format!(
                                "⚠ Large payload ({}). Showing bounded preview — use \"Save to File\" for full content.",
                                c_resp.formatted_size()
                            )),
                    ),
            )
        })
        .child(content)
        .into_any_element()
}

fn render_sending_state(ri: &ResponseInspector, cx: &App) -> AnyElement {
    let t = theme();
    let progress_text = ri
        .editor_state
        .read(cx)
        .send_progress()
        .map(str::to_owned)
        .unwrap_or_else(|| "Waiting for response from server...".to_string());
    let ed_s_cancel = ri.editor_state.clone();

    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.))
                .child(
                    div()
                        .text_base()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(t.text_primary)
                        .child("Sending request..."),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_secondary)
                        .child(progress_text),
                ),
        )
        .child(
            Progress::new("response-send-progress")
                .loading(true)
                .w(px(240.))
                .color(t.primary),
        )
        .child(
            div()
                .w(px(240.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .py(px(4.))
                .child(Skeleton::new().h(px(8.)).rounded(px(4.)))
                .child(Skeleton::new().w(px(180.)).h(px(8.)).rounded(px(4.)))
                .child(Skeleton::new().w(px(210.)).h(px(8.)).rounded(px(4.))),
        )
        .child(
            Button::new("btn-cancel-in-response-view")
                .danger()
                .small()
                .label("Cancel Request")
                .on_click(move |_, _, cx: &mut App| {
                    ed_s_cancel.update(cx, |state, cx| {
                        state.cancel_send();
                        state.stop_sending(cx);
                    });
                }),
        )
        .into_any_element()
}

fn render_waiting_state() -> AnyElement {
    let t = theme();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .child(
            Icon::new(IconName::Globe)
                .size(px(14.))
                .text_color(t.icon_muted),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(t.text_secondary)
                .child("Waiting for response..."),
        )
        .child(
            div()
                .text_xs()
                .text_color(t.text_muted)
                .child("Press \"Send\" or Ctrl+Enter to execute this request"),
        )
        .into_any_element()
}

/// Keyed JSONPath query input synced bidirectionally with `ResponseState::ui.jsonpath_query`.
fn use_jsonpath_input(
    ri: &ResponseInspector,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    let resp_state_for_input = ri.response_state.clone();
    let jsonpath_input = window
        .use_keyed_state(
            gpui_kit::SharedString::from("response-jsonpath-input-holder"),
            cx,
            |window, cx| {
                let query_input = cx.new(|cx| {
                    InputState::new(window, cx).placeholder("JSONPath (e.g. $.data.items[*].id)")
                });
                let q_in = query_input.clone();
                let rs_c = resp_state_for_input;
                let sub = cx.subscribe_in(&query_input, window, move |_, _, event, _, cx| {
                    if let gpui_kit::component::input::InputEvent::Change = event {
                        let q = q_in.read(cx).value().to_string();
                        rs_c.update(cx, |state, cx| {
                            state.set_jsonpath_query(q, cx);
                        });
                    }
                });
                (query_input, sub)
            },
        )
        .read(cx)
        .0
        .clone();

    let state_query = ri.response_state.read(cx).view().jsonpath_query.clone();
    if jsonpath_input.read(cx).value().as_str() != state_query.as_str() {
        jsonpath_input.update(cx, |inp, cx| {
            inp.set_value(state_query.clone(), window, cx);
        });
    }
    jsonpath_input
}

fn render_jsonpath_bar(
    response_state: &Entity<crate::state::ResponseState>,
    jsonpath_input: &Entity<InputState>,
    match_count: Option<usize>,
) -> Div {
    let t = theme();
    let rs_close = response_state.clone();
    let jsonpath_hint = match_count.map(|n| {
        div()
            .flex_shrink_0()
            .text_xs()
            .text_color(t.text_muted)
            .child(if n == 1 {
                "1 match".to_string()
            } else {
                format!("{n} matches")
            })
    });

    div()
        .h(px(36.))
        .px(px(8.))
        .border_b_1()
        .border_color(t.border)
        .bg(t.bg_editor)
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h(px(26.))
                .px(px(8.))
                .rounded(px(6.))
                .bg(t.bg_input)
                .border_1()
                .border_color(t.border_subtle)
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    Icon::new(IconName::Funnel)
                        .size(px(13.))
                        .text_color(t.icon_muted),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(jsonpath_input)
                            .id("response-jsonpath-input")
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .cleanable(true),
                    ),
                ),
        )
        .children(jsonpath_hint)
        .child(
            Button::new("btn-close-jsonpath")
                .ghost()
                .xsmall()
                .icon(IconName::Close)
                .tooltip("Close filter")
                .on_click(move |_, _, cx: &mut App| {
                    rs_close.update(cx, |state, cx| {
                        state.close_jsonpath_bar(cx);
                    });
                }),
        )
}

#[allow(clippy::too_many_arguments)]
fn render_body_content(
    ri: &ResponseInspector,
    c_resp: &Arc<HttpResponseRecord>,
    lines: Arc<Vec<String>>,
    is_json: bool,
    is_large_payload: bool,
    is_preview_mode: bool,
    preview_kind: Option<ResponsePreviewKind>,
    cx: &App,
) -> Div {
    let scheme = editor_scheme();
    if let (true, Some(ResponsePreviewKind::Image(fmt))) = (is_preview_mode, preview_kind) {
        return div().flex_1().size_full().overflow_hidden().child(
            preview_image::render_image_preview(&ri.response_state, c_resp, fmt, cx),
        );
    }

    if is_large_payload {
        let scroll_handle = ri.response_state.read(cx).view().scroll_handle.clone();
        return div()
            .flex_1()
            .size_full()
            .overflow_hidden()
            .p(px(8.))
            .child(
                uniform_list("response-lines-virtual", lines.len(), {
                    let lines = lines.clone();
                    move |range, _window, _cx| {
                        range
                            .map(|idx| {
                                let line_num = idx + 1;
                                let line = &lines[idx];
                                div()
                                    .h(px(20.))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(
                                        div()
                                            .w(px(40.))
                                            .flex()
                                            .justify_end()
                                            .text_xs()
                                            .font_family(".SystemMonoFont")
                                            .text_color(scheme.line_number)
                                            .child(format!("{}", line_num)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_xs()
                                            .font_family(".SystemMonoFont")
                                            .text_color(scheme.foreground)
                                            .child(if line.is_empty() {
                                                div().child(" ")
                                            } else if is_json {
                                                div().child(
                                                    super::json_highlight::highlight_json_line(
                                                        line, scheme,
                                                    ),
                                                )
                                            } else {
                                                div().child(line.clone())
                                            }),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }
                })
                .track_scroll(&scroll_handle)
                .size_full(),
            );
    }

    div().flex_1().size_full().overflow_hidden().child(
        Editor::new(&ri.response_editor)
            .text_color(scheme.foreground)
            .readonly(true)
            .appearance(false)
            .bordered(false)
            .h(gpui_kit::relative(1.)),
    )
}
