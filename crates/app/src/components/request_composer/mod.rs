mod auth_tab;
mod banners;
mod body_tab;
pub(crate) mod composer_inputs;
mod editor_diagnostics;
mod graphql_completion;
mod graphql_query_tab;
mod grpc_completion;
mod grpc_message_tab;
mod grpc_proto_tab;
mod headers_tab;
mod message_tab;
mod params_tab;
mod proto_tab;
mod scripts_tab;
mod tab_strip;
mod ws_message_tab;

pub(crate) use editor_diagnostics::{
    refresh_graphql_variables_validation, refresh_grpc_validation,
};

use crate::app::AppView;
use crate::components::*;
use crate::state::*;
use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use zest_core::models::Auth;

impl AppView {
    pub(crate) fn render_request_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let active_tab_kind = self.state.session.read(cx).active_tab_kind;
        let req_id_str = self.state.session.read(cx).active_request_id().to_string();
        let app_view_entity = cx.entity().clone();

        let active_conflict = self
            .state
            .persistence
            .read(cx)
            .conflicts
            .get(&req_id_str)
            .cloned();
        let active_parse_err = self
            .state
            .collection
            .read(cx)
            .parse_errors
            .get(&req_id_str)
            .cloned();
        let corrupt_sec_err = self.state.env.read(cx).corrupt_secrets_warning.clone();

        div()
            .size_full()
            .bg(t.bg_editor)
            .relative()
            .flex()
            .flex_col()
            // Conflict banner
            .when_some(active_conflict, |this, msg| {
                this.child(banners::conflict_banner(self, &req_id_str, msg))
            })
            // Parse error banner
            .when_some(active_parse_err, |this, err| {
                this.child(banners::parse_error_banner(err))
            })
            // Corrupt secrets banner
            .when_some(corrupt_sec_err, |this, msg| {
                this.child(banners::corrupt_secrets_banner(msg))
            })
            // Top Request Bar: Method + URL + Send
            .child({
                let view_handle = app_view_entity.clone();
                let view_handle_disc = app_view_entity.clone();
                RequestBar::new(
                    self.state.collection.clone(),
                    self.state.env.clone(),
                    self.state.modal.clone(),
                    self.state.session.clone(),
                    self.state.response.clone(),
                    self.composer_inputs.url_input.clone(),
                    RequestBarActions {
                        // `send_current_request` routes by protocol itself.
                        on_send: std::rc::Rc::new(move |_, cx| {
                            view_handle.update(cx, |view, cx| view.send_current_request(cx));
                        }),
                        on_disconnect: std::rc::Rc::new(move |_, cx| {
                            view_handle_disc.update(cx, |view, cx| view.disconnect_websocket(cx));
                        }),
                    },
                )
            })
            // Request Tabs with TabBar — the strip depends on the request's
            // transport protocol (HTTP: Body..scripts, WS/gRPC: Message..)
            .child(tab_strip::render_tab_strip(self, active_tab_kind, cx))
            // Request Tab Content
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(match active_tab_kind {
                        RequestTabKind::Body => body_tab::render_body_tab(self),
                        RequestTabKind::Message => {
                            message_tab::render_message_tab(self, window, cx)
                        }
                        RequestTabKind::Query => {
                            graphql_query_tab::render_query_tab(self, window, cx)
                        }
                        RequestTabKind::Proto => proto_tab::render_proto_tab(self, window, cx),
                        RequestTabKind::Params => params_tab::render_params_tab(self, cx),
                        RequestTabKind::Headers => headers_tab::render_headers_tab(self, cx),
                        RequestTabKind::Auth => {
                            let (auth, folder_id) = {
                                let ws_read = self.state.collection.read(cx);
                                let active_r = ws_read.find_request(&req_id_str);
                                (
                                    active_r.map(|r| r.auth.clone()).unwrap_or(Auth::Inherit),
                                    active_r.and_then(|r| r.folder_id.clone()),
                                )
                            };
                            auth_tab::render_auth_tab(
                                &auth,
                                folder_id.as_deref(),
                                auth_tab::AuthTabContext {
                                    workspace_state: &self.state.collection,
                                    env_state: &self.state.env,
                                    modal_state: &self.state.modal,
                                    editor_state: &self.state.session,
                                },
                                auth_tab::AuthTabInputs {
                                    bearer_token_input: &self
                                        .composer_inputs
                                        .auth
                                        .bearer_token_input,
                                    basic_user_input: &self.composer_inputs.auth.basic_user_input,
                                    basic_pass_input: &self.composer_inputs.auth.basic_pass_input,
                                    apikey_key_input: &self.composer_inputs.auth.apikey_key_input,
                                    apikey_val_input: &self.composer_inputs.auth.apikey_val_input,
                                },
                                window,
                                cx,
                            )
                            .into_any_element()
                        }
                        RequestTabKind::PreRequestScript => scripts_tab::render_script_tab(
                            self,
                            &self.composer_inputs.pre_script_editor,
                            ScriptEditorKind::PreRequest,
                            cx,
                        ),
                        RequestTabKind::Tests => scripts_tab::render_script_tab(
                            self,
                            &self.composer_inputs.test_script_editor,
                            ScriptEditorKind::Tests,
                            cx,
                        ),
                    }),
            )
            .into_any_element()
    }
}
