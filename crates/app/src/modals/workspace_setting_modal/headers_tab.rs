use crate::components::kv_table::KvTable;
use crate::state::{
    CollectionState, EnvironmentState, ModalState, WorkspaceSettingTarget, WorkspaceState,
};
use crate::theme::theme;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, Styled, Window};
use zest_core::models::{Auth, KeyValuePair};

fn read_target_headers(
    ws: &Entity<WorkspaceState>,
    col: &Entity<CollectionState>,
    target: &WorkspaceSettingTarget,
    cx: &App,
) -> Vec<KeyValuePair> {
    match target {
        WorkspaceSettingTarget::Workspace => ws.read(cx).workspace_meta().headers.clone(),
        WorkspaceSettingTarget::Folder(fid) => col
            .read(cx)
            .folders()
            .iter()
            .find(|f| &f.id == fid)
            .map(|f| f.headers.clone())
            .unwrap_or_default(),
    }
}

fn save_target_headers(
    ws: &Entity<WorkspaceState>,
    col: &Entity<CollectionState>,
    target: &WorkspaceSettingTarget,
    headers: Vec<KeyValuePair>,
    cx: &mut App,
) {
    match target {
        WorkspaceSettingTarget::Workspace => {
            let a = ws.read(cx).workspace_meta().auth.clone();
            ws.update(cx, |state, cx| {
                state.save_workspace_settings(headers, a, cx)
            });
        }
        WorkspaceSettingTarget::Folder(fid) => {
            let a = col
                .read(cx)
                .folders()
                .iter()
                .find(|f| &f.id == fid)
                .map(|f| f.auth.clone())
                .unwrap_or(Auth::Inherit);
            col.update(cx, |state, cx| {
                state.save_folder_settings(fid, headers, a, cx)
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_headers_tab(
    ws_state: &Entity<WorkspaceState>,
    col_state: &Entity<CollectionState>,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    target: &WorkspaceSettingTarget,
    current_headers: &[KeyValuePair],
    is_folder: bool,
) -> impl IntoElement {
    let t = theme();
    let ws_up = ws_state.clone();
    let col_up = col_state.clone();
    let tgt = target.clone();
    let pairs = current_headers.to_vec();

    let on_update = {
        let ws_s = ws_up.clone();
        let col_s = col_up.clone();
        let tgt = tgt.clone();
        move |idx: usize,
              key: String,
              val: String,
              enabled: bool,
              _window: &mut Window,
              cx: &mut App| {
            let mut cur_headers = read_target_headers(&ws_s, &col_s, &tgt, cx);
            if idx < cur_headers.len() {
                cur_headers[idx].key = key;
                cur_headers[idx].value = val;
                cur_headers[idx].enabled = enabled;
                save_target_headers(&ws_s, &col_s, &tgt, cur_headers, cx);
            }
        }
    };

    let on_add = {
        let ws_s = ws_up.clone();
        let col_s = col_up.clone();
        let tgt = tgt.clone();
        move |_window: &mut Window, cx: &mut App| {
            let mut cur_headers = read_target_headers(&ws_s, &col_s, &tgt, cx);
            cur_headers.push(KeyValuePair::new("", ""));
            save_target_headers(&ws_s, &col_s, &tgt, cur_headers, cx);
        }
    };

    let on_remove = {
        let ws_s = ws_up;
        let col_s = col_up;
        move |idx: usize, _window: &mut Window, cx: &mut App| {
            let mut cur_headers = read_target_headers(&ws_s, &col_s, &tgt, cx);
            if idx < cur_headers.len() {
                cur_headers.remove(idx);
                save_target_headers(&ws_s, &col_s, &tgt, cur_headers, cx);
            }
        }
    };

    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().text_xs().text_color(t.text_secondary).child(
            if is_folder {
                "Headers configured here will be inherited by all requests inside this folder (and its subfolders)."
            } else {
                "Headers configured here will be inherited by all requests in this workspace."
            },
        ))
        .child(
            div()
                .w_full()
                .rounded(px(6.))
                .border_1()
                .border_color(t.border)
                .overflow_hidden()
                .child(
                    KvTable::new(
                        "ws-setting-headers-table",
                        pairs,
                        on_update,
                        on_add,
                        on_remove,
                    )
                    .env_state(env_state.clone())
                    .modal_state(modal_state.clone()),
                ),
        )
}
