use crate::components::env_autocomplete::{build_tokenized_content, detect_unclosed_env_var};
use crate::state::{EnvironmentState, ScopeSettingsTarget, WorkspaceDataState};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext as _, Entity, Window};
use zest_core::models::{ApiKeyLocation, Auth};

pub struct ScopeAuthInputs {
    pub bearer_input: Entity<InputState>,
    pub basic_user_input: Entity<InputState>,
    pub basic_pass_input: Entity<InputState>,
    pub api_key_input: Entity<InputState>,
    pub api_val_input: Entity<InputState>,
}

pub fn use_scope_auth_inputs(
    window: &mut Window,
    cx: &mut App,
    target: &ScopeSettingsTarget,
    current_auth: &Auth,
    ws_state: &Entity<WorkspaceDataState>,
    env_state: &Entity<EnvironmentState>,
) -> ScopeAuthInputs {
    let ws_clone = ws_state.clone();
    let env_clone = env_state.clone();

    // 1. Bearer Token Input
    let bearer_token_holder = window.use_keyed_state(
        gpui_kit::SharedString::from(format!("scope-auth-bearer-{:?}", target)),
        cx,
        |window, cx| {
            let init_val = match current_auth {
                Auth::Bearer { token } => token.clone(),
                _ => String::new(),
            };
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Token or {{token}}"));
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

    // 2. Basic User Input
    let basic_user_holder = window.use_keyed_state(
        gpui_kit::SharedString::from(format!("scope-auth-basic-u-{:?}", target)),
        cx,
        |window, cx| {
            let init_val = match current_auth {
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
                        ScopeSettingsTarget::Folder(fid) => {
                            match ws_s
                                .read(cx)
                                .folders
                                .iter()
                                .find(|f| &f.id == fid)
                                .map(|f| &f.auth)
                            {
                                Some(Auth::Basic { password, .. }) => password.clone(),
                                _ => String::new(),
                            }
                        }
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

    // 3. Basic Password Input
    let basic_pass_holder = window.use_keyed_state(
        gpui_kit::SharedString::from(format!("scope-auth-basic-p-{:?}", target)),
        cx,
        |window, cx| {
            let init_val = match current_auth {
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
                        ScopeSettingsTarget::Folder(fid) => {
                            match ws_s
                                .read(cx)
                                .folders
                                .iter()
                                .find(|f| &f.id == fid)
                                .map(|f| &f.auth)
                            {
                                Some(Auth::Basic { username, .. }) => username.clone(),
                                _ => String::new(),
                            }
                        }
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

    // 4. API Key Header/Param Name
    let api_key_holder = window.use_keyed_state(
        gpui_kit::SharedString::from(format!("scope-auth-apikey-k-{:?}", target)),
        cx,
        |window, cx| {
            let init_val = match current_auth {
                Auth::ApiKey { key, .. } => key.clone(),
                _ => "X-API-Key".to_string(),
            };
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Header / Param Name"));
            let content = build_tokenized_content(&init_val, env_clone.read(cx));
            input.update(cx, |s, cx| s.set_value(content, window, cx));

            let ws_s = ws_clone.clone();
            let tgt = target.clone();
            let env_sub = env_clone.clone();
            let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                if let InputEvent::Change = ev {
                    let k = inp.read(cx).value().to_string();
                    let (v, cur_loc) = match &tgt {
                        ScopeSettingsTarget::Workspace => {
                            match &ws_s.read(cx).workspace_meta.auth {
                                Auth::ApiKey { value, add_to, .. } => (value.clone(), *add_to),
                                _ => (String::new(), ApiKeyLocation::Header),
                            }
                        }
                        ScopeSettingsTarget::Folder(fid) => {
                            match ws_s
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
                            }
                        }
                    };
                    let new_auth = Auth::ApiKey {
                        key: k.clone(),
                        value: v,
                        add_to: cur_loc,
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

    // 5. API Key Value
    let api_val_holder = window.use_keyed_state(
        gpui_kit::SharedString::from(format!("scope-auth-apikey-v-{:?}", target)),
        cx,
        |window, cx| {
            let init_val = match current_auth {
                Auth::ApiKey { value, .. } => value.clone(),
                _ => String::new(),
            };
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Value or {{secret}}"));
            let content = build_tokenized_content(&init_val, env_clone.read(cx));
            input.update(cx, |s, cx| s.set_value(content, window, cx));

            let ws_s = ws_clone.clone();
            let tgt = target.clone();
            let env_sub = env_clone.clone();
            let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                if let InputEvent::Change = ev {
                    let v = inp.read(cx).value().to_string();
                    let (k, cur_loc) = match &tgt {
                        ScopeSettingsTarget::Workspace => {
                            match &ws_s.read(cx).workspace_meta.auth {
                                Auth::ApiKey { key, add_to, .. } => (key.clone(), *add_to),
                                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                            }
                        }
                        ScopeSettingsTarget::Folder(fid) => {
                            match ws_s
                                .read(cx)
                                .folders
                                .iter()
                                .find(|f| &f.id == fid)
                                .map(|f| &f.auth)
                            {
                                Some(Auth::ApiKey { key, add_to, .. }) => (key.clone(), *add_to),
                                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                            }
                        }
                    };
                    let new_auth = Auth::ApiKey {
                        key: k,
                        value: v.clone(),
                        add_to: cur_loc,
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

    ScopeAuthInputs {
        bearer_input,
        basic_user_input,
        basic_pass_input,
        api_key_input,
        api_val_input,
    }
}
