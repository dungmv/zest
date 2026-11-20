pub mod auth_inputs;
pub mod auth_tab;
pub mod general_tab;
pub mod headers_tab;

pub use auth_inputs::*;
pub use auth_tab::*;
pub use general_tab::*;
pub use headers_tab::*;

use crate::components::ModalDialog;
use crate::state::{
    EnvironmentState, ModalKind, ModalState, WorkspaceDataState, WorkspaceSettingTarget,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::{
    div, px, App, AppContext as _, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window,
};
use zest_core::models::Auth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspaceSettingTab {
    #[default]
    General,
    Headers,
    Auth,
}

#[derive(IntoElement)]
pub struct WorkspaceSettingModal {
    workspace_state: Entity<WorkspaceDataState>,
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
}

impl WorkspaceSettingModal {
    pub fn new(
        workspace_state: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        modal_state: Entity<ModalState>,
    ) -> Self {
        Self {
            workspace_state,
            env_state,
            modal_state,
        }
    }
}

impl RenderOnce for WorkspaceSettingModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ws_clone = self.workspace_state.clone();
        let modal_clone = self.modal_state.clone();
        let env_clone = self.env_state.clone();
        let target = match &modal_clone.read(cx).workspace_setting_target {
            Some(target) => target.clone(),
            None => return div().into_any_element(),
        };

        // Scope details (name, headers, auth)
        let (title, folder_id_opt, current_headers, current_auth) = {
            let ws = ws_clone.read(cx);
            match &target {
                WorkspaceSettingTarget::Workspace => {
                    let name = ws
                        .active_workspace
                        .as_ref()
                        .map(|w| w.name.clone())
                        .unwrap_or_else(|| "Workspace".to_string());
                    let headers = ws
                        .active_workspace
                        .as_ref()
                        .map(|w| w.headers.clone())
                        .unwrap_or_default();
                    let auth = ws
                        .active_workspace
                        .as_ref()
                        .map(|w| w.auth.clone())
                        .unwrap_or(Auth::None);
                    (format!("Workspace Settings: {}", name), None, headers, auth)
                }
                WorkspaceSettingTarget::Folder(f_id) => {
                    let folder = ws.folders.iter().find(|f| &f.id == f_id).cloned();
                    let name = folder
                        .as_ref()
                        .map(|f| f.name.clone())
                        .unwrap_or_else(|| f_id.clone());
                    let headers = folder
                        .as_ref()
                        .map(|f| f.headers.clone())
                        .unwrap_or_default();
                    let auth = folder
                        .as_ref()
                        .map(|f| f.auth.clone())
                        .unwrap_or(Auth::Inherit);
                    (
                        format!("Folder Settings: {}", name),
                        Some(f_id.clone()),
                        headers,
                        auth,
                    )
                }
            }
        };

        // Tab state: General (for Workspace) vs Headers vs Auth
        let is_folder = folder_id_opt.is_some();
        let tab_state = window.use_state(cx, |_, _| WorkspaceSettingTab::General);
        let raw_tab = *tab_state.read(cx);
        let active_tab = if is_folder && raw_tab == WorkspaceSettingTab::General {
            WorkspaceSettingTab::Headers
        } else {
            raw_tab
        };
        // Visible tabs in display order; General is hidden for folders (they inherit
        // workspace-level general settings, so only Headers/Auth apply).
        let visible_tabs: Vec<(WorkspaceSettingTab, String)> = [
            (WorkspaceSettingTab::General, "General".to_string()),
            (
                WorkspaceSettingTab::Headers,
                format!("Headers ({})", current_headers.len()),
            ),
            (
                WorkspaceSettingTab::Auth,
                format!("Auth ({})", auth_type_label(&current_auth)),
            ),
        ]
        .into_iter()
        .filter(|(tab, _)| !is_folder || *tab != WorkspaceSettingTab::General)
        .collect();
        let selected_index = visible_tabs
            .iter()
            .position(|(tab, _)| *tab == active_tab)
            .unwrap_or(0);

        let (workspace_name, workspace_path_str) = {
            let ws_r = ws_clone.read(cx);
            let w_name = ws_r
                .active_workspace
                .as_ref()
                .map(|w| w.name.clone())
                .unwrap_or_else(|| "default".to_string());
            let w_path = ws_r.workspace_path.to_string_lossy().to_string();
            (w_name, w_path)
        };

        let ws_name_holder = window.use_keyed_state(
            gpui_kit::SharedString::from(format!("ws-setting-name-{}", workspace_path_str)),
            cx,
            |window, cx| {
                let input =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Enter workspace name..."));
                input.update(cx, |s, cx| s.set_value(workspace_name.clone(), window, cx));
                let ws_s = ws_clone.clone();
                let sub = cx.subscribe_in(&input, window, move |_, inp, ev, _, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        let val = inp.read(cx).value().to_string();
                        let trimmed = val.trim();
                        if !trimmed.is_empty() {
                            ws_s.update(cx, |ws, cx| {
                                ws.rename_workspace(trimmed.to_string(), cx);
                            });
                        }
                    }
                });
                (input, sub)
            },
        );
        let ws_name_input = ws_name_holder.read(cx).0.clone();

        let auth_inputs =
            use_scope_auth_inputs(window, cx, &target, &current_auth, &ws_clone, &env_clone);

        ModalDialog::new("workspace-setting-modal")
            .w(px(580.))
            .h(px(320.))
            .title(title)
            .icon(IconName::Settings)
            .on_close({
                let modal_c = modal_clone.clone();
                move |_, cx| {
                    modal_c.update(cx, |m, cx| m.close(ModalKind::WorkspaceSetting, cx));
                }
            })
            // Tab Bar
            .child({
                let kinds: Vec<WorkspaceSettingTab> =
                    visible_tabs.iter().map(|(tab, _)| *tab).collect();
                TabBar::new("workspace-setting-tabs")
                    .underline()
                    .px(px(16.))
                    .selected_index(selected_index)
                    .on_click(move |&idx, window, cx| {
                        if let Some(tab) = kinds.get(idx) {
                            let tab = *tab;
                            tab_state.update(cx, |state, _| *state = tab);
                            window.refresh();
                        }
                    })
                    .children(
                        visible_tabs
                            .iter()
                            .map(|(_, label)| Tab::new().label(label.clone())),
                    )
            })
            // Tab Body
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(16.))
                    .overflow_y_scrollbar()
                    .child(match active_tab {
                        WorkspaceSettingTab::General => render_general_tab(
                            &ws_clone,
                            &ws_name_input,
                            &workspace_name,
                            &workspace_path_str,
                            cx,
                        )
                        .into_any_element(),
                        WorkspaceSettingTab::Headers => render_headers_tab(
                            &ws_clone,
                            &self.env_state,
                            &modal_clone,
                            &target,
                            &current_headers,
                            is_folder,
                        )
                        .into_any_element(),
                        WorkspaceSettingTab::Auth => render_auth_tab(
                            AuthTabContext {
                                ws_state: &ws_clone,
                                env_state: &self.env_state,
                                modal_state: &modal_clone,
                                target: &target,
                                current_auth: &current_auth,
                                is_folder,
                                folder_id_opt: folder_id_opt.as_deref(),
                                bearer_input: &auth_inputs.bearer_input,
                                basic_user_input: &auth_inputs.basic_user_input,
                                basic_pass_input: &auth_inputs.basic_pass_input,
                                api_key_input: &auth_inputs.api_key_input,
                                api_val_input: &auth_inputs.api_val_input,
                            },
                            window,
                            cx,
                        )
                        .into_any_element(),
                    }),
            )
            .into_any_element()
    }
}
