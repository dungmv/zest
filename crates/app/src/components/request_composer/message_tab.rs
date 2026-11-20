use crate::app::AppView;
use gpui_kit::{div, AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use zest_core::models::Protocol;

pub(super) fn render_message_tab(
    av: &AppView,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let req_id_str = av.state.session.read(cx).active_request_id().to_string();
    let ws_read = av.state.workspace.read(cx);
    let active_req = ws_read.find_request(&req_id_str);
    let protocol = active_req.map(|r| r.protocol).unwrap_or(Protocol::Http);
    match protocol {
        Protocol::WebSocket => {
            let view_send = cx.entity().clone();
            let view_ping = cx.entity().clone();
            let msg_format = active_req.map(|r| r.ws_message_format).unwrap_or_default();
            div()
                .size_full()
                .child(super::ws_message_tab::render_ws_message_tab(
                    msg_format,
                    super::ws_message_tab::WsMessageTabContext {
                        workspace_state: &av.state.workspace,
                        editor_state: &av.state.session,
                        message_textarea: &av.composer_inputs.ws_message_textarea,
                        on_send_message: std::rc::Rc::new(move |window, cx| {
                            view_send.update(cx, |view, cx| {
                                view.send_ws_message(window, cx);
                            });
                        }),
                        on_ping: std::rc::Rc::new(move |cx| {
                            view_ping.update(cx, |view, cx| {
                                view.send_ws_ping(cx);
                            });
                        }),
                    },
                    cx,
                ))
                .into_any_element()
        }
        Protocol::Grpc => div()
            .size_full()
            .child({
                let payload = active_req.and_then(|r| r.grpc.clone()).unwrap_or_default();
                super::grpc_message_tab::render_grpc_message_tab(
                    &payload,
                    super::grpc_message_tab::GrpcMessageTabContext {
                        workspace_state: &av.state.workspace,
                        editor_state: &av.state.session,
                        message_editor: &av.composer_inputs.grpc.message_editor,
                        error_marks: &av.composer_inputs.grpc.error_marks,
                        diagnostics_hover: &av.composer_inputs.grpc.diagnostics_hover,
                    },
                    window,
                    cx,
                )
            })
            .into_any_element(),
        Protocol::Http | Protocol::GraphQL | Protocol::Sse => div().into_any_element(),
    }
}
