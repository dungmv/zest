use super::ResponseInspector;
use crate::state::ResponseViewMode;
use crate::theme::{theme, AppTheme};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputState;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Div, Entity, InteractiveElement as _, ParentElement,
    StatefulInteractiveElement as _, Styled,
};
use std::sync::Arc;
use zest_core::models::{HttpResponseRecord, ResponsePreviewKind};
use zest_core::storage::models::{RequestExampleFile, RequestKvItem};

pub(super) struct BodyToolbarProps {
    pub active_id: Option<String>,
    pub current_resp: Arc<HttpResponseRecord>,
    pub view_mode: ResponseViewMode,
    pub preview_kind: Option<ResponsePreviewKind>,
    pub is_preview_mode: bool,
    pub is_viewing_example: bool,
    pub is_raster_image: bool,
    pub is_binary: bool,
    pub is_large_payload: bool,
    pub wrap_lines: bool,
    pub jsonpath_active: bool,
    pub show_jsonpath_bar: bool,
    pub jsonpath_input: Entity<InputState>,
}

fn view_mode_pill(
    id: &'static str,
    label: &'static str,
    active: bool,
    mode: ResponseViewMode,
    rs: Entity<crate::state::ResponseState>,
    t: &AppTheme,
) -> gpui_kit::Stateful<Div> {
    div()
        .id(id)
        .px(px(8.))
        .py(px(2.))
        .rounded(px(4.))
        .border_1()
        .border_color(if active { t.border } else { t.transparent })
        .bg(if active { t.bg_selected } else { t.transparent })
        .text_color(if active {
            t.text_primary
        } else {
            t.text_secondary
        })
        .text_xs()
        .font_weight(if active {
            gpui_kit::FontWeight::SEMIBOLD
        } else {
            gpui_kit::FontWeight::NORMAL
        })
        .cursor_pointer()
        .hover(move |s| {
            if active {
                s
            } else {
                s.text_color(t.text_primary).bg(t.bg_hover)
            }
        })
        .on_click(move |_, _, cx| {
            rs.update(cx, |state, cx| {
                state.set_view_mode(mode, cx);
            });
        })
        .child(label)
}

pub(super) fn render_body_toolbar(ri: &ResponseInspector, props: BodyToolbarProps) -> Div {
    let t = theme();
    let rs = ri.response_state.clone();
    let resp_ed = ri.response_editor.clone();
    let ws = ri.workspace_state.clone();

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
                .child(view_mode_pill(
                    "btn-view-pretty",
                    "Pretty",
                    matches!(props.view_mode, ResponseViewMode::Pretty),
                    ResponseViewMode::Pretty,
                    rs.clone(),
                    t,
                ))
                .child(view_mode_pill(
                    "btn-view-raw",
                    "Raw",
                    matches!(props.view_mode, ResponseViewMode::Raw),
                    ResponseViewMode::Raw,
                    rs.clone(),
                    t,
                ))
                .when(props.preview_kind.is_some(), |this| {
                    this.child(view_mode_pill(
                        "btn-view-preview",
                        "Preview",
                        props.is_preview_mode,
                        ResponseViewMode::Preview,
                        rs.clone(),
                        t,
                    ))
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                // Toggle JSONPath Filter Button (JSON bodies only)
                .when(props.jsonpath_active, |this| {
                    let rs_f = rs.clone();
                    let jp_in = props.jsonpath_input.clone();
                    let filter_icon = if props.show_jsonpath_bar {
                        Icon::new(IconName::Funnel).text_color(t.primary)
                    } else {
                        Icon::new(IconName::Funnel)
                    };
                    this.child(
                        Button::new("btn-toggle-jsonpath")
                            .ghost()
                            .xsmall()
                            .icon(filter_icon)
                            .tooltip("Filter with JSONPath")
                            .on_click(move |_, window, cx: &mut App| {
                                let now_open =
                                    rs_f.update(cx, |state, cx| state.toggle_jsonpath_bar(cx));
                                if now_open {
                                    jp_in.update(cx, |inp, cx| {
                                        inp.focus(window, cx);
                                    });
                                }
                            }),
                    )
                })
                // Save as Example Button (text responses within size limit only)
                .when_some(props.active_id, |this, req_id| {
                    if props.is_viewing_example
                        || props.is_raster_image
                        || props.is_binary
                        || props.is_large_payload
                    {
                        return this;
                    }
                    let ws_s = ws.clone();
                    let c_resp = props.current_resp.clone();
                    this.child(
                        Button::new("btn-save-example")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Save)
                            .tooltip("Save as Example")
                            .on_click(move |_, _, cx: &mut App| {
                                ws_s.update(cx, |ws, cx| {
                                    if let Some(mut req) = ws.get_request_file(&req_id) {
                                        let base_name = format!(
                                            "{} {}",
                                            c_resp.status_code, c_resp.status_text
                                        );
                                        let mut name = base_name.clone();
                                        let mut idx = 2;
                                        while req.examples.iter().any(|e| e.name == name) {
                                            name = format!("{}-{}", base_name, idx);
                                            idx += 1;
                                        }
                                        let ex = RequestExampleFile {
                                            name,
                                            status: c_resp.status_code,
                                            headers: c_resp
                                                .headers
                                                .iter()
                                                .map(|(k, v)| RequestKvItem::new(k, v))
                                                .collect(),
                                            body: Some(
                                                String::from_utf8_lossy(&c_resp.body).to_string(),
                                            ),
                                        };
                                        req.examples.push(ex);
                                        ws.save_request_file(&req_id, &req, cx);
                                    }
                                });
                            }),
                    )
                })
                // Toggle Wrap Line Button (text view only)
                .when(!props.is_preview_mode, |this| {
                    let rs_w = rs.clone();
                    let r_ed = resp_ed.clone();
                    let wrap_icon = if props.wrap_lines {
                        Icon::new(IconName::TextWrap).text_color(t.primary)
                    } else {
                        Icon::new(IconName::TextWrap)
                    };

                    this.child(
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
                            }),
                    )
                })
                // Copy Button (text responses only)
                .when(!props.is_raster_image && !props.is_binary, |this| {
                    let rs_c = rs.clone();
                    this.child(
                        Button::new("btn-copy-response")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Copy)
                            .tooltip("Copy Response")
                            .on_click(move |_, _, cx: &mut App| {
                                let copy_text = rs_c
                                    .read(cx)
                                    .formatted_body
                                    .as_ref()
                                    .map(|f| f.display_text.to_string())
                                    .unwrap_or_default();
                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                    copy_text,
                                ));
                            }),
                    )
                })
                // Save to File (Download) Button
                .child({
                    let rs_s = rs;
                    Button::new("btn-save-response")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Download)
                        .tooltip("Save to File")
                        .on_click(move |_, _, cx: &mut App| {
                            // Zero-copy Arc clone: never duplicate a 100MB Vec<u8> on the UI thread.
                            let save_data = rs_s
                                .read(cx)
                                .current_response
                                .as_ref()
                                .map(|r| (r.clone(), r.suggested_filename()));
                            if let Some((record, file_name)) = save_data {
                                cx.spawn(async move |cx| {
                                    if let Some(file) = rfd::AsyncFileDialog::new()
                                        .set_file_name(&file_name)
                                        .save_file()
                                        .await
                                    {
                                        let path = file.path().to_path_buf();
                                        let write_res = cx
                                            .background_executor()
                                            .spawn(async move {
                                                std::fs::write(&path, &record.body)
                                                    .map_err(|err| (path, err))
                                            })
                                            .await;
                                        if let Err((path, err)) = write_res {
                                            cx.update(|cx| {
                                                crate::notify::error(
                                                    "Failed to save response",
                                                    format!("{}: {}", path.display(), err),
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                })
                                .detach();
                            }
                        })
                }),
        )
}
