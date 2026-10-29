use crate::components::env_autocomplete::{build_tokenized_content, detect_unclosed_env_var};
use crate::components::env_input::render_env_input;
use crate::components::kv_table::KvTable;
use crate::components::ModalDialog;
use crate::state::{EnvironmentState, ModalState, ScopeSettingsTarget, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use zest_core::models::{ApiKeyLocation, Auth, KeyValuePair};

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

        let auth_label = match &current_auth {
            Auth::Inherit => "Inherit",
            Auth::None => "None",
            Auth::Bearer { .. } => "Bearer Token",
            Auth::Basic { .. } => "Basic Auth",
            Auth::ApiKey { .. } => "API Key",
        };

        let is_inherit = matches!(current_auth, Auth::Inherit);
        let is_none = matches!(current_auth, Auth::None);
        let is_bearer = matches!(current_auth, Auth::Bearer { .. });
        let is_basic = matches!(current_auth, Auth::Basic { .. });
        let is_apikey = matches!(current_auth, Auth::ApiKey { .. });

        // Input state holders for Auth
        let bearer_token_holder = window.use_keyed_state(
            gpui_kit::SharedString::from(format!("scope-auth-bearer-{:?}", target)),
            cx,
            |window, cx| {
                let init_val = match &current_auth {
                    Auth::Bearer { token } => token.clone(),
                    _ => String::new(),
                };
                let input =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Token or {{token}}"));
                let content = build_tokenized_content(&init_val, env_clone.read(cx));
                input.update(cx, |s, cx| s.set_value(content, window, cx));

                let ws_s = ws_clone.clone();
                let tgt = target.clone();
                let env_sub = env_clone.clone();
                let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                    if let InputEvent::Change = ev {
                        let token = inp.read(cx).value().to_string();
                        let new_auth = Auth::Bearer {
                            token: token.clone(),
                        };
                        match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .active_workspace
                                    .as_ref()
                                    .map(|w| w.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_workspace_scope_settings(cur_h, new_auth, cx)
                                });
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .folders
                                    .iter()
                                    .find(|f| &f.id == fid)
                                    .map(|f| f.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_folder_scope_settings(fid, cur_h, new_auth, cx)
                                });
                            }
                        }
                        if token.contains("}}") && detect_unclosed_env_var(&token).is_none() {
                            let tokenized = build_tokenized_content(&token, env_sub.read(cx));
                            inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                        }
                    }
                });
                (input, sub)
            },
        );
        let bearer_input = bearer_token_holder.read(cx).0.clone();

        let basic_user_holder = window.use_keyed_state(
            gpui_kit::SharedString::from(format!("scope-auth-basic-u-{:?}", target)),
            cx,
            |window, cx| {
                let init_val = match &current_auth {
                    Auth::Basic { username, .. } => username.clone(),
                    _ => String::new(),
                };
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Username"));
                let content = build_tokenized_content(&init_val, env_clone.read(cx));
                input.update(cx, |s, cx| s.set_value(content, window, cx));

                let ws_s = ws_clone.clone();
                let tgt = target.clone();
                let env_sub = env_clone.clone();
                let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                    if let InputEvent::Change = ev {
                        let u = inp.read(cx).value().to_string();
                        let p = match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                match &ws_s.read(cx).workspace_meta.auth {
                                    Auth::Basic { password, .. } => password.clone(),
                                    _ => String::new(),
                                }
                            }
                            ScopeSettingsTarget::Folder(fid) => match ws_s
                                .read(cx)
                                .folders
                                .iter()
                                .find(|f| &f.id == fid)
                                .map(|f| &f.auth)
                            {
                                Some(Auth::Basic { password, .. }) => password.clone(),
                                _ => String::new(),
                            },
                        };
                        let new_auth = Auth::Basic {
                            username: u.clone(),
                            password: p,
                        };
                        match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .active_workspace
                                    .as_ref()
                                    .map(|w| w.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_workspace_scope_settings(cur_h, new_auth, cx)
                                });
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .folders
                                    .iter()
                                    .find(|f| &f.id == fid)
                                    .map(|f| f.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_folder_scope_settings(fid, cur_h, new_auth, cx)
                                });
                            }
                        }
                        if u.contains("}}") && detect_unclosed_env_var(&u).is_none() {
                            let tokenized = build_tokenized_content(&u, env_sub.read(cx));
                            inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                        }
                    }
                });
                (input, sub)
            },
        );
        let basic_user_input = basic_user_holder.read(cx).0.clone();

        let basic_pass_holder = window.use_keyed_state(
            gpui_kit::SharedString::from(format!("scope-auth-basic-p-{:?}", target)),
            cx,
            |window, cx| {
                let init_val = match &current_auth {
                    Auth::Basic { password, .. } => password.clone(),
                    _ => String::new(),
                };
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Password"));
                let content = build_tokenized_content(&init_val, env_clone.read(cx));
                input.update(cx, |s, cx| s.set_value(content, window, cx));

                let ws_s = ws_clone.clone();
                let tgt = target.clone();
                let env_sub = env_clone.clone();
                let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                    if let InputEvent::Change = ev {
                        let p = inp.read(cx).value().to_string();
                        let u = match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                match &ws_s.read(cx).workspace_meta.auth {
                                    Auth::Basic { username, .. } => username.clone(),
                                    _ => String::new(),
                                }
                            }
                            ScopeSettingsTarget::Folder(fid) => match ws_s
                                .read(cx)
                                .folders
                                .iter()
                                .find(|f| &f.id == fid)
                                .map(|f| &f.auth)
                            {
                                Some(Auth::Basic { username, .. }) => username.clone(),
                                _ => String::new(),
                            },
                        };
                        let new_auth = Auth::Basic {
                            username: u,
                            password: p.clone(),
                        };
                        match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .active_workspace
                                    .as_ref()
                                    .map(|w| w.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_workspace_scope_settings(cur_h, new_auth, cx)
                                });
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .folders
                                    .iter()
                                    .find(|f| &f.id == fid)
                                    .map(|f| f.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_folder_scope_settings(fid, cur_h, new_auth, cx)
                                });
                            }
                        }
                        if p.contains("}}") && detect_unclosed_env_var(&p).is_none() {
                            let tokenized = build_tokenized_content(&p, env_sub.read(cx));
                            inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                        }
                    }
                });
                (input, sub)
            },
        );
        let basic_pass_input = basic_pass_holder.read(cx).0.clone();

        let api_key_holder = window.use_keyed_state(
            gpui_kit::SharedString::from(format!("scope-auth-api-key-{:?}", target)),
            cx,
            |window, cx| {
                let (init_k, _) = match &current_auth {
                    Auth::ApiKey { key, value, .. } => (key.clone(), value.clone()),
                    _ => ("X-API-Key".to_string(), String::new()),
                };
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Key"));
                let content = build_tokenized_content(&init_k, env_clone.read(cx));
                input.update(cx, |s, cx| s.set_value(content, window, cx));

                let ws_s = ws_clone.clone();
                let tgt = target.clone();
                let env_sub = env_clone.clone();
                let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                    if let InputEvent::Change = ev {
                        let k = inp.read(cx).value().to_string();
                        let (v, add_to) = match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                match &ws_s.read(cx).workspace_meta.auth {
                                    Auth::ApiKey { value, add_to, .. } => (value.clone(), *add_to),
                                    _ => (String::new(), ApiKeyLocation::Header),
                                }
                            }
                            ScopeSettingsTarget::Folder(fid) => match ws_s
                                .read(cx)
                                .folders
                                .iter()
                                .find(|f| &f.id == fid)
                                .map(|f| &f.auth)
                            {
                                Some(Auth::ApiKey { value, add_to, .. }) => {
                                    (value.clone(), *add_to)
                                }
                                _ => (String::new(), ApiKeyLocation::Header),
                            },
                        };
                        let new_auth = Auth::ApiKey {
                            key: k.clone(),
                            value: v,
                            add_to,
                        };
                        match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .active_workspace
                                    .as_ref()
                                    .map(|w| w.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_workspace_scope_settings(cur_h, new_auth, cx)
                                });
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .folders
                                    .iter()
                                    .find(|f| &f.id == fid)
                                    .map(|f| f.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_folder_scope_settings(fid, cur_h, new_auth, cx)
                                });
                            }
                        }
                        if k.contains("}}") && detect_unclosed_env_var(&k).is_none() {
                            let tokenized = build_tokenized_content(&k, env_sub.read(cx));
                            inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                        }
                    }
                });
                (input, sub)
            },
        );
        let api_key_input = api_key_holder.read(cx).0.clone();

        let api_val_holder = window.use_keyed_state(
            gpui_kit::SharedString::from(format!("scope-auth-api-val-{:?}", target)),
            cx,
            |window, cx| {
                let (_, init_v) = match &current_auth {
                    Auth::ApiKey { key, value, .. } => (key.clone(), value.clone()),
                    _ => ("X-API-Key".to_string(), String::new()),
                };
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));
                let content = build_tokenized_content(&init_v, env_clone.read(cx));
                input.update(cx, |s, cx| s.set_value(content, window, cx));

                let ws_s = ws_clone.clone();
                let tgt = target.clone();
                let env_sub = env_clone.clone();
                let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                    if let InputEvent::Change = ev {
                        let v = inp.read(cx).value().to_string();
                        let (k, add_to) = match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                match &ws_s.read(cx).workspace_meta.auth {
                                    Auth::ApiKey { key, add_to, .. } => (key.clone(), *add_to),
                                    _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                                }
                            }
                            ScopeSettingsTarget::Folder(fid) => match ws_s
                                .read(cx)
                                .folders
                                .iter()
                                .find(|f| &f.id == fid)
                                .map(|f| &f.auth)
                            {
                                Some(Auth::ApiKey { key, add_to, .. }) => (key.clone(), *add_to),
                                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                            },
                        };
                        let new_auth = Auth::ApiKey {
                            key: k,
                            value: v.clone(),
                            add_to,
                        };
                        match &tgt {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .active_workspace
                                    .as_ref()
                                    .map(|w| w.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_workspace_scope_settings(cur_h, new_auth, cx)
                                });
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s
                                    .read(cx)
                                    .folders
                                    .iter()
                                    .find(|f| &f.id == fid)
                                    .map(|f| f.headers.clone())
                                    .unwrap_or_default();
                                ws_s.update(cx, |state, cx| {
                                    state.save_folder_scope_settings(fid, cur_h, new_auth, cx)
                                });
                            }
                        }
                        if v.contains("}}") && detect_unclosed_env_var(&v).is_none() {
                            let tokenized = build_tokenized_content(&v, env_sub.read(cx));
                            inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                        }
                    }
                });
                (input, sub)
            },
        );
        let api_val_input = api_val_holder.read(cx).0.clone();

        // Render modal
        ModalDialog::new("scope-settings-modal")
            .w(px(640.))
            .max_h(px(580.))
            .title(title)
            .icon(IconName::Settings)
            .on_close({
                let modal_s = modal_clone.clone();
                move |_, cx| {
                    modal_s.update(cx, |state, cx| state.close_scope_settings(cx));
                }
            })
            // Tab Bar
            .child(
                div()
                    .px(px(16.))
                    .pt(px(8.))
                            .border_b_1()
                            .border_color(t.border)
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .when(!is_folder, |this| {
                                let tab = tab_state.clone();
                                let is_active = active_tab == ScopeSettingsTab::General;
                                this.child(
                                    div()
                                        .id("tab-scope-general")
                                        .cursor_pointer()
                                        .pb(px(8.))
                                        .border_b_2()
                                        .border_color(if is_active { t.border_focus } else { gpui_kit::transparent_black() })
                                        .text_xs()
                                        .font_weight(if is_active { gpui_kit::FontWeight::BOLD } else { gpui_kit::FontWeight::NORMAL })
                                        .text_color(if is_active { t.text_primary } else { t.text_muted })
                                        .on_click(move |_, window, cx| {
                                            tab.update(cx, |state, _| *state = ScopeSettingsTab::General);
                                            window.refresh();
                                        })
                                        .child("General"),
                                )
                            })
                            .child({
                                let tab = tab_state.clone();
                                let is_active = active_tab == ScopeSettingsTab::Headers;
                                div()
                                    .id("tab-scope-headers")
                                    .cursor_pointer()
                                    .pb(px(8.))
                                    .border_b_2()
                                    .border_color(if is_active { t.border_focus } else { gpui_kit::transparent_black() })
                                    .text_xs()
                                    .font_weight(if is_active { gpui_kit::FontWeight::BOLD } else { gpui_kit::FontWeight::NORMAL })
                                    .text_color(if is_active { t.text_primary } else { t.text_muted })
                                    .on_click(move |_, window, cx| {
                                        tab.update(cx, |state, _| *state = ScopeSettingsTab::Headers);
                                        window.refresh();
                                    })
                                    .child(format!("Headers ({})", headers_count))
                            })
                            .child({
                                let tab = tab_state.clone();
                                let is_active = active_tab == ScopeSettingsTab::Auth;
                                div()
                                    .id("tab-scope-auth")
                                    .cursor_pointer()
                                    .pb(px(8.))
                                    .border_b_2()
                                    .border_color(if is_active { t.border_focus } else { gpui_kit::transparent_black() })
                                    .text_xs()
                                    .font_weight(if is_active { gpui_kit::FontWeight::BOLD } else { gpui_kit::FontWeight::NORMAL })
                                    .text_color(if is_active { t.text_primary } else { t.text_muted })
                                    .on_click(move |_, window, cx| {
                                        tab.update(cx, |state, _| *state = ScopeSettingsTab::Auth);
                                        window.refresh();
                                    })
                                    .child(format!("Auth ({})", auth_label))
                            }),
                    )
                    // Tab Body
                    .child(
                        div()
                            .flex_1()
                            .p(px(16.))
                            .overflow_y_scrollbar()
                            .child(match active_tab {
                                ScopeSettingsTab::General => {
                                    let ws_open = ws_clone.clone();
                                    let ws_save = ws_clone.clone();
                                    let name_inp = ws_name_input.clone();
                                    let name_inp_save = ws_name_input.clone();
                                    let cur_inp_val = ws_name_input.read(cx).value().to_string();
                                    let is_name_changed =
                                        !cur_inp_val.trim().is_empty() && cur_inp_val.trim() != workspace_name.trim();

                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(14.))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(t.text_secondary)
                                                .child("WORKSPACE & STORAGE"),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .min_w_0()
                                                .p(px(14.))
                                                .rounded(px(6.))
                                                .bg(t.bg_app)
                                                .border_1()
                                                .border_color(t.border)
                                                .flex()
                                                .flex_col()
                                                .gap(px(14.))
                                                // Workspace Name (Editable + Save)
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .min_w_0()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(6.))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                .text_color(t.text_primary)
                                                                .child("Workspace Name"),
                                                        )
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .min_w_0()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(8.))
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .min_w_0()
                                                                        .h(px(30.))
                                                                        .px(px(8.))
                                                                        .rounded(px(5.))
                                                                        .bg(t.bg_input)
                                                                        .border_1()
                                                                        .border_color(t.border)
                                                                        .flex()
                                                                        .items_center()
                                                                        .child(
                                                                            Input::new(&name_inp)
                                                                                .id("ws-settings-name-input")
                                                                                .appearance(false)
                                                                                .bordered(false)
                                                                                .focus_bordered(false),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .id("btn-ws-settings-save-name")
                                                                        .flex_shrink_0()
                                                                        .h(px(30.))
                                                                        .px(px(12.))
                                                                        .rounded(px(5.))
                                                                        .bg(if is_name_changed {
                                                                            t.primary
                                                                        } else {
                                                                            t.bg_input
                                                                        })
                                                                        .border_1()
                                                                        .border_color(if is_name_changed {
                                                                            t.primary
                                                                        } else {
                                                                            t.border
                                                                        })
                                                                        .text_color(if is_name_changed {
                                                                            t.primary_foreground
                                                                        } else {
                                                                            t.text_muted
                                                                        })
                                                                        .cursor_pointer()
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                        .hover(|s| s.opacity(0.9))
                                                                        .on_click(move |_, _, cx| {
                                                                            let val = name_inp_save.read(cx).value().to_string();
                                                                            let trimmed = val.trim();
                                                                            if !trimmed.is_empty() {
                                                                                ws_save.update(cx, |ws, cx| {
                                                                                    ws.rename_workspace(trimmed.to_string(), cx);
                                                                                });
                                                                            }
                                                                        })
                                                                        .child("Save"),
                                                                ),
                                                        ),
                                                )
                                                // Separator
                                                .child(div().w_full().h(px(1.)).bg(t.border_subtle))
                                                // Storage Directory
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .min_w_0()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap(px(12.))
                                                        .child(
                                                            div()
                                                                .flex_shrink_0()
                                                                .text_xs()
                                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                .text_color(t.text_primary)
                                                                .child("Storage Directory"),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(8.))
                                                                .min_w_0()
                                                                .child(
                                                                    div()
                                                                        .truncate()
                                                                        .text_xs()
                                                                        .font_family(".SystemMonoFont")
                                                                        .text_color(t.text_secondary)
                                                                        .child(workspace_path_str),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .id("btn-ws-settings-open-folder")
                                                                        .flex_shrink_0()
                                                                        .px(px(8.))
                                                                        .py(px(3.))
                                                                        .rounded(px(4.))
                                                                        .bg(t.bg_input)
                                                                        .border_1()
                                                                        .border_color(t.border)
                                                                        .text_color(t.text_secondary)
                                                                        .hover(|s| {
                                                                            s.bg(t.bg_hover).text_color(t.text_primary)
                                                                        })
                                                                        .cursor_pointer()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap(px(4.))
                                                                        .text_xs()
                                                                        .on_click(move |_, _, cx| {
                                                                            ws_open.update(cx, |ws, _| {
                                                                                ws.open_workspace_folder();
                                                                            });
                                                                        })
                                                                        .child(
                                                                            Icon::new(IconName::FolderOpen).size(px(12.)),
                                                                        )
                                                                        .child("Reveal"),
                                                                ),
                                                        ),
                                                ),
                                        )
                                        .into_any_element()
                                }
                                ScopeSettingsTab::Headers => {
                                    let ws_up = ws_clone.clone();
                                    let tgt = target.clone();
                                    let pairs = current_headers.clone();

                                    let on_update = {
                                        let ws_s = ws_up.clone();
                                        let tgt = tgt.clone();
                                        move |idx: usize, key: String, val: String, enabled: bool, _window: &mut Window, cx: &mut App| {
                                            let mut cur_headers = match &tgt {
                                                ScopeSettingsTarget::Workspace => {
                                                    ws_s.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default()
                                                }
                                                ScopeSettingsTarget::Folder(fid) => {
                                                    ws_s.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default()
                                                }
                                            };
                                            if idx < cur_headers.len() {
                                                cur_headers[idx].key = key;
                                                cur_headers[idx].value = val;
                                                cur_headers[idx].enabled = enabled;
                                                match &tgt {
                                                    ScopeSettingsTarget::Workspace => {
                                                        let a = ws_s.read(cx).active_workspace.as_ref().map(|w| w.auth.clone()).unwrap_or(Auth::None);
                                                        ws_s.update(cx, |state, cx| state.save_workspace_scope_settings(cur_headers, a, cx));
                                                    }
                                                    ScopeSettingsTarget::Folder(fid) => {
                                                        let a = ws_s.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.auth.clone()).unwrap_or(Auth::Inherit);
                                                        ws_s.update(cx, |state, cx| state.save_folder_scope_settings(fid, cur_headers, a, cx));
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
                                                ScopeSettingsTarget::Workspace => {
                                                    ws_s.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default()
                                                }
                                                ScopeSettingsTarget::Folder(fid) => {
                                                    ws_s.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default()
                                                }
                                            };
                                            cur_headers.push(KeyValuePair::new("", ""));
                                            match &tgt {
                                                ScopeSettingsTarget::Workspace => {
                                                    let a = ws_s.read(cx).active_workspace.as_ref().map(|w| w.auth.clone()).unwrap_or(Auth::None);
                                                    ws_s.update(cx, |state, cx| state.save_workspace_scope_settings(cur_headers, a, cx));
                                                }
                                                ScopeSettingsTarget::Folder(fid) => {
                                                    let a = ws_s.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.auth.clone()).unwrap_or(Auth::Inherit);
                                                    ws_s.update(cx, |state, cx| state.save_folder_scope_settings(fid, cur_headers, a, cx));
                                                }
                                            }
                                        }
                                    };

                                    let on_remove = {
                                        let ws_s = ws_up.clone();
                                        let tgt = tgt.clone();
                                        move |idx: usize, _window: &mut Window, cx: &mut App| {
                                            let mut cur_headers = match &tgt {
                                                ScopeSettingsTarget::Workspace => {
                                                    ws_s.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default()
                                                }
                                                ScopeSettingsTarget::Folder(fid) => {
                                                    ws_s.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default()
                                                }
                                            };
                                            if idx < cur_headers.len() {
                                                cur_headers.remove(idx);
                                                match &tgt {
                                                    ScopeSettingsTarget::Workspace => {
                                                        let a = ws_s.read(cx).active_workspace.as_ref().map(|w| w.auth.clone()).unwrap_or(Auth::None);
                                                        ws_s.update(cx, |state, cx| state.save_workspace_scope_settings(cur_headers, a, cx));
                                                    }
                                                    ScopeSettingsTarget::Folder(fid) => {
                                                        let a = ws_s.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.auth.clone()).unwrap_or(Auth::Inherit);
                                                        ws_s.update(cx, |state, cx| state.save_folder_scope_settings(fid, cur_headers, a, cx));
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
                                                .env_state(self.env_state.clone())
                                                .modal_state(modal_clone.clone()),
                                        )
                                        .into_any_element()
                                }
                                ScopeSettingsTab::Auth => {
                                    let ws_s = ws_clone.clone();
                                    let tgt = target.clone();

                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(14.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(10.))
                                                .child(
                                                    div()
                                                        .w(px(70.))
                                                        .flex_none()
                                                        .text_xs()
                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                        .text_color(t.text_secondary)
                                                        .child("Type:"),
                                                )
                                                .child({
                                                    let ws_drop = ws_s.clone();
                                                    let tgt_drop = tgt.clone();

                                                    Button::new("btn-scope-auth-type-dropdown")
                                                        .ghost()
                                                        .compact()
                                                        .w(px(140.))
                                                        .px(px(8.))
                                                        .py(px(4.))
                                                        .h(px(28.))
                                                        .rounded(px(4.))
                                                        .bg(t.bg_card)
                                                        .border_1()
                                                        .border_color(t.border)
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                                        .text_color(t.text_primary)
                                                                        .child(auth_label),
                                                                )
                                                                .child(
                                                                    Icon::new(IconName::ChevronDown)
                                                                        .size(px(12.))
                                                                        .text_color(t.text_muted),
                                                                ),
                                                        )
                                                        .dropdown_menu(move |menu, _window, _cx| {
                                                            let mut m = menu;
                                                            if is_folder {
                                                                let ws_inh = ws_drop.clone();
                                                                let tgt_inh = tgt_drop.clone();
                                                                m = m.item(
                                                                    PopupMenuItem::new("Inherit")
                                                                        .checked(is_inherit)
                                                                        .on_click(move |_, _, cx| {
                                                                            if let ScopeSettingsTarget::Folder(fid) = &tgt_inh {
                                                                                let cur_h = ws_inh.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                                ws_inh.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::Inherit, cx));
                                                                            }
                                                                        }),
                                                                );
                                                            }

                                                            let ws_none = ws_drop.clone();
                                                            let tgt_none = tgt_drop.clone();
                                                            let ws_bearer = ws_drop.clone();
                                                            let tgt_bearer = tgt_drop.clone();
                                                            let ws_basic = ws_drop.clone();
                                                            let tgt_basic = tgt_drop.clone();
                                                            let ws_api = ws_drop.clone();
                                                            let tgt_api = tgt_drop.clone();

                                                            m.item(
                                                                PopupMenuItem::new("None")
                                                                    .checked(is_none)
                                                                    .on_click(move |_, _, cx| {
                                                                        match &tgt_none {
                                                                            ScopeSettingsTarget::Workspace => {
                                                                                let cur_h = ws_none.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                                ws_none.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, Auth::None, cx));
                                                                            }
                                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                                let cur_h = ws_none.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                                ws_none.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::None, cx));
                                                                            }
                                                                        }
                                                                    }),
                                                            )
                                                            .item(
                                                                PopupMenuItem::new("Bearer Token")
                                                                    .checked(is_bearer)
                                                                    .on_click(move |_, _, cx| {
                                                                        let new_auth = Auth::Bearer { token: String::new() };
                                                                        match &tgt_bearer {
                                                                            ScopeSettingsTarget::Workspace => {
                                                                                let cur_h = ws_bearer.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                                ws_bearer.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                                            }
                                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                                let cur_h = ws_bearer.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                                ws_bearer.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                                            }
                                                                        }
                                                                    }),
                                                            )
                                                            .item(
                                                                PopupMenuItem::new("Basic Auth")
                                                                    .checked(is_basic)
                                                                    .on_click(move |_, _, cx| {
                                                                        let new_auth = Auth::Basic { username: String::new(), password: String::new() };
                                                                        match &tgt_basic {
                                                                            ScopeSettingsTarget::Workspace => {
                                                                                let cur_h = ws_basic.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                                ws_basic.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                                            }
                                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                                let cur_h = ws_basic.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                                ws_basic.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                                            }
                                                                        }
                                                                    }),
                                                            )
                                                            .item(
                                                                PopupMenuItem::new("API Key")
                                                                    .checked(is_apikey)
                                                                    .on_click(move |_, _, cx| {
                                                                        let new_auth = Auth::ApiKey { key: "X-API-Key".to_string(), value: String::new(), add_to: ApiKeyLocation::Header };
                                                                        match &tgt_api {
                                                                            ScopeSettingsTarget::Workspace => {
                                                                                let cur_h = ws_api.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                                ws_api.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                                            }
                                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                                let cur_h = ws_api.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                                ws_api.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                                            }
                                                                        }
                                                                    }),
                                                            )
                                                        })
                                                }),
                                        )
                                        .child({
                                            match current_auth {
                                                Auth::Inherit => {
                                                    let desc = ws_s.read(cx).get_inherited_auth_description(folder_id_opt.as_deref());
                                                    div()
                                                        .p(px(10.))
                                                        .rounded(px(6.))
                                                        .bg(t.bg_app)
                                                        .border_1()
                                                        .border_color(t.border)
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(8.))
                                                        .child(Icon::new(IconName::Info).size(px(16.)))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(t.text_secondary)
                                                                .child(desc),
                                                        )
                                                }
                                                Auth::None => {
                                                    div()
                                                        .pt(px(4.))
                                                        .text_xs()
                                                        .italic()
                                                        .text_color(t.text_muted)
                                                        .child(if is_folder {
                                                            "No authentication will be added to requests in this folder."
                                                        } else {
                                                            "No authentication will be added to requests in this workspace."
                                                        })
                                                }
                                                Auth::Bearer { .. } => {
                                                    let ws_s = ws_clone.clone();
                                                    let tgt = target.clone();
                                                    let on_bearer_update = move |v: String, _window: &mut Window, cx: &mut App| {
                                                        let new_auth = Auth::Bearer { token: v };
                                                        match &tgt {
                                                            ScopeSettingsTarget::Workspace => {
                                                                let cur_h = ws_s.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                ws_s.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                            }
                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                let cur_h = ws_s.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                ws_s.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                            }
                                                        }
                                                    };

                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(10.))
                                                        .child(
                                                            div()
                                                                .w(px(70.))
                                                                .flex_none()
                                                                .text_xs()
                                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                .text_color(t.text_secondary)
                                                                .child("Token:"),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .child(render_env_input(
                                                                    "scope-auth-bearer-token",
                                                                    &bearer_input,
                                                                    Some(&self.env_state),
                                                                    Some(&self.modal_state),
                                                                    window,
                                                                    cx,
                                                                    on_bearer_update,
                                                                )),
                                                        )
                                                }
                                                Auth::Basic { .. } => {
                                                    let ws_s_u = ws_clone.clone();
                                                    let tgt_u = target.clone();
                                                    let on_user_update = move |u: String, _window: &mut Window, cx: &mut App| {
                                                        let pass = match &tgt_u {
                                                            ScopeSettingsTarget::Workspace => match &ws_s_u.read(cx).workspace_meta.auth {
                                                                Auth::Basic { password, .. } => password.clone(),
                                                                _ => String::new(),
                                                            },
                                                            ScopeSettingsTarget::Folder(fid) => match ws_s_u.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                                                Some(Auth::Basic { password, .. }) => password.clone(),
                                                                _ => String::new(),
                                                            },
                                                        };
                                                        let new_auth = Auth::Basic { username: u, password: pass };
                                                        match &tgt_u {
                                                            ScopeSettingsTarget::Workspace => {
                                                                let cur_h = ws_s_u.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                ws_s_u.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                            }
                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                let cur_h = ws_s_u.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                ws_s_u.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                            }
                                                        }
                                                    };

                                                    let ws_s_p = ws_clone.clone();
                                                    let tgt_p = target.clone();
                                                    let on_pass_update = move |p: String, _window: &mut Window, cx: &mut App| {
                                                        let user = match &tgt_p {
                                                            ScopeSettingsTarget::Workspace => match &ws_s_p.read(cx).workspace_meta.auth {
                                                                Auth::Basic { username, .. } => username.clone(),
                                                                _ => String::new(),
                                                            },
                                                            ScopeSettingsTarget::Folder(fid) => match ws_s_p.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                                                Some(Auth::Basic { username, .. }) => username.clone(),
                                                                _ => String::new(),
                                                            },
                                                        };
                                                        let new_auth = Auth::Basic { username: user, password: p };
                                                        match &tgt_p {
                                                            ScopeSettingsTarget::Workspace => {
                                                                let cur_h = ws_s_p.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                ws_s_p.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                            }
                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                let cur_h = ws_s_p.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                ws_s_p.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                            }
                                                        }
                                                    };

                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(8.))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(10.))
                                                                .child(
                                                                    div()
                                                                        .w(px(70.))
                                                                        .flex_none()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                        .text_color(t.text_secondary)
                                                                        .child("Username:"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .child(render_env_input(
                                                                            "scope-auth-basic-user",
                                                                            &basic_user_input,
                                                                            Some(&self.env_state),
                                                                            Some(&self.modal_state),
                                                                            window,
                                                                            cx,
                                                                            on_user_update,
                                                                        )),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(10.))
                                                                .child(
                                                                    div()
                                                                        .w(px(70.))
                                                                        .flex_none()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                        .text_color(t.text_secondary)
                                                                        .child("Password:"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .child(render_env_input(
                                                                            "scope-auth-basic-pass",
                                                                            &basic_pass_input,
                                                                            Some(&self.env_state),
                                                                            Some(&self.modal_state),
                                                                            window,
                                                                            cx,
                                                                            on_pass_update,
                                                                        )),
                                                                ),
                                                        )
                                                }
                                                Auth::ApiKey { add_to, .. } => {
                                                    let add_to_val = add_to;
                                                    let ws_s_k = ws_clone.clone();
                                                    let tgt_k = target.clone();
                                                    let on_api_key_update = move |k: String, _window: &mut Window, cx: &mut App| {
                                                        let (v, cur_loc) = match &tgt_k {
                                                            ScopeSettingsTarget::Workspace => match &ws_s_k.read(cx).workspace_meta.auth {
                                                                Auth::ApiKey { value, add_to, .. } => (value.clone(), *add_to),
                                                                _ => (String::new(), ApiKeyLocation::Header),
                                                            },
                                                            ScopeSettingsTarget::Folder(fid) => match ws_s_k.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                                                Some(Auth::ApiKey { value, add_to, .. }) => (value.clone(), *add_to),
                                                                _ => (String::new(), ApiKeyLocation::Header),
                                                            },
                                                        };
                                                        let new_auth = Auth::ApiKey { key: k, value: v, add_to: cur_loc };
                                                        match &tgt_k {
                                                            ScopeSettingsTarget::Workspace => {
                                                                let cur_h = ws_s_k.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                ws_s_k.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                            }
                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                let cur_h = ws_s_k.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                ws_s_k.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                            }
                                                        }
                                                    };

                                                    let ws_s_v = ws_clone.clone();
                                                    let tgt_v = target.clone();
                                                    let on_api_val_update = move |v: String, _window: &mut Window, cx: &mut App| {
                                                        let (k, cur_loc) = match &tgt_v {
                                                            ScopeSettingsTarget::Workspace => match &ws_s_v.read(cx).workspace_meta.auth {
                                                                Auth::ApiKey { key, add_to, .. } => (key.clone(), *add_to),
                                                                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                                                            },
                                                            ScopeSettingsTarget::Folder(fid) => match ws_s_v.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                                                Some(Auth::ApiKey { key, add_to, .. }) => (key.clone(), *add_to),
                                                                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                                                            },
                                                        };
                                                        let new_auth = Auth::ApiKey { key: k, value: v, add_to: cur_loc };
                                                        match &tgt_v {
                                                            ScopeSettingsTarget::Workspace => {
                                                                let cur_h = ws_s_v.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                ws_s_v.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                                            }
                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                let cur_h = ws_s_v.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                ws_s_v.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                                            }
                                                        }
                                                    };

                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(8.))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(10.))
                                                                .child(
                                                                    div()
                                                                        .w(px(70.))
                                                                        .flex_none()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                        .text_color(t.text_secondary)
                                                                        .child("Key:"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .child(render_env_input(
                                                                            "scope-auth-apikey-key",
                                                                            &api_key_input,
                                                                            Some(&self.env_state),
                                                                            Some(&self.modal_state),
                                                                            window,
                                                                            cx,
                                                                            on_api_key_update,
                                                                        )),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(10.))
                                                                .child(
                                                                    div()
                                                                        .w(px(70.))
                                                                        .flex_none()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                        .text_color(t.text_secondary)
                                                                        .child("Value:"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .child(render_env_input(
                                                                            "scope-auth-apikey-val",
                                                                            &api_val_input,
                                                                            Some(&self.env_state),
                                                                            Some(&self.modal_state),
                                                                            window,
                                                                            cx,
                                                                            on_api_val_update,
                                                                        )),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(10.))
                                                                .child(
                                                                    div()
                                                                        .w(px(70.))
                                                                        .flex_none()
                                                                        .text_xs()
                                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                                        .text_color(t.text_secondary)
                                                                        .child("Add to:"),
                                                                )
                                                                .child({
                                                                    let loc_label = if add_to_val.is_header() { "Header" } else { "Query Params" };
                                                                    let ws_loc = ws_s.clone();
                                                                    let tgt_loc = tgt.clone();
                                                                    let k_input = api_key_input.clone();
                                                                    let v_input = api_val_input.clone();

                                                                    Button::new("btn-scope-auth-apikey-location-dropdown")
                                                                        .ghost()
                                                                        .compact()
                                                                        .min_w(px(120.))
                                                                        .px(px(8.))
                                                                        .py(px(4.))
                                                                        .h(px(28.))
                                                                        .rounded(px(4.))
                                                                        .bg(t.bg_card)
                                                                        .border_1()
                                                                        .border_color(t.border)
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                                                .text_color(t.text_primary)
                                                                                .child(loc_label),
                                                                        )
                                                                        .dropdown_menu(move |menu, _window, _cx| {
                                                                            let ws_h = ws_loc.clone();
                                                                            let tgt_h = tgt_loc.clone();
                                                                            let k_h = k_input.clone();
                                                                            let v_h = v_input.clone();

                                                                            let ws_q = ws_loc.clone();
                                                                            let tgt_q = tgt_loc.clone();
                                                                            let k_q = k_input.clone();
                                                                            let v_q = v_input.clone();

                                                                            menu.item(
                                                                                PopupMenuItem::new("Header")
                                                                                    .checked(add_to_val.is_header())
                                                                                    .on_click(move |_, _, cx| {
                                                                                        let key = k_h.read(cx).value().to_string();
                                                                                        let val = v_h.read(cx).value().to_string();
                                                                                        match &tgt_h {
                                                                                            ScopeSettingsTarget::Workspace => {
                                                                                                let cur_h = ws_h.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                                                ws_h.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Header }, cx));
                                                                                            }
                                                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                                                let cur_h = ws_h.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                                                ws_h.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Header }, cx));
                                                                                            }
                                                                                        }
                                                                                    }),
                                                                            )
                                                                            .item(
                                                                                PopupMenuItem::new("Query Params")
                                                                                    .checked(add_to_val.is_query())
                                                                                    .on_click(move |_, _, cx| {
                                                                                        let key = k_q.read(cx).value().to_string();
                                                                                        let val = v_q.read(cx).value().to_string();
                                                                                        match &tgt_q {
                                                                                            ScopeSettingsTarget::Workspace => {
                                                                                                let cur_h = ws_q.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                                                ws_q.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Query }, cx));
                                                                                            }
                                                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                                                let cur_h = ws_q.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                                                ws_q.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Query }, cx));
                                                                                            }
                                                                                        }
                                                                                    }),
                                                                            )
                                                                        })
                                                                }),
                                                        )
                                                }
                                            }
                                        })
                                        .into_any_element()
                                }
                            }),
                    )
            .into_any_element()
    }
}
