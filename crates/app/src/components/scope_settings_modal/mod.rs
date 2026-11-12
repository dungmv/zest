pub mod auth_inputs;
pub mod auth_tab;
pub mod general_tab;
pub mod headers_tab;

pub use auth_inputs::*;
pub use auth_tab::*;
pub use general_tab::*;
pub use headers_tab::*;

use crate::components::ModalDialog;
use crate::state::{EnvironmentState, ModalState, ScopeSettingsTarget, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use zest_core::models::Auth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScopeSettingsTab {
    #[default]
    General,
    Headers,
    Auth,
}

#[derive(IntoElement)]
pub struct ScopeSettingsModal {
    workspace_state: Entity<WorkspaceDataState>,
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
}

impl ScopeSettingsModal {
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

impl RenderOnce for ScopeSettingsModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let ws_clone = self.workspace_state.clone();
        let modal_clone = self.modal_state.clone();
        let env_clone = self.env_state.clone();
        let target = match &modal_clone.read(cx).scope_settings_target {
            Some(target) => target.clone(),
            None => return div().into_any_element(),
        };

        // Scope details (name, headers, auth)
        let (title, folder_id_opt, current_headers, current_auth) = {
            let ws = ws_clone.read(cx);
            match &target {
                ScopeSettingsTarget::Workspace => {
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
                ScopeSettingsTarget::Folder(f_id) => {
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
        let tab_state = window.use_state(cx, |_, _| ScopeSettingsTab::General);
        let raw_tab = *tab_state.read(cx);
        let active_tab = if is_folder && raw_tab == ScopeSettingsTab::General {
            ScopeSettingsTab::Headers
        } else {
            raw_tab
        };
        let headers_count = current_headers.len();

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
            gpui_kit::SharedString::from(format!("scope-ws-name-{}", workspace_path_str)),
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

        ModalDialog::new("scope-settings-modal")
            .title(title)
            .w(px(580.))
            .on_close({
                let modal_c = modal_clone.clone();
                move |_, cx| {
                    modal_c.update(cx, |m, cx| m.close_scope_settings(cx));
                }
            })
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    // Tab Bar
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .p(px(3.))
                            .rounded(px(6.))
                            .bg(t.bg_input)
                            .border_1()
                            .border_color(t.border)
                            .when(!is_folder, |this| {
                                let is_active = active_tab == ScopeSettingsTab::General;
                                let tab = tab_state.clone();
                                this.child(
                                    div()
                                        .id("tab-scope-general")
                                        .flex_1()
                                        .h(px(26.))
                                        .rounded(px(4.))
                                        .bg(if is_active { t.bg_card } else { t.transparent })
                                        .text_color(if is_active {
                                            t.text_primary
                                        } else {
                                            t.text_muted
                                        })
                                        .font_weight(if is_active {
                                            gpui_kit::FontWeight::SEMIBOLD
                                        } else {
                                            gpui_kit::FontWeight::NORMAL
                                        })
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .on_click(move |_, _, cx| {
                                            tab.update(cx, |state, _| {
                                                *state = ScopeSettingsTab::General
                                            });
                                        })
                                        .child("General"),
                                )
                            })
                            .child({
                                let is_active = active_tab == ScopeSettingsTab::Headers;
                                let tab = tab_state.clone();
                                div()
                                    .id("tab-scope-headers")
                                    .flex_1()
                                    .h(px(26.))
                                    .rounded(px(4.))
                                    .bg(if is_active { t.bg_card } else { t.transparent })
                                    .text_color(if is_active {
                                        t.text_primary
                                    } else {
                                        t.text_muted
                                    })
                                    .font_weight(if is_active {
                                        gpui_kit::FontWeight::SEMIBOLD
                                    } else {
                                        gpui_kit::FontWeight::NORMAL
                                    })
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(4.))
                                    .text_xs()
                                    .on_click(move |_, _, cx| {
                                        tab.update(cx, |state, _| {
                                            *state = ScopeSettingsTab::Headers
                                        });
                                    })
                                    .child(format!("Headers ({})", headers_count))
                            })
                            .child({
                                let is_active = active_tab == ScopeSettingsTab::Auth;
                                let tab = tab_state.clone();
                                div()
                                    .id("tab-scope-auth")
                                    .flex_1()
                                    .h(px(26.))
                                    .rounded(px(4.))
                                    .bg(if is_active { t.bg_card } else { t.transparent })
                                    .text_color(if is_active {
                                        t.text_primary
                                    } else {
                                        t.text_muted
                                    })
                                    .font_weight(if is_active {
                                        gpui_kit::FontWeight::SEMIBOLD
                                    } else {
                                        gpui_kit::FontWeight::NORMAL
                                    })
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .on_click(move |_, _, cx| {
                                        tab.update(cx, |state, _| *state = ScopeSettingsTab::Auth);
                                    })
                                    .child("Authentication")
                            }),
                    )
                    // Tab Content
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .h(px(320.))
                            .overflow_y_scrollbar()
                            .child(match active_tab {
                                ScopeSettingsTab::General => render_general_tab(
                                    &ws_clone,
                                    &ws_name_input,
                                    &workspace_name,
                                    &workspace_path_str,
                                    cx,
                                )
                                .into_any_element(),
                                ScopeSettingsTab::Headers => render_headers_tab(
                                    &ws_clone,
                                    &self.env_state,
                                    &modal_clone,
                                    &target,
                                    &current_headers,
                                    is_folder,
                                )
                                .into_any_element(),
                                ScopeSettingsTab::Auth => render_auth_tab(
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
                    ),
            )
            .into_any_element()
    }
}
