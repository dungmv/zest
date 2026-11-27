use std::sync::Arc;

use crate::components::auth_form::{
    render_auth_form, AuthFormContext, AuthFormIds, AuthFormInputs, AuthFormOptions,
};
use crate::state::{EnvironmentState, ModalState, WorkspaceDataState, WorkspaceSettingTarget};
use gpui_kit::component::input::InputState;
use gpui_kit::{App, Entity, IntoElement, Window};
use zest_core::models::Auth;

pub struct AuthTabContext<'a> {
    pub ws_state: &'a Entity<WorkspaceDataState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub modal_state: &'a Entity<ModalState>,
    pub target: &'a WorkspaceSettingTarget,
    pub current_auth: &'a Auth,
    pub is_folder: bool,
    pub folder_id_opt: Option<&'a str>,
    pub bearer_input: &'a Entity<InputState>,
    pub basic_user_input: &'a Entity<InputState>,
    pub basic_pass_input: &'a Entity<InputState>,
    pub api_key_input: &'a Entity<InputState>,
    pub api_val_input: &'a Entity<InputState>,
}

const WS_SETTING_AUTH_IDS: AuthFormIds = AuthFormIds {
    type_dropdown: "btn-ws-setting-auth-type-dropdown",
    bearer_token: "ws-setting-auth-bearer-token",
    basic_user: "ws-setting-auth-basic-user",
    basic_pass: "ws-setting-auth-basic-pass",
    apikey_key: "ws-setting-auth-apikey-key",
    apikey_val: "ws-setting-auth-apikey-val",
    apikey_location_dropdown: "btn-ws-setting-auth-apikey-location-dropdown",
};

fn save_target_auth(
    ws: &Entity<WorkspaceDataState>,
    target: &WorkspaceSettingTarget,
    new_auth: Auth,
    cx: &mut App,
) {
    match target {
        WorkspaceSettingTarget::Workspace => {
            let cur_h = ws
                .read(cx)
                .active_workspace
                .as_ref()
                .map(|w| w.headers.clone())
                .unwrap_or_default();
            ws.update(cx, |s, cx| s.save_workspace_settings(cur_h, new_auth, cx));
        }
        WorkspaceSettingTarget::Folder(fid) => {
            let cur_h = ws
                .read(cx)
                .folders
                .iter()
                .find(|f| &f.id == fid)
                .map(|f| f.headers.clone())
                .unwrap_or_default();
            ws.update(cx, |s, cx| s.save_folder_settings(fid, cur_h, new_auth, cx));
        }
    }
}

pub fn render_auth_tab(
    ctx: AuthTabContext<'_>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let ws = ctx.ws_state.clone();
    let target = ctx.target.clone();
    let on_save = Arc::new(move |new_auth: Auth, cx: &mut App| {
        save_target_auth(&ws, &target, new_auth, cx);
    });

    let none_message = if ctx.is_folder {
        "No authentication will be added to requests in this folder."
    } else {
        "No authentication will be added to requests in this workspace."
    };

    render_auth_form(
        AuthFormContext {
            auth: ctx.current_auth,
            folder_id: ctx.folder_id_opt,
            workspace_state: ctx.ws_state,
            env_state: ctx.env_state,
            modal_state: ctx.modal_state,
            inputs: AuthFormInputs {
                bearer_token: ctx.bearer_input,
                basic_user: ctx.basic_user_input,
                basic_pass: ctx.basic_pass_input,
                apikey_key: ctx.api_key_input,
                apikey_val: ctx.api_val_input,
            },
            options: AuthFormOptions {
                ids: WS_SETTING_AUTH_IDS,
                allow_inherit: ctx.is_folder,
                show_inherited_preview: false,
                compact_rows: false,
                none_message,
            },
            on_save,
        },
        window,
        cx,
    )
}
