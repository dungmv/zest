mod body_tab;
mod body_tab_toolbar;
mod console_tab;
mod cookies_tab;
pub(crate) mod format;
mod headers_tab;
pub mod json_highlight;
mod messages_tab;
mod preview_image;
mod request_sent_tab;
mod status_pill;
mod timing_popover;
pub(crate) mod view;

use crate::components::tabs_with_badge::render_tab_badge;
use crate::state::{CollectionState, RequestSessionState, ResponseState, ResponseTabKind};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::EditorState as CodeEditorState;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window,
};
use status_pill::status_pill;
use zest_core::models::Protocol;
use zest_core::ws::WebSocketStatus;

#[derive(IntoElement)]
pub struct ResponseInspector {
    workspace_state: Entity<CollectionState>,
    editor_state: Entity<RequestSessionState>,
    response_state: Entity<ResponseState>,
    response_editor: Entity<CodeEditorState>,
    request_body_editor: Entity<CodeEditorState>,
}

impl ResponseInspector {
    pub fn new(
        workspace_state: Entity<CollectionState>,
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let resp_s = self.response_state.read(cx);
        let ed_s = self.editor_state.read(cx);

        let active_id = ed_s.active_request_id.clone();
        let is_viewing_example = resp_s.active_example_name.is_some();
        let is_sending_this_request = ed_s.is_sending_for(active_id.as_deref());

        let protocol = active_id
            .as_deref()
            .and_then(|id| self.workspace_state.read(cx).find_request(id))
            .map(|r| r.protocol)
            .unwrap_or(Protocol::Http);
        // Only WS/gRPC render realtime pills and the handshake headers tab;
        // GraphQL is a plain exchange and SSE keeps normal HTTP headers.
        let is_realtime = protocol.is_realtime();

        let current_resp = resp_s.current_response.clone();
        let active_tab = resp_s.view().active_tab;
        let _view_mode = resp_s.view().mode;
        let show_timing = resp_s.view().show_timing_popover;

        let resp_state_clone = self.response_state.clone();

        div()
            .size_full()
            .bg(t.bg_response)
            .flex()
            .flex_col()
            .relative()
            // Top Status Bar: "200 OK • 18 ms • 2 B" + history icon, or the
            // realtime connection status pill for WS/gRPC requests
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
                                .send_progress()
                                .map(str::to_owned)
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
                                        .bg(gpui_kit::Hsla {
                                            a: 0.12,
                                            ..t.primary
                                        })
                                        .border_1()
                                        .border_color(gpui_kit::Hsla {
                                            a: 0.3,
                                            ..t.primary
                                        })
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.accent_text)
                                        .child(Spinner::new().with_size(px(12.)).color(t.primary))
                                        .child(progress_text),
                                )
                                .into_any_element()
                        } else if protocol == Protocol::Grpc {
                            Self::render_grpc_status_pill(
                                resp_s.realtime().grpc_last_call.as_ref(),
                                t,
                            )
                        } else if protocol == Protocol::WebSocket {
                            Self::render_realtime_status_pill(&resp_s.realtime().ws_status, t)
                        } else if let Some(resp) = &current_resp {
                            let status_color = t.status_color(resp.status_code);
                            let rs_clone = resp_state_clone.clone();

                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(status_pill(
                                    format!("● {} {}", resp.status_code, resp.status_text),
                                    status_color,
                                ))
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
                                .into_any_element()
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
                                .into_any_element()
                        }
                    })
                    .child(div().flex().items_center().gap(px(6.)).when(
                        is_viewing_example,
                        |this| {
                            let rs = resp_state_clone.clone();
                            let req_id = active_id.clone().unwrap_or_default();
                            this.child(
                                Button::new("btn-return-live")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowLeft)
                                    .label("Live")
                                    .on_click(move |_, _, cx: &mut App| {
                                        rs.update(cx, |state, cx| {
                                            state.restore_live_response(&req_id, cx);
                                        });
                                    }),
                            )
                        },
                    )),
            )
            .when_some(
                current_resp.as_ref().and_then(|r| {
                    timing_popover::render_timing_popover(r, show_timing, is_sending_this_request)
                }),
                |this, el| this.child(el),
            )
            // Response Tabs Bar — strip depends on the request protocol
            .child({
                let tabs = ResponseTabKind::tabs_for(protocol);
                let active_idx = tabs.iter().position(|k| *k == active_tab).unwrap_or(0);
                let header_count = if is_sending_this_request {
                    "-".to_string()
                } else if is_realtime {
                    format!("{}", resp_s.realtime().ws_handshake_headers.len())
                } else {
                    current_resp
                        .as_ref()
                        .map(|r| format!("{}", r.headers.len()))
                        .unwrap_or_else(|| "0".to_string())
                };
                let cookie_count = if is_sending_this_request {
                    "-".to_string()
                } else {
                    current_resp
                        .as_ref()
                        .map(|r| format!("{}", r.cookies.len()))
                        .unwrap_or_else(|| "0".to_string())
                };
                let console_count = if is_sending_this_request {
                    "-".to_string()
                } else {
                    format!("{}", resp_s.console().logs.len())
                };
                let message_count = format!("{}", resp_s.realtime().frames.len());
                let rs_c = resp_state_clone.clone();

                let mut bar = TabBar::new("response-inspector-tabs")
                    .underline()
                    .px(px(8.))
                    .selected_index(active_idx)
                    .on_click(move |&idx, _, cx| {
                        let tabs = ResponseTabKind::tabs_for(protocol);
                        let kind = tabs.get(idx).copied().unwrap_or(tabs[0]);
                        rs_c.update(cx, |state, cx| {
                            state.set_active_tab(kind, cx);
                        });
                    });
                for kind in tabs {
                    let mut tab = Tab::new().label(kind.label());
                    tab = match kind {
                        ResponseTabKind::Messages => tab.suffix(render_tab_badge(
                            message_count.clone(),
                            active_tab == ResponseTabKind::Messages,
                            t,
                        )),
                        ResponseTabKind::Headers => tab.suffix(render_tab_badge(
                            header_count.clone(),
                            active_tab == ResponseTabKind::Headers,
                            t,
                        )),
                        ResponseTabKind::Cookies => tab.suffix(render_tab_badge(
                            cookie_count.clone(),
                            active_tab == ResponseTabKind::Cookies,
                            t,
                        )),
                        ResponseTabKind::Console => tab.suffix(render_tab_badge(
                            console_count.clone(),
                            active_tab == ResponseTabKind::Console,
                            t,
                        )),
                        _ => tab,
                    };
                    bar = bar.child(tab);
                }
                bar
            })
            .child(div().flex_1().overflow_hidden().child(match active_tab {
                ResponseTabKind::Messages => {
                    messages_tab::render_messages_tab(&self, cx, is_sending_this_request)
                }
                ResponseTabKind::Response => body_tab::render_body_tab(
                    &self,
                    window,
                    cx,
                    is_sending_this_request,
                    &current_resp,
                ),
                ResponseTabKind::Request => request_sent_tab::render_request_sent_tab(
                    &self,
                    cx,
                    is_sending_this_request,
                    &current_resp,
                ),
                ResponseTabKind::Headers if is_realtime => {
                    messages_tab::render_realtime_headers_tab(&self, cx)
                }
                ResponseTabKind::Headers => headers_tab::render_headers_tab(
                    &self,
                    cx,
                    is_sending_this_request,
                    &current_resp,
                ),
                ResponseTabKind::Cookies => cookies_tab::render_cookies_tab(
                    &self,
                    cx,
                    is_sending_this_request,
                    &current_resp,
                ),
                ResponseTabKind::Console => console_tab::render_console_tab(
                    &self,
                    cx,
                    is_sending_this_request,
                    &current_resp,
                ),
            }))
    }
}

impl ResponseInspector {
    /// grpc-status pill for the most recent gRPC call.
    fn render_grpc_status_pill(
        last_call: Option<&zest_core::grpc::GrpcCallSummary>,
        t: &crate::theme::AppTheme,
    ) -> gpui_kit::AnyElement {
        let (label, color) = match last_call {
            Some(summary) if summary.is_ok() => (
                format!("\u{25cf} OK \u{00b7} {} ms", summary.duration_ms),
                t.status_2xx,
            ),
            Some(summary) => (
                format!(
                    "\u{25cf} ERROR {} {}",
                    summary.status_code,
                    summary.status_message.clone().unwrap_or_default()
                ),
                t.status_5xx,
            ),
            None => ("\u{25cf} Not called".to_string(), t.text_muted),
        };
        status_pill(label, color)
    }

    /// Connection status pill for WebSocket requests (non-sending states).
    fn render_realtime_status_pill(
        status: &WebSocketStatus,
        t: &crate::theme::AppTheme,
    ) -> gpui_kit::AnyElement {
        let (label, color) = match status {
            WebSocketStatus::Connected => ("● Connected", t.status_2xx),
            WebSocketStatus::Disconnected => ("● Disconnected", t.text_muted),
            WebSocketStatus::Connecting => ("● Connecting…", t.text_muted),
            WebSocketStatus::Failed(_) => ("● Failed", t.status_5xx),
        };
        let title = match status {
            WebSocketStatus::Failed(err) => format!("{label}: {err}"),
            _ => label.to_string(),
        };
        status_pill(title, color)
    }
}
