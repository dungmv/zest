use crate::components::kv_table::KvTable;
use crate::state::{EnvironmentState, ModalState, ScopeSettingsTarget, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, Styled, Window};
use zest_core::models::{Auth, KeyValuePair};

pub fn render_headers_tab(
    ws_state: &Entity<WorkspaceDataState>,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    target: &ScopeSettingsTarget,
    current_headers: &[KeyValuePair],
    is_folder: bool,
) -> impl IntoElement {
    let t = theme();
    let ws_up = ws_state.clone();
    let tgt = target.clone();
    let pairs = current_headers.to_vec();

    let on_update = {
        let ws_s = ws_up.clone();
        let tgt = tgt.clone();
        move |idx: usize,
              key: String,
              val: String,
              enabled: bool,
              _window: &mut Window,
              cx: &mut App| {
            let mut cur_headers = match &tgt {
                ScopeSettingsTarget::Workspace => ws_s
                    .read(cx)
                    .active_workspace
                    .as_ref()
                    .map(|w| w.headers.clone())
                    .unwrap_or_default(),
                ScopeSettingsTarget::Folder(fid) => ws_s
                    .read(cx)
                    .folders
                    .iter()
                    .find(|f| &f.id == fid)
                    .map(|f| f.headers.clone())
                    .unwrap_or_default(),
            };
            if idx < cur_headers.len() {
                cur_headers[idx].key = key;
                cur_headers[idx].value = val;
                cur_headers[idx].enabled = enabled;
                match &tgt {
                    ScopeSettingsTarget::Workspace => {
                        let a = ws_s
                            .read(cx)
                            .active_workspace
                            .as_ref()
                            .map(|w| w.auth.clone())
                            .unwrap_or(Auth::None);
                        ws_s.update(cx, |state, cx| {
                            state.save_workspace_scope_settings(cur_headers, a, cx)
                        });
                    }
                    ScopeSettingsTarget::Folder(fid) => {
                        let a = ws_s
                            .read(cx)
                            .folders
                            .iter()
                            .find(|f| &f.id == fid)
                            .map(|f| f.auth.clone())
                            .unwrap_or(Auth::Inherit);
                        ws_s.update(cx, |state, cx| {
                            state.save_folder_scope_settings(fid, cur_headers, a, cx)
                        });
                    }
                }
            }
        }
    };

    let on_add = {
        let ws_s = ws_up.clone();
        let tgt = tgt.clone();
        move |_window: &mut Window, cx: &mut App| {
            let mut cur_headers = match &tgt {
                ScopeSettingsTarget::Workspace => ws_s
                    .read(cx)
                    .active_workspace
                    .as_ref()
                    .map(|w| w.headers.clone())
                    .unwrap_or_default(),
                ScopeSettingsTarget::Folder(fid) => ws_s
                    .read(cx)
                    .folders
                    .iter()
                    .find(|f| &f.id == fid)
                    .map(|f| f.headers.clone())
                    .unwrap_or_default(),
            };
            cur_headers.push(KeyValuePair::new("", ""));
            match &tgt {
                ScopeSettingsTarget::Workspace => {
                    let a = ws_s
                        .read(cx)
                        .active_workspace
                        .as_ref()
                        .map(|w| w.auth.clone())
                        .unwrap_or(Auth::None);
                    ws_s.update(cx, |state, cx| {
                        state.save_workspace_scope_settings(cur_headers, a, cx)
                    });
                }
                ScopeSettingsTarget::Folder(fid) => {
                    let a = ws_s
                        .read(cx)
                        .folders
                        .iter()
                        .find(|f| &f.id == fid)
                        .map(|f| f.auth.clone())
                        .unwrap_or(Auth::Inherit);
                    ws_s.update(cx, |state, cx| {
                        state.save_folder_scope_settings(fid, cur_headers, a, cx)
                    });
                }
            }
        }
    };

    let on_remove = {
        let ws_s = ws_up.clone();
        let tgt = tgt.clone();
        move |idx: usize, _window: &mut Window, cx: &mut App| {
            let mut cur_headers = match &tgt {
                ScopeSettingsTarget::Workspace => ws_s
                    .read(cx)
                    .active_workspace
                    .as_ref()
                    .map(|w| w.headers.clone())
                    .unwrap_or_default(),
                ScopeSettingsTarget::Folder(fid) => ws_s
                    .read(cx)
                    .folders
                    .iter()
                    .find(|f| &f.id == fid)
                    .map(|f| f.headers.clone())
                    .unwrap_or_default(),
            };
            if idx < cur_headers.len() {
                cur_headers.remove(idx);
                match &tgt {
                    ScopeSettingsTarget::Workspace => {
                        let a = ws_s
                            .read(cx)
                            .active_workspace
                            .as_ref()
                            .map(|w| w.auth.clone())
                            .unwrap_or(Auth::None);
                        ws_s.update(cx, |state, cx| {
                            state.save_workspace_scope_settings(cur_headers, a, cx)
                        });
                    }
                    ScopeSettingsTarget::Folder(fid) => {
                        let a = ws_s
                            .read(cx)
                            .folders
                            .iter()
                            .find(|f| &f.id == fid)
                            .map(|f| f.auth.clone())
                            .unwrap_or(Auth::Inherit);
                        ws_s.update(cx, |state, cx| {
                            state.save_folder_scope_settings(fid, cur_headers, a, cx)
                        });
                    }
                }
            }
        }
    };

    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .text_xs()
                .text_color(t.text_muted)
                .child(if is_folder {
                    "Headers configured here apply to all requests in this folder (and subfolders)."
                } else {
                    "Headers configured here apply to all requests in this workspace."
                }),
        )
        .child(
            KvTable::new("scope-headers-table", pairs, on_update, on_add, on_remove)
                .env_state(env_state.clone())
                .modal_state(modal_state.clone()),
        )
}
