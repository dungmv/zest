mod banners;
mod body;
mod composer_inputs;
mod editor_diagnostics;
mod graphql;
mod grpc;
mod request_bar;
mod scripts;
mod tabs;

pub(crate) use composer_inputs::ComposerInputs;
pub use tabs::RequestTabKind;

pub(crate) use editor_diagnostics::{
    refresh_graphql_variables_validation, refresh_grpc_validation,
};

use crate::app::AppView;
use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use request_bar::{RequestBar, RequestBarActions};
use scripts::ScriptEditorKind;

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
            .request_conflict(&req_id_str)
            .map(str::to_owned);
        let active_parse_err = self
            .state
            .collection
            .read(cx)
            .parse_errors()
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
            .child(tabs::render_tab_strip(self, active_tab_kind, cx))
            // Request Tab Content
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(match active_tab_kind {
                        RequestTabKind::Body => body::render_body_tab(self),
                        RequestTabKind::Message => tabs::render_message_tab(self, window, cx),
                        RequestTabKind::Query => graphql::render_query_tab(self, window, cx),
                        RequestTabKind::Proto => grpc::render_proto_tab(self, window, cx),
                        RequestTabKind::Params => tabs::render_params_tab(self, cx),
                        RequestTabKind::Headers => tabs::render_headers_tab(self, cx),
                        RequestTabKind::Auth => tabs::render_auth_tab(self, window, cx),
                        RequestTabKind::PreRequestScript => scripts::render_script_tab(
                            self,
                            &self.composer_inputs.pre_script_editor,
                            ScriptEditorKind::PreRequest,
                            cx,
                        ),
                        RequestTabKind::Tests => scripts::render_script_tab(
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
