use crate::app::AppView;
use crate::components::*;
use crate::state::*;
use gpui_kit::{div, AnyElement, Context, IntoElement, ParentElement, Styled};

pub(super) fn render_headers_tab(av: &AppView, cx: &mut Context<AppView>) -> AnyElement {
    let req_id_str = av.state.session.read(cx).active_request_id().to_string();
    let ws_read = av.state.workspace.read(cx);
    let active_r = ws_read.find_request(&req_id_str);
    let headers = active_r.map(|r| r.headers.clone()).unwrap_or_default();
    let readonly_rows = active_r
        .map(|r| inherited_readonly_rows(ws_read, r))
        .unwrap_or_default();
    let ed_up = av.state.session.clone();
    let ws_up = av.state.workspace.clone();
    let ed_add = av.state.session.clone();
    let ws_add = av.state.workspace.clone();
    let ed_del = av.state.session.clone();
    let ws_del = av.state.workspace.clone();
    let req_id_k = req_id_str.clone();
    let req_id_a = req_id_str.clone();
    let req_id_d = req_id_str.clone();
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(
            div().flex_1().child(
                KvTable::new(
                    format!("headers-kv-{}", req_id_str),
                    headers,
                    move |idx, k, v, enabled, _, cx| {
                        edit_active_request(&ws_up, &ed_up, &req_id_k, cx, |r| {
                            r.update_header(idx, k, v, enabled);
                        });
                    },
                    move |_, cx| {
                        edit_active_request(&ws_add, &ed_add, &req_id_a, cx, |r| {
                            r.add_header();
                        });
                    },
                    move |idx, _, cx| {
                        edit_active_request(&ws_del, &ed_del, &req_id_d, cx, |r| {
                            r.remove_header(idx);
                        });
                    },
                )
                .readonly_rows(readonly_rows)
                .env_state(av.state.env.clone())
                .modal_state(av.state.modal.clone()),
            ),
        )
        .into_any_element()
}
