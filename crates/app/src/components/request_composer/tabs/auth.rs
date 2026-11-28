use std::sync::Arc;

use crate::app::AppView;
use crate::components::auth_form::{
    render_auth_form, AuthFormContext, AuthFormIds, AuthFormInputs, AuthFormOptions,
};
use crate::state::{CollectionState, EnvironmentState, ModalState, RequestSessionState};
use gpui_kit::component::input::InputState;
use gpui_kit::{AnyElement, App, Context, Div, Entity, IntoElement, Window};
use zest_core::models::Auth;

struct AuthTabInputs<'a> {
    pub bearer_token_input: &'a Entity<InputState>,
    pub basic_user_input: &'a Entity<InputState>,
    pub basic_pass_input: &'a Entity<InputState>,
    pub apikey_key_input: &'a Entity<InputState>,
    pub apikey_val_input: &'a Entity<InputState>,
}

struct AuthTabContext<'a> {
    pub workspace_state: &'a Entity<CollectionState>,
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

pub(in crate::components::request_composer) fn render_auth_tab(
    av: &AppView,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let req_id = av.state.session.read(cx).active_request_id();
    let active_request = av.state.collection.read(cx).find_request(req_id);
    let (auth, folder_id) = active_request
        .map(|request| (&request.auth, request.folder_id.as_deref()))
        .unwrap_or((&Auth::Inherit, None));
    let auth = auth.clone();
    let folder_id = folder_id.map(str::to_owned);
    let inputs = &av.composer_inputs.auth;
    render_request_auth_form(
        &auth,
        folder_id.as_deref(),
        AuthTabContext {
            workspace_state: &av.state.collection,
            env_state: &av.state.env,
            modal_state: &av.state.modal,
            editor_state: &av.state.session,
        },
        AuthTabInputs {
            bearer_token_input: &inputs.bearer_token_input,
            basic_user_input: &inputs.basic_user_input,
            basic_pass_input: &inputs.basic_pass_input,
            apikey_key_input: &inputs.apikey_key_input,
            apikey_val_input: &inputs.apikey_val_input,
        },
        window,
        cx,
    )
    .into_any_element()
}

fn render_request_auth_form(
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
