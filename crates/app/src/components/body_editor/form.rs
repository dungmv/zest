use crate::components::kv_table::KvTable;
use crate::state::{EnvironmentState, ModalState, RequestSessionState, WorkspaceDataState};
use gpui::{div, AnyElement, Entity, IntoElement, ParentElement, Styled};
use gpui_component::scroll::ScrollableElement as _;
use zest_core::models::KeyValuePair;

pub fn render_form_editor(
    req_id_str: &str,
    pairs: &[KeyValuePair],
    editor_state: &Entity<RequestSessionState>,
    workspace_state: Option<&Entity<WorkspaceDataState>>,
    env_state: Option<&Entity<EnvironmentState>>,
    modal_state: Option<&Entity<ModalState>>,
) -> AnyElement {
    let pairs = pairs.to_vec();
    let ed_up = editor_state.clone();
    let ws_up = workspace_state.cloned();
    let ws_add = ws_up.clone();
    let ws_del = ws_up.clone();
    let ed_add = editor_state.clone();
    let ed_del = editor_state.clone();
    let req_id_change = req_id_str.to_string();
    let req_id_add = req_id_str.to_string();
    let req_id_del = req_id_str.to_string();

    div()
        .size_full()
        .overflow_y_scrollbar()
        .child(
            KvTable::new(
                format!("form-kv-{}", req_id_str),
                pairs,
                move |idx, k, v, enabled, _, cx| {
                    if let Some(ws) = &ws_up {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&req_id_change, |r| {
                                r.update_form_param(idx, k, v, enabled);
                            });
                        });
                    }
                    ed_up.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                },
                move |_, cx| {
                    if let Some(ws) = &ws_add {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&req_id_add, |r| {
                                r.add_form_param();
                            });
                        });
                    }
                    ed_add.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                },
                move |idx, _, cx| {
                    if let Some(ws) = &ws_del {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&req_id_del, |r| {
                                r.remove_form_param(idx);
                            });
                        });
                    }
                    ed_del.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                },
            )
            .show_add_button(false)
            .optional_env_state(env_state.cloned())
            .optional_modal_state(modal_state.cloned()),
        )
        .into_any_element()
}
