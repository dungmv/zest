use super::ResponseInspector;
use crate::state::ResponseViewMode;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Editor;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, uniform_list, AnyElement, App, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};
use std::sync::Arc;
use zest_core::models::HttpResponseRecord;
use zest_core::storage::models::{RequestExampleFile, RequestKvItem};

pub(super) fn render_body_tab(
    ri: &ResponseInspector,
    cx: &App,
    is_sending_this_request: bool,
    current_resp: &Option<Arc<HttpResponseRecord>>,
) -> AnyElement {
    let t = theme();
    let resp_s = ri.response_state.read(cx);
    let ed_s = ri.editor_state.read(cx);
    let active_id = ed_s.active_request_id.clone();
    let is_viewing_example = resp_s.active_example_name.is_some();
    let view_mode = resp_s.view_mode;
    let resp_state_clone = ri.response_state.clone();
    let current_resp = current_resp.clone();
    if is_sending_this_request {
        let progress_text = ed_s
            .send_progress
            .clone()
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
                            if let Some(tx) = state.cancel_tx.take() {
                                let _ = tx.send(());
                            }
                            state.stop_sending(cx);
                        });
                    }),
            )
            .into_any_element()
    } else if current_resp.is_none() {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .child(
                div()
                    .text_3xl()
                    .text_color(t.text_muted)
                    .child(Icon::new(IconName::Globe).size(px(14.))),
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
    } else {
        let body_len = current_resp.as_ref().map(|r| r.body.len()).unwrap_or(0);

        let (_display_text, lines, is_json) = if let Some(fmt) = &resp_s.formatted_body {
            (fmt.display_text.clone(), fmt.lines.clone(), fmt.is_json)
        } else {
            let raw_str = current_resp
                .as_ref()
                .map(|r| String::from_utf8_lossy(&r.body).to_string())
                .unwrap_or_default();
            let parsed_json = serde_json::from_str::<serde_json::Value>(&raw_str).ok();
            let is_json = parsed_json.is_some()
                || current_resp.as_ref().map(|r| r.is_json()).unwrap_or(false)
                || raw_str.trim_start().starts_with('{')
                || raw_str.trim_start().starts_with('[');

            let display_text = if matches!(view_mode, ResponseViewMode::Pretty) {
                if let Some(parsed) = &parsed_json {
                    serde_json::to_string_pretty(parsed).unwrap_or_else(|_| raw_str.clone())
                } else {
                    raw_str.clone()
                }
            } else {
                raw_str.clone()
            };

            let lines: Vec<String> = display_text.lines().map(|s| s.to_string()).collect();
            (
                std::sync::Arc::new(display_text),
                std::sync::Arc::new(lines),
                is_json,
            )
        };

        let total_lines = lines.len();
        let is_large_payload = body_len > 10 * 1024 * 1024 || total_lines > 20000;

        let rs_toggle = resp_state_clone.clone();
        let rs_copy = resp_state_clone.clone();
        let rs_save = resp_state_clone.clone();
        let rs_wrap = resp_state_clone.clone();
        let resp_ed = ri.response_editor.clone();
        let ws_s = ri.workspace_state.clone();
        let c_resp = current_resp.clone().unwrap_or_else(|| {
            std::sync::Arc::new(zest_core::models::HttpResponseRecord::default())
        });
        let req_id_opt = active_id.clone();
        let wrap_lines = resp_s.wrap_lines;

        div()
                                    .size_full()
                                    .flex()
                                    .flex_col()
                                    // Response sub-toolbar
                                    .child(
                                        div()
                                            .h(px(32.))
                                            .px(px(8.))
                                            .border_b_1()
                                            .border_color(t.border)
                                            .bg(t.bg_editor)
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(4.))
                                                    // Pretty Button
                                                    .child({
                                                        let rs_c = rs_toggle.clone();
                                                        let is_pretty = matches!(view_mode, ResponseViewMode::Pretty);
                                                        div()
                                                            .id("btn-view-pretty")
                                                            .px(px(8.))
                                                            .py(px(2.))
                                                            .rounded(px(4.))
                                                            .border_1()
                                                            .border_color(if is_pretty { t.border } else { t.transparent })
                                                            .bg(if is_pretty { t.bg_selected } else { t.transparent })
                                                            .text_color(if is_pretty { t.text_primary } else { t.text_secondary })
                                                            .text_xs()
                                                            .font_weight(if is_pretty { gpui_kit::FontWeight::SEMIBOLD } else { gpui_kit::FontWeight::NORMAL })
                                                            .cursor_pointer()
                                                            .hover(|s| if is_pretty { s } else { s.text_color(t.text_primary).bg(t.bg_hover) })
                                                            .on_click(move |_, _, cx| {
                                                                rs_c.update(cx, |state, cx| {
                                                                    state.set_view_mode(ResponseViewMode::Pretty, cx);
                                                                });
                                                            })
                                                            .child("Pretty")
                                                    })
                                                    // Raw Button
                                                    .child({
                                                        let rs_c = rs_toggle.clone();
                                                        let is_raw = matches!(view_mode, ResponseViewMode::Raw);
                                                        div()
                                                            .id("btn-view-raw")
                                                            .px(px(8.))
                                                            .py(px(2.))
                                                            .rounded(px(4.))
                                                            .border_1()
                                                            .border_color(if is_raw { t.border } else { t.transparent })
                                                            .bg(if is_raw { t.bg_selected } else { t.transparent })
                                                            .text_color(if is_raw { t.text_primary } else { t.text_secondary })
                                                            .text_xs()
                                                            .font_weight(if is_raw { gpui_kit::FontWeight::SEMIBOLD } else { gpui_kit::FontWeight::NORMAL })
                                                            .cursor_pointer()
                                                            .hover(|s| if is_raw { s } else { s.text_color(t.text_primary).bg(t.bg_hover) })
                                                            .on_click(move |_, _, cx| {
                                                                rs_c.update(cx, |state, cx| {
                                                                    state.set_view_mode(ResponseViewMode::Raw, cx);
                                                                });
                                                            })
                                                            .child("Raw")
                                                    })
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.))
                                                    // Save as Example Button
                                                    .when_some(req_id_opt.clone(), |this, req_id| {
                                                        if is_viewing_example {
                                                            return this;
                                                        }
                                                        let ws_s = ws_s.clone();
                                                        let c_resp = c_resp.clone();
                                                        this.child(
                                                            Button::new("btn-save-example")
                                                                .ghost()
                                                                .xsmall()
                                                                .icon(IconName::Save)
                                                                .tooltip("Save as Example")
                                                                .on_click(move |_, _, cx: &mut App| {
                                                                    ws_s.update(cx, |ws, cx| {
                                                                        if let Some(mut req) = ws.get_request_file(&req_id) {
                                                                            let base_name = format!("{} {}", c_resp.status_code, c_resp.status_text);
                                                                            let mut name = base_name.clone();
                                                                            let mut idx = 2;
                                                                            while req.examples.iter().any(|e| e.name == name) {
                                                                                name = format!("{}-{}", base_name, idx);
                                                                                idx += 1;
                                                                            }
                                                                            let ex = RequestExampleFile {
                                                                                name,
                                                                                status: c_resp.status_code,
                                                                                headers: c_resp.headers.iter().map(|(k, v)| RequestKvItem::new(k, v)).collect(),
                                                                                body: Some(String::from_utf8_lossy(&c_resp.body).to_string()),
                                                                            };
                                                                            req.examples.push(ex);
                                                                            ws.save_request_file(&req_id, &req, cx);
                                                                        }
                                                                    });
                                                                }),
                                                        )
                                                    })
                                                    // Toggle Wrap Line Button
                                                    .child({
                                                        let rs_w = rs_wrap.clone();
                                                        let r_ed = resp_ed.clone();
                                                        let wrap_icon = if wrap_lines {
                                                            Icon::new(IconName::TextWrap).text_color(t.primary)
                                                        } else {
                                                            Icon::new(IconName::TextWrap)
                                                        };

                                                        Button::new("btn-toggle-wrap")
                                                            .ghost()
                                                            .xsmall()
                                                            .icon(wrap_icon)
                                                            .tooltip("Toggle line wrap")
                                                            .on_click(move |_, window, cx: &mut App| {
                                                                rs_w.update(cx, |state, cx| {
                                                                    state.toggle_wrap_lines(cx);
                                                                });
                                                                let new_wrap = rs_w.read(cx).wrap_lines;
                                                                r_ed.update(cx, |ed, cx| {
                                                                    ed.set_soft_wrap(new_wrap, window, cx);
                                                                });
                                                            })
                                                    })
                                                    // Copy Button
                                                    .child({
                                                        let rs_c = rs_copy.clone();
                                                        Button::new("btn-copy-response")
                                                            .ghost()
                                                            .xsmall()
                                                            .icon(IconName::Copy)
                                                            .tooltip("Copy Response")
                                                            .on_click(move |_, _, cx: &mut App| {
                                                                let copy_text = rs_c.read(cx).formatted_body.as_ref()
                                                                    .map(|f| f.display_text.to_string())
                                                                    .or_else(|| {
                                                                        rs_c.read(cx).current_response.as_ref()
                                                                            .map(|r| String::from_utf8_lossy(&r.body).to_string())
                                                                    })
                                                                    .unwrap_or_default();
                                                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(copy_text));
                                                            })
                                                    })
                                                    // Save to File (Download) Button
                                                    .child({
                                                        let rs_s = rs_save.clone();
                                                        Button::new("btn-save-response")
                                                            .ghost()
                                                            .xsmall()
                                                            .icon(IconName::Download)
                                                            .tooltip("Save to File")
                                                            .on_click(move |_, _, cx: &mut App| {
                                                                let save_bytes = rs_s.read(cx).current_response.as_ref().map(|r| r.body.clone());
                                                                if let Some(bytes) = save_bytes {
                                                                    cx.spawn(async move |cx| {
                                                                        if let Some(file) = rfd::AsyncFileDialog::new()
                                                                            .set_file_name("response.json")
                                                                            .save_file()
                                                                            .await
                                                                        {
                                                                            let path = file.path().to_path_buf();
                                                                            cx.background_executor()
                                                                                .spawn(async move {
                                                                                    let _ = std::fs::write(path, &bytes);
                                                                                })
                                                                                .await;
                                                                        }
                                                                    })
                                                                    .detach();
                                                                }
                                                            })
                                                    }),
                                            ),
                                    )
                                    // Large payload notice banner
                                    .when(is_large_payload, |this| {
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
                                                        .child(format!("⚠ Large payload ({} lines). Virtualized list active at 120 FPS.", total_lines)),
                                                )
                                        )
                                    })
                                    // Response lines area: Editor with folding/selection for normal payloads, uniform_list for large payloads
                                    .child({
                                        if is_large_payload {
                                            div()
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
                                                                            .text_color(t.text_muted)
                                                                            .child(format!("{}", line_num)),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .flex_1()
                                                                            .text_xs()
                                                                            .font_family(".SystemMonoFont")
                                                                            .text_color(t.text_primary)
                                                                            .child(if line.is_empty() {
                                                                                div().child(" ")
                                                                            } else if is_json {
                                                                                div().child(super::json_highlight::highlight_json_line(line, t))
                                                                            } else {
                                                                                div().child(line.clone())
                                                                            }),
                                                                    )
                                                            })
                                                            .collect::<Vec<_>>()
                                                    }
                                                })
                                                .track_scroll(&resp_s.scroll_handle)
                                                .size_full(),
                                            )
                                        } else {
                                            div()
                                                .flex_1()
                                                .size_full()
                                                .overflow_hidden()
                                                .child(
                                                    Editor::new(&ri.response_editor)
                                                        .readonly(true)
                                                        .appearance(false)
                                                        .bordered(false)
                                                        .h(gpui_kit::relative(1.))
                                                )
                                        }
                                    })
                                    .into_any_element()
    }
}
