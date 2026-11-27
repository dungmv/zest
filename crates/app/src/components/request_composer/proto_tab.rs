use crate::app::AppView;
use gpui_kit::{div, AnyElement, Context, IntoElement, ParentElement, Styled, Window};

pub(super) fn render_proto_tab(
    av: &AppView,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let req_id_str = av.state.session.read(cx).active_request_id().to_string();
    div()
        .size_full()
        .child({
            let ws_read = av.state.collection.read(cx);
            let payload = ws_read
                .find_request(&req_id_str)
                .and_then(|r| r.grpc.clone())
                .unwrap_or_default();
            let view_fetch = cx.entity().clone();
            super::grpc_proto_tab::render_grpc_proto_tab(
                &payload,
                super::grpc_proto_tab::GrpcProtoTabContext {
                    workspace_state: &av.state.collection,
                    editor_state: &av.state.session,
                    on_fetch_schema: std::rc::Rc::new(move |cx| {
                        view_fetch.update(cx, |view, cx| view.fetch_grpc_schema(cx));
                    }),
                    proto_path_input: &av.composer_inputs.grpc.proto_path_input,
                },
                window,
                cx,
            )
        })
        .into_any_element()
}
