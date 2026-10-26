pub mod actions;
pub mod auth_tab;
pub mod composer_inputs;
pub mod inspector_displays;
pub mod modals;
pub mod request_composer;
pub mod subscriptions;
pub mod workspace_event_handler;

pub use actions::*;
pub use composer_inputs::*;
pub use inspector_displays::*;
pub use modals::*;
pub use subscriptions::*;

use crate::components::*;
use crate::services::{HttpEngine, Runner};
use crate::state::*;
use crate::theme::theme;
use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};
use std::sync::Arc;

pub struct AppView {
    pub(crate) state: AppState,
    pub(crate) composer_inputs: ComposerInputs,
    pub(crate) inspector_displays: InspectorDisplays,
    pub(crate) runner: Runner,
    pub(crate) _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = AppState::new(cx);

        type HighlighterFactory =
            std::rc::Rc<dyn Fn(&str) -> Option<Box<dyn gpui_component::input::InputHighlighter>>>;

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

        let mut composer_inputs =
            ComposerInputs::new(window, state.env.clone(), highlighter_factory.clone(), cx);
        let mut inspector_displays = InspectorDisplays::new(window, highlighter_factory, cx);

        let executor = state.workspace.read(cx).executor.clone();
        let tokio_rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime"),
        );
        let engine = Arc::new(HttpEngine::new(executor, tokio_rt));
        let runner = Runner::new(
            state.session.clone(),
            state.workspace.clone(),
            state.env.clone(),
            state.response.clone(),
            state.toast.clone(),
            state.modal.clone(),
            engine,
        );

        // Select initial request from workspace if the user has saved requests
        let (initial_req, ws_store) = {
            let ws_read = state.workspace.read(cx);
            let per_read = state.persistence.read(cx);
            let req = per_read
                .local_state
                .active_request
                .as_ref()
                .and_then(|act_id| ws_read.requests.iter().find(|r| &r.id == act_id).cloned())
                .or_else(|| ws_read.requests.first().cloned());
            (req, ws_read.store.clone())
        };
        if let Some(req) = &initial_req {
            state.session.update(cx, |ed, cx| {
                ed.set_active_request_id(&req.id, cx);
            });
            composer_inputs.sync_inputs(
                &req.id,
                ComposerSyncScope::All,
                &state.workspace,
                &state.env,
                window,
                cx,
            );
            let req_id = req.id.clone();
            state.response.update(cx, |resp, cx| {
                resp.load_for_request(&req_id, ws_store.as_ref(), cx);
            });
            inspector_displays.sync_response_editor(&state.response, window, cx);
            inspector_displays.sync_sent_request_preview(&state.response, window, cx);
        }

        // Set up subscriptions:
        let _subscriptions = setup_app_subscriptions(
            SubscriptionTargets {
                workspace_state: &state.workspace,
                env_state: &state.env,
                persistence_state: &state.persistence,
                modal_state: &state.modal,
                sidebar_state: &state.sidebar,
                git_state: &state.git,
                editor_state: &state.session,
                response_state: &state.response,
                toast_state: &state.toast,
                composer_inputs: &composer_inputs,
            },
            window,
            cx,
        );

        if state.git.read(cx).tracked_warning {
            state.toast.update(cx, |t, cx| {
                t.push_error(
                    "Security Warning",
                    "A git repository was detected with .local/ or *.loc tracked in git. Secrets or response tokens may leak!",
                    cx,
                );
            });
        }

        Self {
            state,
            composer_inputs,
            inspector_displays,
            runner,
            _subscriptions,
        }
    }

    pub fn send_current_request(&self, cx: &mut App) {
        self.runner.send_current_request(cx);
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();

        let sidebar_collapsed = self.state.sidebar.read(cx).sidebar_collapsed;
        let sidebar_width = self.state.sidebar.read(cx).sidebar_width;
        let is_vertical_split = self.state.modal.read(cx).split_layout_vertical;

        let ws_entity = self.state.workspace.clone();
        let sidebar_entity = self.state.sidebar.clone();
        let modal_entity = self.state.modal.clone();
        let git_entity = self.state.git.clone();
        let session_entity = self.state.session.clone();
        let resp_entity = self.state.response.clone();
        let toast_entity = self.state.toast.clone();
        let app_view_entity = cx.entity().clone();

        let composer_pane = self.render_request_composer(window, cx);

        let response_pane = ResponseInspector::new(
            ws_entity.clone(),
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
                        .child(SidebarTree::new(
                            ws_entity.clone(),
                            self.state.persistence.clone(),
                            sidebar_entity.clone(),
                            modal_entity.clone(),
                            git_entity.clone(),
                            session_entity.clone(),
                            resp_entity.clone(),
                            toast_entity.clone(),
                        )),
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
            .size_full()
            .bg(t.bg_app)
            .text_color(t.text_primary)
            .flex()
            .flex_col()
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
                        state.toggle_command_palette(cx);
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
                        state.toggle_settings_modal(cx);
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
                move |_: &OpenImportModal, _, cx| {
                    modal_c.update(cx, |state, cx| {
                        state.open_import_modal(None, cx);
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
                    let git_open = git_c.read(cx).show_modal;
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
                move |event, _, cx| {
                    if event.keystroke.key.as_str() == "escape" {
                        let git_open = git_c.read(cx).show_modal;
                        if git_open {
                            git_c.update(cx, |g, cx| g.close_modal(cx));
                        } else {
                            modal_c.update(cx, |m, cx| {
                                m.dismiss_top_modal(cx);
                            });
                        }
                    }
                }
            })
            // Custom Title Bar
            .child(AppTitleBar::new(
                ws_entity.clone(),
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

        let send_view = app_view_entity.clone();
        attach_modal_overlays(
            root,
            ModalOverlayEntities {
                workspace_state: &ws_entity,
                env_state: &self.state.env,
                persistence_state: &self.state.persistence,
                modal_state: &modal_entity,
                sidebar_state: &sidebar_entity,
                git_state: &git_entity,
                editor_state: &session_entity,
                toast_state: &toast_entity,
            },
            move |_, cx| {
                send_view.update(cx, |view, cx| {
                    view.send_current_request(cx);
                });
            },
            cx,
        )
    }
}
