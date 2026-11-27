use std::sync::Arc;

use crate::components::auth_form::{
    render_auth_form, AuthFormContext, AuthFormIds, AuthFormInputs, AuthFormOptions,
};
use crate::state::{EnvironmentState, ModalState, RequestSessionState, WorkspaceDataState};
use gpui_kit::component::input::InputState;
use gpui_kit::{App, Div, Entity, Window};
use zest_core::models::Auth;

pub(super) struct AuthTabInputs<'a> {
    pub bearer_token_input: &'a Entity<InputState>,
    pub basic_user_input: &'a Entity<InputState>,
    pub basic_pass_input: &'a Entity<InputState>,
    pub apikey_key_input: &'a Entity<InputState>,
    pub apikey_val_input: &'a Entity<InputState>,
}

pub(super) struct AuthTabContext<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub modal_state: &'a Entity<ModalState>,
    pub editor_state: &'a Entity<RequestSessionState>,
}

const REQUEST_AUTH_IDS: AuthFormIds = AuthFormIds {
    type_dropdown: "btn-auth-type-dropdown",
    bearer_token: "auth-bearer-token-input",
    basic_user: "auth-basic-user-input",
    basic_pass: "auth-basic-pass-input",
    apikey_key: "auth-apikey-key-input",
    apikey_val: "auth-apikey-val-input",
    apikey_location_dropdown: "btn-auth-apikey-location-dropdown",
};

pub(super) fn render_auth_tab(
    auth: &Auth,
    folder_id: Option<&str>,
    ctx: AuthTabContext,
    inputs: AuthTabInputs,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let ws = ctx.workspace_state.clone();
    let ed = ctx.editor_state.clone();
    let req_id = ctx.editor_state.read(cx).active_request_id().to_string();

    let on_save = Arc::new(move |new_auth: Auth, cx: &mut App| {
        ws.update(cx, |ws, _| {
            ws.update_active_request(&req_id, |r| {
                r.set_auth(new_auth);
            });
        });
        ed.update(cx, |ed, cx| {
            ed.mark_dirty(cx);
        });
    });

    render_auth_form(
        AuthFormContext {
            auth,
            folder_id,
            workspace_state: ctx.workspace_state,
            env_state: ctx.env_state,
            modal_state: ctx.modal_state,
            inputs: AuthFormInputs {
                bearer_token: inputs.bearer_token_input,
                basic_user: inputs.basic_user_input,
                basic_pass: inputs.basic_pass_input,
                apikey_key: inputs.apikey_key_input,
                apikey_val: inputs.apikey_val_input,
            },
            options: AuthFormOptions {
                ids: REQUEST_AUTH_IDS,
                allow_inherit: true,
                show_inherited_preview: true,
                compact_rows: true,
                none_message: "No authentication will be added to this request.",
            },
            on_save,
        },
        window,
        cx,
    )
}
