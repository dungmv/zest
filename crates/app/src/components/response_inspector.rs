use crate::components::tabs_with_badge::TabItem;
use crate::state::{
    RequestSessionState, ResponseState, ResponseTabKind, ResponseViewMode, WorkspaceState,
};
use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, uniform_list, AnyElement, App, Entity, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState as CodeEditorState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::assets::IconName;
use zest_core::models::HttpResponseRecord;
use zest_core::storage::models::{RequestExampleFile, RequestFile, RequestKvItem};

#[derive(IntoElement)]
pub struct ResponseInspector {
    workspace_state: Entity<WorkspaceState>,
    editor_state: Entity<RequestSessionState>,
    response_state: Entity<ResponseState>,
    response_editor: Entity<CodeEditorState>,
    request_body_editor: Entity<CodeEditorState>,
}

impl ResponseInspector {
    pub fn new(
        workspace_state: Entity<WorkspaceState>,
        editor_state: Entity<RequestSessionState>,
        response_state: Entity<ResponseState>,
        response_editor: Entity<CodeEditorState>,
        request_body_editor: Entity<CodeEditorState>,
    ) -> Self {
        Self {
            workspace_state,
            editor_state,
            response_state,
            response_editor,
            request_body_editor,
        }
    }
}

impl RenderOnce for ResponseInspector {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let resp_s = self.response_state.read(cx);
        let ed_s = self.editor_state.read(cx);

        let active_id = ed_s.active_request_id.clone();
        let is_viewing_example = resp_s.active_example_name.is_some();
        let is_sending_this_request = ed_s.is_sending
            && (ed_s.running_request_id.is_none()
                || active_id.as_deref() == ed_s.running_request_id.as_deref());

        let current_resp = resp_s.current_response.clone();
        let active_tab = resp_s.active_tab;
        let view_mode = resp_s.view_mode;
        let show_timing = resp_s.show_timing_popover;

        let resp_state_clone = self.response_state.clone();

        div()
            .size_full()
            .bg(t.bg_response)
            .flex()
            .flex_col()
            .relative()
            // Top Status Bar: "200 OK • 18 ms • 2 B" + history icon
            .child(
                div()
                    .h(px(40.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(t.border)
                    .bg(t.bg_editor)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child({
                        if is_sending_this_request {
                            let progress_text = ed_s
                                .send_progress
                                .clone()
                                .unwrap_or_else(|| "Sending request...".to_string());
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .px(px(8.))
                                        .py(px(3.))
                                        .rounded(px(4.))
                                        .bg(gpui_kit::Hsla { a: 0.12, ..t.primary })
                                        .border_1()
                                        .border_color(gpui_kit::Hsla { a: 0.3, ..t.primary })
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.primary)
                                        .child(
                                            Spinner::new()
                                                .icon(IconName::RefreshCw)
                                                .with_size(px(12.))
                                                .color(t.primary),
                                        )
                                        .child(progress_text),
                                )
                        } else if let Some(resp) = &current_resp {
                            let status_color = t.status_color(resp.status_code);
                            let rs_clone = resp_state_clone.clone();

                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(gpui_kit::Hsla { a: 0.12, ..status_color })
                                        .border_1()
                                        .border_color(gpui_kit::Hsla { a: 0.3, ..status_color })
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::NORMAL)
                                        .text_color(status_color)
                                        .child(format!("● {} {}", resp.status_code, resp.status_text)),
                                )
                                .child({
                                    // Duration with timing popover hover/click
                                    div()
                                        .id("btn-timing-breakdown")
                                        .cursor_pointer()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(if show_timing { t.bg_hover } else { t.bg_card })
                                        .border_1()
                                        .border_color(t.border)
                                        .text_xs()
                                        .text_color(t.text_secondary)
                                        .hover(|s| s.text_color(t.text_primary).bg(t.bg_hover))
                                        .on_click(move |_, _, cx| {
                                            rs_clone.update(cx, |state, cx| {
                                                state.toggle_timing_popover(cx);
                                            });
                                        })
                                        .child(format!("⚡ {}", resp.formatted_duration()))
                                })
                                .child(
                                    div()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(t.bg_card)
                                        .border_1()
                                        .border_color(t.border)
                                        .text_xs()
                                        .text_color(t.text_secondary)
                                        .child(format!("📦 {}", resp.formatted_size())),
                                )
                        } else {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .child("No response yet"),
                                )
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .when(is_viewing_example, |this| {
                                let rs = resp_state_clone.clone();
                                let req_id = active_id.clone().unwrap_or_default();
                                this.child(
                                    div()
                                        .id("btn-return-live")
                                        .cursor_pointer()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(t.bg_card)
                                        .border_1()
                                        .border_color(t.border)
                                        .text_xs()
                                        .text_color(t.primary)
                                        .hover(|s| s.bg(t.bg_hover))
                                        .on_click(move |_, _, cx| {
                                            rs.update(cx, |state, cx| {
                                                state.restore_live_response(&req_id, cx);
                                            });
                                        })
                                        .child(div().flex().items_center().gap(px(4.)).child(Icon::new(IconName::ArrowLeft).size(px(14.))).child("Live")),
                                )
                            }),
                    ),
            )
            // High-Precision Timing Popover
            .when(show_timing && !is_sending_this_request, |this| {
                if let Some(resp) = &current_resp {
                    let timing = &resp.timing;
                    let total_ms = timing.total_ms.max(1);

                    // Proportions for visual bar
                    let dns_pct = (timing.dns_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
                    let tcp_pct = (timing.connect_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
                    let tls_pct = (timing.tls_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
                    let ttfb_pct = (timing.ttfb_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
                    let dl_pct = (timing.download_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);

                    this.child(
                        div()
                            .p(px(10.))
                            .mx(px(12.))
                            .mt(px(4.))
                            .rounded(px(6.))
                            .bg(t.bg_card)
                            .border_1()
                            .border_color(t.border)
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .text_xs()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(t.text_primary)
                                            .child("High-Precision Timing Breakdown"),
                                    )
                                    .child(
                                        div()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.primary)
                                            .child(format!("Total: {} ms", timing.total_ms)),
                                    ),
                            )
                            // Visual Waterfall bar
                            .child(
                                div()
                                    .h(px(8.))
                                    .w_full()
                                    .rounded(px(4.))
                                    .bg(t.bg_input)
                                    .overflow_hidden()
                                    .flex()
                                    .child(div().w(px(dns_pct * 2.5)).bg(gpui_kit::rgb(0x06b6d4))) // Cyan for DNS
                                    .child(div().w(px(tcp_pct * 2.5)).bg(gpui_kit::rgb(0x3b82f6))) // Blue for TCP
                                    .child(div().w(px(tls_pct * 2.5)).bg(gpui_kit::rgb(0xa855f7))) // Purple for TLS
                                    .child(div().w(px(ttfb_pct * 2.5)).bg(gpui_kit::rgb(0xeab308))) // Yellow for TTFB
                                    .child(div().w(px(dl_pct * 2.5)).bg(gpui_kit::rgb(0x22c55e))), // Green for Download
                            )
                            // Detail rows
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_color(t.text_secondary)
                                    .child(div().flex().gap(px(4.)).child("● DNS Lookup:").text_color(gpui_kit::rgb(0x06b6d4)))
                                    .child(format!("{} ms", timing.dns_ms.unwrap_or(0))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_color(t.text_secondary)
                                    .child(div().flex().gap(px(4.)).child("● TCP Connect:").text_color(gpui_kit::rgb(0x3b82f6)))
                                    .child(format!("{} ms", timing.connect_ms.unwrap_or(0))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_color(t.text_secondary)
                                    .child(div().flex().gap(px(4.)).child("● TLS Handshake:").text_color(gpui_kit::rgb(0xa855f7)))
                                    .child(format!("{} ms", timing.tls_ms.unwrap_or(0))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_color(t.text_secondary)
                                    .child(div().flex().gap(px(4.)).child("● TTFB (Wait):").text_color(gpui_kit::rgb(0xeab308)))
                                    .child(format!("{} ms", timing.ttfb_ms.unwrap_or(0))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_color(t.text_secondary)
                                    .child(div().flex().gap(px(4.)).child("● Content Download:").text_color(gpui_kit::rgb(0x22c55e)))
                                    .child(format!("{} ms", timing.download_ms.unwrap_or(0))),
                            )
                            .when(timing.connection_reused, |this| {
                                this.child(
                                    div()
                                        .px(px(4.))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .bg(t.primary.opacity(0.15))
                                        .text_color(t.primary)
                                        .child("⚡ Connection Reused (HTTP Keep-Alive)"),
                                )
                            }),
                    )
                } else {
                    this
                }
            })
            // Response Tabs Bar: Response, Request Sent, Headers [N], Cookies [N]
            .child(
                div()
                    .h(px(34.))
                    .px(px(8.))
                    .border_b_1()
                    .border_color(t.border)
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child({
                        let rs_c = resp_state_clone.clone();
                        TabItem::new("tab-resp-response", "Response")
                            .active(active_tab == ResponseTabKind::Response)
                            .on_click(move |_, _, cx| {
                                rs_c.update(cx, |state, cx| {
                                    state.set_active_tab(ResponseTabKind::Response, cx);
                                });
                            })
                    })
                    .child({
                        let rs_c = resp_state_clone.clone();
                        TabItem::new("tab-resp-request", "Request Sent")
                            .active(active_tab == ResponseTabKind::Request)
                            .on_click(move |_, _, cx| {
                                rs_c.update(cx, |state, cx| {
                                    state.set_active_tab(ResponseTabKind::Request, cx);
                                });
                            })
                    })
                    .child({
                        let rs_c = resp_state_clone.clone();
                        let count_str = if is_sending_this_request {
                            "-".to_string()
                        } else {
                            current_resp.as_ref().map(|r| format!("{}", r.headers.len())).unwrap_or_else(|| "0".to_string())
                        };
                        TabItem::new("tab-resp-headers", "Headers")
                            .active(active_tab == ResponseTabKind::Headers)
                            .badge(count_str)
                            .on_click(move |_, _, cx| {
                                rs_c.update(cx, |state, cx| {
                                    state.set_active_tab(ResponseTabKind::Headers, cx);
                                });
                            })
                    })
                    .child({
                        let rs_c = resp_state_clone.clone();
                        let count_str = if is_sending_this_request {
                            "-".to_string()
                        } else {
                            current_resp.as_ref().map(|r| format!("{}", r.cookies.len())).unwrap_or_else(|| "0".to_string())
                        };
                        TabItem::new("tab-resp-cookies", "Cookies")
                            .active(active_tab == ResponseTabKind::Cookies)
                            .badge(count_str)
                            .on_click(move |_, _, cx| {
                                rs_c.update(cx, |state, cx| {
                                    state.set_active_tab(ResponseTabKind::Cookies, cx);
                                });
                            })
                    })
                    .child({
                        let rs_c = resp_state_clone.clone();
                        let badge_str = if is_sending_this_request {
                            "-".to_string()
                        } else {
                            format!("{}", resp_s.console_logs.len())
                        };
                        TabItem::new("tab-resp-console", "Console")
                            .active(active_tab == ResponseTabKind::Console)
                            .badge(badge_str)
                            .on_click(move |_, _, cx| {
                                rs_c.update(cx, |state, cx| {
                                    state.set_active_tab(ResponseTabKind::Console, cx);
                                });
                            })
                    }),
            )
            // Body / Content area
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child({
                        match active_tab {
                            ResponseTabKind::Response => {
                                if is_sending_this_request {
                                    let progress_text = ed_s
                                        .send_progress
                                        .clone()
                                        .unwrap_or_else(|| "Waiting for response from server...".to_string());
                                    let ed_s_cancel = self.editor_state.clone();

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
                                                .items_center()
                                                .justify_center()
                                                .w(px(56.))
                                                .h(px(56.))
                                                .rounded_full()
                                                .bg(gpui_kit::Hsla { a: 0.1, ..t.primary })
                                                .text_color(t.primary)
                                                .child(
                                                    Spinner::new()
                                                        .icon(IconName::RefreshCw)
                                                        .with_size(px(26.))
                                                        .color(t.primary),
                                                ),
                                        )
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
                                            div()
                                                .id("btn-cancel-in-response-view")
                                                .cursor_pointer()
                                                .mt(px(4.))
                                                .px(px(12.))
                                                .py(px(5.))
                                                .rounded(px(4.))
                                                .bg(t.bg_card)
                                                .border_1()
                                                .border_color(t.border)
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                .text_color(t.status_5xx)
                                                .hover(|s| s.bg(t.bg_hover))
                                                .on_click(move |_, _, cx| {
                                                    ed_s_cancel.update(cx, |state, cx| {
                                                        if let Some(tx) = state.cancel_tx.take() {
                                                            let _ = tx.send(());
                                                        }
                                                        state.stop_sending(cx);
                                                    });
                                                })
                                                .child("Cancel Request"),
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
                                        let raw_str = current_resp.as_ref().map(|r| String::from_utf8_lossy(&r.body).to_string()).unwrap_or_default();
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
                                        (std::sync::Arc::new(display_text), std::sync::Arc::new(lines), is_json)
                                    };

                                    let total_lines = lines.len();
                                    let is_large_payload = body_len > 10 * 1024 * 1024 || total_lines > 20000;

                                let rs_toggle = resp_state_clone.clone();
                                let rs_copy = resp_state_clone.clone();
                                let rs_save = resp_state_clone.clone();
                                let rs_wrap = resp_state_clone.clone();
                                let resp_ed = self.response_editor.clone();
                                let ws_s = self.workspace_state.clone();
                                let c_resp = current_resp.clone().unwrap_or_else(|| std::sync::Arc::new(zest_core::models::HttpResponseRecord::default()));
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
                                                            div()
                                                                .id("btn-save-example")
                                                                .tooltip(move |window, cx| {
                                                                    gpui_kit::component::tooltip::Tooltip::new("Save as Example").build(window, cx)
                                                                })
                                                                .cursor_pointer()
                                                                .px(px(6.))
                                                                .py(px(2.))
                                                                .rounded(px(4.))
                                                                .bg(t.bg_card)
                                                                .border_1()
                                                                .border_color(t.border)
                                                                .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                                                                .text_xs()
                                                                .text_color(t.text_secondary)
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .on_click(move |_, _, cx| {
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
                                                                })
                                                                .child(Icon::new(IconName::Save).size(px(14.))),
                                                        )
                                                    })
                                                    // Toggle Wrap Line Button
                                                    .child({
                                                        let rs_w = rs_wrap.clone();
                                                        let r_ed = resp_ed.clone();
                                                        div()
                                                            .id("btn-toggle-wrap")
                                                            .tooltip(move |window, cx| {
                                                                    let tip = if wrap_lines { "Line Wrap: Enabled" } else { "Line Wrap: Disabled" };
                                                                gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx)
                                                            })
                                                            .px(px(6.))
                                                            .py(px(2.))
                                                            .rounded(px(4.))
                                                            .border_1()
                                                            .border_color(t.border)
                                                            .bg(if wrap_lines { t.bg_selected } else { t.bg_card })
                                                            .hover(|s| s.bg(t.bg_hover).text_color(if wrap_lines { t.primary } else { t.text_primary }))
                                                            .text_xs()
                                                            .text_color(if wrap_lines { t.primary } else { t.text_secondary })
                                                            .cursor_pointer()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .on_click(move |_, window, cx| {
                                                                rs_w.update(cx, |state, cx| {
                                                                    state.toggle_wrap_lines(cx);
                                                                });
                                                                let new_wrap = rs_w.read(cx).wrap_lines;
                                                                r_ed.update(cx, |ed, cx| {
                                                                    ed.set_soft_wrap(new_wrap, window, cx);
                                                                });
                                                            })
                                                            .child(Icon::new(IconName::TextWrap).size(px(14.)))
                                                    })
                                                    // Copy Button
                                                    .child({
                                                        let rs_c = rs_copy.clone();
                                                        div()
                                                            .id("btn-copy-response")
                                                            .tooltip(move |window, cx| {
                                                                gpui_kit::component::tooltip::Tooltip::new("Copy Response").build(window, cx)
                                                            })
                                                            .px(px(6.))
                                                            .py(px(2.))
                                                            .rounded(px(4.))
                                                            .bg(t.bg_card)
                                                            .border_1()
                                                            .border_color(t.border)
                                                            .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                                                            .text_xs()
                                                            .text_color(t.text_secondary)
                                                            .cursor_pointer()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .on_click(move |_, _, cx| {
                                                                let copy_text = rs_c.read(cx).formatted_body.as_ref()
                                                                    .map(|f| f.display_text.to_string())
                                                                    .or_else(|| {
                                                                        rs_c.read(cx).current_response.as_ref()
                                                                            .map(|r| String::from_utf8_lossy(&r.body).to_string())
                                                                    })
                                                                    .unwrap_or_default();
                                                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(copy_text));
                                                            })
                                                            .child(Icon::new(IconName::Copy).size(px(14.)))
                                                    })
                                                    // Save to File (Download) Button
                                                    .child({
                                                        let rs_s = rs_save.clone();
                                                        div()
                                                            .id("btn-save-response")
                                                            .tooltip(move |window, cx| {
                                                                gpui_kit::component::tooltip::Tooltip::new("Save to File").build(window, cx)
                                                            })
                                                            .px(px(6.))
                                                            .py(px(2.))
                                                            .rounded(px(4.))
                                                            .bg(t.bg_card)
                                                            .border_1()
                                                            .border_color(t.border)
                                                            .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                                                            .text_xs()
                                                            .text_color(t.text_secondary)
                                                            .cursor_pointer()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .on_click(move |_, _, cx| {
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
                                                            .child(Icon::new(IconName::Download).size(px(14.)))
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
                                                                                div().child(highlight_json_line(line, t))
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
                                                .py(px(4.))
                                                .px(px(4.))
                                                .child(
                                                    Editor::new(&self.response_editor)
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
                            ResponseTabKind::Request => {
                                if is_sending_this_request {
                                    div()
                                        .size_full()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(8.))
                                        .child(
                                            Spinner::new()
                                                .icon(IconName::RefreshCw)
                                                .with_size(px(20.))
                                                .color(t.primary),
                                        )
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
                                                            .child(
                                                                div()
                                                                    .px(px(6.))
                                                                    .py(px(2.))
                                                                    .rounded(px(4.))
                                                                    .bg(gpui_kit::Hsla { a: 0.15, ..t.method_color(sent.method) })
                                                                    .border_1()
                                                                    .border_color(gpui_kit::Hsla { a: 0.3, ..t.method_color(sent.method) })
                                                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                                                    .text_xs()
                                                                    .text_color(t.method_color(sent.method))
                                                                    .child(sent.method.as_str()),
                                                            )
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
                                                                TextView::markdown("sent-request-url", url_md)
                                                                    .selectable(true),
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
                                                                    Editor::new(&self.request_body_editor)
                                                                        .readonly(true)
                                                                        .appearance(false)
                                                                        .bordered(false)
                                                                        .h(gpui_kit::relative(1.))
                                                                ),
                                                        ),
                                                )
                                            })
                                            .into_any_element()
                                    } else {
                                        div().p(px(12.)).text_xs().text_color(t.text_muted).child("No request sent info available.").into_any_element()
                                    }
                                } else {
                                    div().p(px(12.)).text_xs().text_color(t.text_muted).child("No request sent yet.").into_any_element()
                                }
                            }
                            ResponseTabKind::Headers => {
                                if is_sending_this_request {
                                    div()
                                        .size_full()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(8.))
                                        .child(
                                            Spinner::new()
                                                .icon(IconName::RefreshCw)
                                                .with_size(px(20.))
                                                .color(t.primary),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(t.text_secondary)
                                                .child("Sending request... Response headers will appear here."),
                                        )
                                        .into_any_element()
                                } else if let Some(resp) = &current_resp {
                                    div()
                                        .size_full()
                                        .overflow_y_scrollbar()
                                        .p(px(12.))
                                        .flex()
                                        .flex_col()
                                        .gap(px(6.))
                                        .children(resp.headers.iter().enumerate().map(|(idx, (k, v))| {
                                            let val_copy = v.clone();
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(12.))
                                                .py(px(4.))
                                                .border_b_1()
                                                .border_color(t.border_subtle)
                                                .text_xs()
                                                .font_family(".SystemMonoFont")
                                                .child(div().w(px(200.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).text_color(t.primary).child(k.clone()))
                                                .child(div().flex_1().text_color(t.text_primary).child(v.clone()))
                                                .child(
                                                    div()
                                                        .id(format!("copy-hdr-{}", idx))
                                                        .cursor_pointer()
                                                        .text_color(t.text_muted)
                                                        .hover(|s| s.text_color(t.text_primary))
                                                        .on_click(move |_, _, cx| {
                                                            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(val_copy.clone()));
                                                        })
                                                        .child(Icon::new(IconName::Copy).size(px(14.))),
                                                )
                                        }))
                                        .into_any_element()
                                } else {
                                    div().p(px(12.)).text_xs().text_color(t.text_muted).child("No headers received.").into_any_element()
                                }
                            }
                            ResponseTabKind::Cookies => {
                                if is_sending_this_request {
                                    div()
                                        .size_full()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(8.))
                                        .child(
                                            Spinner::new()
                                                .icon(IconName::RefreshCw)
                                                .with_size(px(20.))
                                                .color(t.primary),
                                        )
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
                                        div().p(px(12.)).text_xs().text_color(t.text_muted).child("No cookies received in this response.").into_any_element()
                                    } else {
                                        div()
                                            .size_full()
                                            .overflow_y_scrollbar()
                                            .p(px(12.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .children(parsed_cookies.into_iter().map(|cookie| {
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
                                                                        this.child(div().px(px(4.)).rounded(px(2.)).bg(gpui_kit::rgb(0x22c55e)).text_color(t.primary_foreground).text_xs().child("Secure"))
                                                                    })
                                                                    .when(cookie.http_only, |this| {
                                                                        this.child(div().px(px(4.)).rounded(px(2.)).bg(gpui_kit::rgb(0x3b82f6)).text_color(t.primary_foreground).text_xs().child("HttpOnly"))
                                                                    }),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .font_family(".SystemMonoFont")
                                                            .text_xs()
                                                            .text_color(t.text_primary)
                                                            .child(cookie.value.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .gap(px(12.))
                                                            .text_xs()
                                                            .text_color(t.text_muted)
                                                            .when_some(cookie.domain, |this, d| this.child(format!("Domain: {}", d)))
                                                            .when_some(cookie.path, |this, p| this.child(format!("Path: {}", p)))
                                                            .when_some(cookie.expires, |this, exp| this.child(format!("Expires: {}", exp))),
                                                    )
                                            }))
                                            .into_any_element()
                                    }
                                } else {
                                    div().p(px(12.)).text_xs().text_color(t.text_muted).child("No cookies received.").into_any_element()
                                }
                            }
                            ResponseTabKind::Console => {
                                if is_sending_this_request {
                                    div()
                                        .size_full()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(8.))
                                        .child(
                                            Spinner::new()
                                                .icon(IconName::RefreshCw)
                                                .with_size(px(20.))
                                                .color(t.primary),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(t.text_secondary)
                                                .child("Sending request... Scripts and tests will execute after response is received."),
                                        )
                                        .into_any_element()
                                } else {
                                    crate::components::ConsolePanel::new(
                                        resp_state_clone.clone(),
                                    )
                                    .into_any_element()
                                }
                            }
                        }
                    }),
            )
    }
}

pub fn highlight_json_line(line: &str, t: &crate::theme::AppTheme) -> gpui_kit::StyledText {
    let mut highlights = Vec::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        if b == b'"' {
            let start = i;
            i += 1;
            while i < len {
                let ch = line[i..].chars().next().unwrap();
                let ch_len = ch.len_utf8();
                if ch == '\\' {
                    i += ch_len;
                    if i < len {
                        let next_ch = line[i..].chars().next().unwrap();
                        i += next_ch.len_utf8();
                    }
                } else if ch == '"' {
                    i += ch_len;
                    break;
                } else {
                    i += ch_len;
                }
            }
            let end = i;

            // Check if followed by colon (ignoring spaces) -> JSON object key
            let mut j = end;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let is_key = j < len && bytes[j] == b':';
            let color = if is_key { t.json_key } else { t.json_string };
            highlights.push((
                start..end,
                gpui_kit::HighlightStyle {
                    color: Some(color),
                    ..Default::default()
                },
            ));
        } else if matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',') {
            highlights.push((
                i..i + 1,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_punctuation),
                    ..Default::default()
                },
            ));
            i += 1;
        } else if b == b'-' || b.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < len
                && (bytes[i].is_ascii_digit()
                    || matches!(bytes[i], b'.' | b'e' | b'E' | b'+' | b'-'))
            {
                i += 1;
            }
            highlights.push((
                start..i,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_number),
                    ..Default::default()
                },
            ));
        } else if line[i..].starts_with("true")
            && (i + 4 == len || !bytes[i + 4].is_ascii_alphanumeric())
        {
            highlights.push((
                i..i + 4,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_boolean),
                    font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ));
            i += 4;
        } else if line[i..].starts_with("false")
            && (i + 5 == len || !bytes[i + 5].is_ascii_alphanumeric())
        {
            highlights.push((
                i..i + 5,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_boolean),
                    font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ));
            i += 5;
        } else if line[i..].starts_with("null")
            && (i + 4 == len || !bytes[i + 4].is_ascii_alphanumeric())
        {
            highlights.push((
                i..i + 4,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_null),
                    font_style: Some(gpui_kit::FontStyle::Italic),
                    ..Default::default()
                },
            ));
            i += 4;
        } else {
            let ch = line[i..].chars().next().unwrap();
            i += ch.len_utf8();
        }
    }

    gpui_kit::StyledText::new(line.to_string()).with_highlights(highlights)
}
