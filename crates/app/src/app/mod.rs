pub mod actions;
pub mod inspector_displays;
pub mod modals;
pub mod subscriptions;
pub mod workspace_event_handler;

pub use crate::components::request_composer::composer_inputs::*;
pub use actions::*;
pub use inspector_displays::*;
pub use modals::*;
pub use subscriptions::*;

use crate::components::*;
use crate::modals::*;
use crate::services::{HttpEngine, Runner};
use crate::state::*;
use crate::theme::theme;
use gpui_kit::component::input::ShowCompletions;
use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::*;
use std::sync::Arc;

pub struct AppView {
    pub(crate) state: AppState,
    pub(crate) composer_inputs: ComposerInputs,
    pub(crate) inspector_displays: InspectorDisplays,
    pub(crate) env_var_table: Entity<EnvVarTable>,
    pub(crate) runner: Runner,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) _subscriptions: Vec<Subscription>,
    /// Startup restore guard: the workspace loads asynchronously, so the
    /// initial request selection is replayed once (in the `RequestsReloaded`
    /// handler) when the first scan lands.
    pub(crate) initial_restore_done: bool,
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        let state = AppState::new(cx);

        type HighlighterFactory = std::rc::Rc<
            dyn Fn(&str) -> Option<Box<dyn gpui_kit::component::input::InputHighlighter>>,
        >;

        let highlighter_factory: HighlighterFactory = std::rc::Rc::new(|lang: &str| match lang {
            "json" => Some(Box::new(
                crate::components::body_editor::JsonHighlighter::new(),
            )),
            "xml" => Some(Box::new(
                crate::components::body_editor::XmlHighlighter::new(),
            )),
            "javascript" | "js" => Some(Box::new(JavaScriptHighlighter::new())),
            _ => None,
        });

        let composer_inputs = ComposerInputs::new(
            window,
            state.env.clone(),
            state.modal.clone(),
            state.session.clone(),
            state.collection.clone(),
            highlighter_factory.clone(),
            cx,
        );
        let inspector_displays = InspectorDisplays::new(window, highlighter_factory, cx);
        let env_var_table = {
            let env_ent = state.env.clone();
            let modal_ent = state.modal.clone();
            cx.new(|cx| EnvVarTable::new(window, env_ent, modal_ent, cx))
        };

        let executor = state.workspace.read(cx).executor.clone();
        let tokio_rt = crate::services::global_tokio_runtime();
        let http_engine = Arc::new(HttpEngine::new(executor, tokio_rt));
        let ws_service = Arc::new(crate::services::WsService::new());
        let grpc_engine = Arc::new(crate::services::GrpcEngine::new());
        let runner = Runner::new(
            state.session.clone(),
            state.workspace.clone(),
            state.collection.clone(),
            state.env.clone(),
            state.response.clone(),
            state.modal.clone(),
            crate::services::Engines {
                http: http_engine,
                ws: ws_service,
                grpc: grpc_engine,
            },
        );

        // Set up subscriptions:
        let mut _subscriptions = setup_app_subscriptions(
            SubscriptionTargets {
                workspace_state: &state.workspace,
                collection_state: &state.collection,
                env_state: &state.env,
                persistence_state: &state.persistence,
                modal_state: &state.modal,
                sidebar_state: &state.sidebar,
                git_state: &state.git,
                editor_state: &state.session,
                response_state: &state.response,
                composer_inputs: &composer_inputs,
            },
            window,
            cx,
        );

        let focus_lost_sub = cx.on_focus_lost(window, |this, window, cx| {
            window.focus(&this.focus_handle, cx);
        });
        _subscriptions.push(focus_lost_sub);

        if state.git.read(cx).tracked_warning {
            crate::notify::error_in(
                "Security Warning",
                "A git repository was detected with .local/ or *.loc tracked in git. Secrets or response tokens may leak!",
                window,
                cx,
            );
        }

        Self {
            state,
            composer_inputs,
            inspector_displays,
            env_var_table,
            runner,
            focus_handle,
            _subscriptions,
            initial_restore_done: false,
        }
    }

    /// Primary send action; the Runner routes by transport protocol (HTTP
    /// sends, WebSocket connects, gRPC invokes). Keeps Ctrl+Enter and the
    /// URL-bar Enter consistent with the RequestBar button.
    pub fn send_current_request(&self, cx: &mut App) {
        self.runner.send_current_request(cx);
    }

    pub fn disconnect_websocket(&self, cx: &mut App) {
        self.runner.disconnect_websocket(cx);
    }

    /// Sends the text currently in the WS message box over the active
    /// connection and clears the box on success. When the request's message
    /// format is JSON, the content must parse as JSON before it goes out.
    pub fn send_ws_message(&self, window: &mut Window, cx: &mut App) {
        let text = self
            .composer_inputs
            .ws_message_textarea
            .read(cx)
            .value()
            .to_string();
        if text.trim().is_empty() {
            return;
        }
        let req_id = self.state.session.read(cx).active_request_id().to_string();
        let wants_json = self
            .state
            .collection
            .read(cx)
            .find_request(&req_id)
            .map(|r| r.ws_message_format.is_json())
            .unwrap_or(false);
        if wants_json {
            if let Err(e) = serde_json::from_str::<serde_json::Value>(&text) {
                crate::notify::error_in("Invalid JSON", e.to_string(), window, cx);
                return;
            }
        }
        self.runner.send_ws_message(text, cx);
        if self.runner.engines.ws.is_connected(&req_id) {
            self.composer_inputs
                .ws_message_textarea
                .update(cx, |txt, cx| {
                    txt.set_value(String::new(), window, cx);
                });
        }
    }

    pub fn send_ws_ping(&self, cx: &mut App) {
        self.runner.send_ws_ping(cx);
    }

    pub fn fetch_grpc_schema(&self, cx: &mut App) {
        self.runner.fetch_grpc_schema(cx);
    }

    pub fn fetch_graphql_schema(&self, cx: &mut App) {
        self.runner.fetch_graphql_schema(cx);
    }

    /// Revalidates the gRPC message editor content and refreshes the error
    /// diagnostics + gutter mark. Called when the active request changes so
    /// the marks follow the editor's restored content.
    pub fn refresh_grpc_message_diagnostics(&self, cx: &mut App) {
        let text = self
            .composer_inputs
            .grpc
            .message_editor
            .read(cx)
            .value()
            .to_string();
        crate::components::request_composer::refresh_grpc_validation(
            &self.composer_inputs.grpc.message_editor,
            &self.composer_inputs.grpc.error_marks,
            self.composer_inputs.grpc.diagnostics_hover.get(),
            &text,
            &self.state.session,
            &self.state.collection,
            cx,
        );
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();

        let sidebar_collapsed = self.state.sidebar.read(cx).sidebar_collapsed;
        let sidebar_width = self.state.sidebar.read(cx).sidebar_width;
        let is_vertical_split = self.state.modal.read(cx).split_layout_vertical;

        let ws_entity = self.state.workspace.clone();
        let col_entity = self.state.collection.clone();
        let sidebar_entity = self.state.sidebar.clone();
        let modal_entity = self.state.modal.clone();
        let git_entity = self.state.git.clone();
        let session_entity = self.state.session.clone();
        let resp_entity = self.state.response.clone();
        let app_view_entity = cx.entity().clone();

        let composer_pane = self.render_request_composer(window, cx);

        let response_pane = ResponseInspector::new(
            col_entity.clone(),
            session_entity.clone(),
            resp_entity.clone(),
            self.inspector_displays.response_editor.clone(),
            self.inspector_displays.request_body_editor.clone(),
        );

        let split_pane = if is_vertical_split {
            div().size_full().child(
                v_resizable("inner-vertical-split")
                    .child(resizable_panel().child(composer_pane))
                    .child(resizable_panel().child(response_pane)),
            )
        } else {
            div().size_full().child(
                h_resizable("inner-horizontal-split")
                    .child(resizable_panel().child(composer_pane))
                    .child(resizable_panel().child(response_pane)),
            )
        };

        let main_content = div().size_full().child(split_pane);

        let workspace_content = if sidebar_collapsed {
            main_content.into_any_element()
        } else {
            h_resizable("workspace-sidebar-split")
                .child(
                    resizable_panel()
                        .size(sidebar_width)
                        .size_range(px(200.)..px(480.))
                        .child(SidebarTree {
                            workspace_state: col_entity.clone(),
                            persistence_state: self.state.persistence.clone(),
                            sidebar_state: sidebar_entity.clone(),
                            modal_state: modal_entity.clone(),
                            git_state: git_entity.clone(),
                            editor_state: session_entity.clone(),
                            response_state: resp_entity.clone(),
                        }),
                )
                .child(resizable_panel().child(main_content))
                .on_resize({
                    let sb_entity = sidebar_entity.clone();
                    move |state, _window, cx| {
                        let new_size = state.read(cx).sizes().first().copied();
                        if let Some(size) = new_size {
                            sb_entity.update(cx, |sb, cx| {
                                sb.set_sidebar_width(size, cx);
                            });
                        }
                    }
                })
                .into_any_element()
        };

        let root = div()
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(t.bg_app)
            .text_color(t.text_primary)
            .flex()
            .flex_col()
            // Fallback for the completion popup: fires only when the
            // Ctrl+Space keybinding was not consumed by binding dispatch
            // (e.g. a key-context quirk), so there is no double dispatch.
            .on_key_down(move |event: &gpui_kit::KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                if k.modifiers.control && !k.modifiers.alt && !k.modifiers.shift && k.key == "space"
                {
                    window.dispatch_action(Box::new(ShowCompletions), cx);
                }
            })
            // Keyboard shortcut action handlers
            .on_action({
                let view_handle = app_view_entity.clone();
                move |_: &SendRequest, _, cx| {
                    view_handle.update(cx, |view, cx| {
                        view.send_current_request(cx);
                    });
                }
            })
            .on_action({
                let modal_c = modal_entity.clone();
                move |_: &ToggleCommandPalette, _, cx| {
                    modal_c.update(cx, |state, cx| {
                        state.toggle(ModalKind::CommandPalette, cx);
                    });
                }
            })
            .on_action({
                let sb_c = sidebar_entity.clone();
                move |_: &ToggleSidebar, _, cx| {
                    sb_c.update(cx, |state, cx| {
                        state.toggle_sidebar(cx);
                    });
                }
            })
            .on_action({
                let modal_c = modal_entity.clone();
                move |_: &ToggleSettings, _, cx| {
                    modal_c.update(cx, |state, cx| {
                        state.toggle(ModalKind::Settings, cx);
                    });
                }
            })
            .on_action({
                let git_c = git_entity.clone();
                move |_: &ToggleGitModal, _, cx| {
                    git_c.update(cx, |state, cx| {
                        state.toggle_modal(cx);
                    });
                }
            })
            .on_action({
                let modal_c = modal_entity.clone();
                move |_: &ToggleAboutModal, _, cx| {
                    modal_c.update(cx, |state, cx| {
                        state.toggle(ModalKind::About, cx);
                    });
                }
            })
            .on_action({
                let modal_c = modal_entity.clone();
                move |_: &OpenImportModal, _, cx| {
                    modal_c.update(cx, |state, cx| {
                        state.open(
                            ModalKind::Import,
                            Some(ModalPayload::Import {
                                target_folder_rel: None,
                            }),
                            cx,
                        );
                    });
                }
            })
            .on_action({
                let sb_c = sidebar_entity.clone();
                let session_c = session_entity.clone();
                move |_: &NavigateBack, _, cx| {
                    if let Some(req_id) = sb_c.update(cx, |sb, _| sb.navigate_back()) {
                        session_c.update(cx, |ed, cx| {
                            ed.set_active_request_id(&req_id, cx);
                        });
                    }
                }
            })
            .on_action({
                let sb_c = sidebar_entity.clone();
                let session_c = session_entity.clone();
                move |_: &NavigateForward, _, cx| {
                    if let Some(req_id) = sb_c.update(cx, |sb, _| sb.navigate_forward()) {
                        session_c.update(cx, |ed, cx| {
                            ed.set_active_request_id(&req_id, cx);
                        });
                    }
                }
            })
            .on_action({
                let modal_c = modal_entity.clone();
                let git_c = git_entity.clone();
                move |_: &DismissModal, _, cx| {
                    let git_open = git_c.read(cx).modal.is_open;
                    if git_open {
                        git_c.update(cx, |g, cx| g.close_modal(cx));
                    } else {
                        modal_c.update(cx, |m, cx| {
                            m.dismiss_top_modal(cx);
                        });
                    }
                }
            })
            .on_key_down({
                let modal_c = modal_entity.clone();
                let git_c = git_entity.clone();
                let view_handle = app_view_entity.clone();
                move |event, _, cx| {
                    if event.keystroke.key.as_str() == "escape" {
                        let git_open = git_c.read(cx).modal.is_open;
                        if git_open {
                            git_c.update(cx, |g, cx| g.close_modal(cx));
                        } else {
                            modal_c.update(cx, |m, cx| {
                                m.dismiss_top_modal(cx);
                            });
                        }
                        cx.stop_propagation();
                    } else if (event.keystroke.modifiers.control
                        || event.keystroke.modifiers.platform)
                        && event.keystroke.key.eq_ignore_ascii_case("k")
                        && !event.keystroke.modifiers.alt
                    {
                        let git_open = git_c.read(cx).modal.is_open;
                        if !git_open {
                            modal_c.update(cx, |m, cx| {
                                m.toggle(ModalKind::CommandPalette, cx);
                            });
                            cx.stop_propagation();
                        }
                    } else if (event.keystroke.modifiers.control
                        || event.keystroke.modifiers.platform)
                        && (event.keystroke.key.as_str() == "enter"
                            || event.keystroke.key.as_str() == "return")
                        && !event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.shift
                    {
                        let git_open = git_c.read(cx).modal.is_open;
                        let any_modal_open = modal_c.read(cx).has_any_modal_open();
                        if !git_open && !any_modal_open {
                            view_handle.update(cx, |view, cx| {
                                view.send_current_request(cx);
                            });
                            cx.stop_propagation();
                        }
                    }
                }
            })
            // Custom Title Bar
            .child(AppTitleBar::new(
                ws_entity.clone(),
                col_entity.clone(),
                self.state.env.clone(),
                modal_entity.clone(),
                sidebar_entity.clone(),
                session_entity.clone(),
                resp_entity.clone(),
            ))
            // Main workspace content area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(workspace_content),
            );

        attach_modal_overlays(
            root,
            ModalOverlayEntities {
                workspace_state: &ws_entity,
                collection_state: &col_entity,
                env_state: &self.state.env,
                persistence_state: &self.state.persistence,
                modal_state: &modal_entity,
                sidebar_state: &sidebar_entity,
                git_state: &git_entity,
                editor_state: &session_entity,
                env_var_table: &self.env_var_table,
            },
            cx,
        )
    }
}
