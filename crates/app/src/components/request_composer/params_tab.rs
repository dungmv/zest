use crate::app::AppView;
use crate::components::*;
use crate::state::*;
use gpui_kit::{div, AnyElement, Context, IntoElement, ParentElement, Styled};

pub(super) fn render_params_tab(av: &AppView, cx: &mut Context<AppView>) -> AnyElement {
    let req_id_str = av.state.session.read(cx).active_request_id().to_string();
    let ws_read = av.state.collection.read(cx);
    let params = ws_read
        .find_request(&req_id_str)
        .map(|r| r.params.clone())
        .unwrap_or_default();
    let ed_up = av.state.session.clone();
    let ws_up = av.state.collection.clone();
    let ed_add = av.state.session.clone();
    let ws_add = av.state.collection.clone();
    let ed_del = av.state.session.clone();
    let ws_del = av.state.collection.clone();
    let req_id_k = req_id_str.clone();
    let req_id_a = req_id_str.clone();
    let req_id_d = req_id_str.clone();
    div()
        .size_full()
        .child(
            KvTable::new(
                format!("params-kv-{}", req_id_str),
                params,
                move |idx, k, v, enabled, _, cx| {
                    edit_active_request(&ws_up, &ed_up, &req_id_k, cx, |r| {
                        r.update_param(idx, k, v, enabled);
                    });
                },
                move |_, cx| {
                    edit_active_request(&ws_add, &ed_add, &req_id_a, cx, |r| {
                        r.add_param();
                    });
                },
                move |idx, _, cx| {
                    edit_active_request(&ws_del, &ed_del, &req_id_d, cx, |r| {
                        r.remove_param(idx);
                    });
                },
            )
            .env_state(av.state.env.clone())
            .modal_state(av.state.modal.clone()),
        )
        .into_any_element()
}
