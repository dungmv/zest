use crate::env::{build_tokenized_content, detect_unclosed_env_var};
use crate::state::{CollectionState, EnvironmentState, WorkspaceSettingTarget, WorkspaceState};
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

fn read_target_auth(
    ws: &Entity<WorkspaceState>,
    col: &Entity<CollectionState>,
    target: &WorkspaceSettingTarget,
    cx: &App,
) -> Option<Auth> {
    match target {
        WorkspaceSettingTarget::Workspace => Some(ws.read(cx).workspace_meta().auth.clone()),
        WorkspaceSettingTarget::Folder(fid) => col
            .read(cx)
            .folders()
            .iter()
            .find(|f| &f.id == fid)
            .map(|f| f.auth.clone()),
    }
}

fn save_target_auth(
    ws: &Entity<WorkspaceState>,
    col: &Entity<CollectionState>,
    target: &WorkspaceSettingTarget,
    new_auth: Auth,
    cx: &mut App,
) {
    match target {
        WorkspaceSettingTarget::Workspace => {
            let cur_h = ws.read(cx).workspace_meta().headers.clone();
            ws.update(cx, |state, cx| {
                state.save_workspace_settings(cur_h, new_auth, cx)
            });
        }
        WorkspaceSettingTarget::Folder(fid) => {
            let cur_h = col
                .read(cx)
                .folders()
                .iter()
                .find(|f| &f.id == fid)
                .map(|f| f.headers.clone())
                .unwrap_or_default();
            col.update(cx, |state, cx| {
                state.save_folder_settings(fid, cur_h, new_auth, cx)
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn use_auth_field_input(
    window: &mut Window,
    cx: &mut App,
    key: String,
    placeholder: &'static str,
    init_val: String,
    ws: &Entity<WorkspaceState>,
    col: &Entity<CollectionState>,
    env: &Entity<EnvironmentState>,
    target: &WorkspaceSettingTarget,
    build_auth: impl Fn(String, Option<Auth>) -> Auth + 'static,
) -> Entity<InputState> {
    let ws_c = ws.clone();
    let col_c = col.clone();
    let env_c = env.clone();
    let tgt = target.clone();
    window
        .use_keyed_state(gpui_kit::SharedString::from(key), cx, |window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            let content = build_tokenized_content(&init_val, env_c.read(cx));
            input.update(cx, |s, cx| s.set_value(content, window, cx));

            let sub = cx.subscribe_in(&input, window, move |_, inp, ev, window, cx| {
                if let InputEvent::Change = ev {
                    let val = inp.read(cx).value().to_string();
                    let cur = read_target_auth(&ws_c, &col_c, &tgt, cx);
                    let new_auth = build_auth(val.clone(), cur);
                    save_target_auth(&ws_c, &col_c, &tgt, new_auth, cx);
                    if val.contains("}}") && detect_unclosed_env_var(&val).is_none() {
                        let tokenized = build_tokenized_content(&val, env_c.read(cx));
                        inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                    }
                }
            });
            (input, sub)
        })
        .read(cx)
        .0
        .clone()
}

pub fn use_scope_auth_inputs(
    window: &mut Window,
    cx: &mut App,
    target: &WorkspaceSettingTarget,
    current_auth: &Auth,
    ws_state: &Entity<WorkspaceState>,
    col_state: &Entity<CollectionState>,
    env_state: &Entity<EnvironmentState>,
) -> ScopeAuthInputs {
    let bearer_init = match current_auth {
        Auth::Bearer { token } => token.clone(),
        _ => String::new(),
    };
    let bearer_input = use_auth_field_input(
        window,
        cx,
        format!("ws-setting-auth-bearer-{:?}", target),
        "Token or {{token}}",
        bearer_init,
        ws_state,
        col_state,
        env_state,
        target,
        |token, _| Auth::Bearer { token },
    );

    let basic_u_init = match current_auth {
        Auth::Basic { username, .. } => username.clone(),
        _ => String::new(),
    };
    let basic_user_input = use_auth_field_input(
        window,
        cx,
        format!("ws-setting-auth-basic-u-{:?}", target),
        "Username",
        basic_u_init,
        ws_state,
        col_state,
        env_state,
        target,
        |username, cur| {
            let password = match cur {
                Some(Auth::Basic { password, .. }) => password,
                _ => String::new(),
            };
            Auth::Basic { username, password }
        },
    );

    let basic_p_init = match current_auth {
        Auth::Basic { password, .. } => password.clone(),
        _ => String::new(),
    };
    let basic_pass_input = use_auth_field_input(
        window,
        cx,
        format!("ws-setting-auth-basic-p-{:?}", target),
        "Password",
        basic_p_init,
        ws_state,
        col_state,
        env_state,
        target,
        |password, cur| {
            let username = match cur {
                Some(Auth::Basic { username, .. }) => username,
                _ => String::new(),
            };
            Auth::Basic { username, password }
        },
    );

    let api_k_init = match current_auth {
        Auth::ApiKey { key, .. } => key.clone(),
        _ => "X-API-Key".to_string(),
    };
    let api_key_input = use_auth_field_input(
        window,
        cx,
        format!("ws-setting-auth-apikey-k-{:?}", target),
        "Header / Param Name",
        api_k_init,
        ws_state,
        col_state,
        env_state,
        target,
        |key, cur| {
            let (value, add_to) = match cur {
                Some(Auth::ApiKey { value, add_to, .. }) => (value, add_to),
                _ => (String::new(), ApiKeyLocation::Header),
            };
            Auth::ApiKey { key, value, add_to }
        },
    );

    let api_v_init = match current_auth {
        Auth::ApiKey { value, .. } => value.clone(),
        _ => String::new(),
    };
    let api_val_input = use_auth_field_input(
        window,
        cx,
        format!("ws-setting-auth-apikey-v-{:?}", target),
        "Value or {{secret}}",
        api_v_init,
        ws_state,
        col_state,
        env_state,
        target,
        |value, cur| {
            let (key, add_to) = match cur {
                Some(Auth::ApiKey { key, add_to, .. }) => (key, add_to),
                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
            };
            Auth::ApiKey { key, value, add_to }
        },
    );

    ScopeAuthInputs {
        bearer_input,
        basic_user_input,
        basic_pass_input,
        api_key_input,
        api_val_input,
    }
}
