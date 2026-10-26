use crate::components::status_badge::MethodBadge;
use crate::state::{
    EnvironmentState, GitActiveModal, GitState, GitTab, ModalState, PersistenceState,
    RequestSessionState, SettingsTab, SidebarState, WorkspaceDataState,
};
use crate::theme::theme;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::Icon;
use gpui_kit::assets::IconName;

#[allow(clippy::type_complexity)]
#[derive(IntoElement)]
pub struct CommandPalette {
    workspace_state: Entity<WorkspaceDataState>,
    env_state: Entity<EnvironmentState>,
    persistence_state: Entity<PersistenceState>,
    modal_state: Entity<ModalState>,
    sidebar_state: Entity<SidebarState>,
    git_state: Entity<GitState>,
    editor_state: Entity<RequestSessionState>,
    on_send: std::rc::Rc<dyn Fn(&mut Window, &mut App)>,
}

impl CommandPalette {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        workspace_state: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        persistence_state: Entity<PersistenceState>,
        modal_state: Entity<ModalState>,
        sidebar_state: Entity<SidebarState>,
        git_state: Entity<GitState>,
        editor_state: Entity<RequestSessionState>,
        on_send: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            workspace_state,
            env_state,
            persistence_state,
            modal_state,
            sidebar_state,
            git_state,
            editor_state,
            on_send: std::rc::Rc::new(on_send),
        }
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();

        let env_clone = self.env_state.clone();
        let modal_clone = self.modal_state.clone();
        let sb_clone = self.sidebar_state.clone();
        let git_clone = self.git_state.clone();
        let ed_clone = self.editor_state.clone();
        let on_send = self.on_send.clone();

        let cmd_holder = window.use_keyed_state(
            gpui::SharedString::from("cmd-palette-search-holder"),
            cx,
            |window, cx| {
                let inp = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Type a command, request, or environment...")
                });
                let sub = cx.subscribe_in(&inp, window, move |_, _, event, _, cx| {
                    if let InputEvent::Change = event {
                        cx.refresh_windows();
                    }
                });
                (inp, sub)
            },
        );

        let search_input = cmd_holder.read(cx).0.clone();
        let q = search_input.read(cx).value().to_lowercase();

        let (matched_requests, envs) = {
            let ws = self.workspace_state.read(cx);
            let per = self.persistence_state.read(cx);
            let env = self.env_state.read(cx);
            let recent_len = per.local_state.recent.len();
            let recent_index: std::collections::HashMap<&str, usize> = per
                .local_state
                .recent
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();

            let mut filtered: Vec<(usize, String, String, zest_core::models::HttpMethod)> = ws
                .requests
                .iter()
                .filter(|r| {
                    q.is_empty()
                        || r.name.to_lowercase().contains(&q)
                        || r.url.to_lowercase().contains(&q)
                })
                .map(|r| {
                    let rank = recent_index
                        .get(r.id.as_str())
                        .copied()
                        .unwrap_or(recent_len + 100);
                    (rank, r.id.clone(), r.name.clone(), r.method)
                })
                .collect();
            filtered.sort_by_key(|(rank, _, _, _)| *rank);
            filtered.truncate(50);

            (filtered, env.environments.clone())
        };

        div()
            .id("cmd-palette-overlay")
            .absolute()
            .inset_0()
            .bg(gpui::Hsla { a: 0.6, ..t.bg_app })
            .flex()
            .items_start()
            .justify_center()
            .pt(px(100.))
            .on_click({
                let modal_s = modal_clone.clone();
                move |_, _, cx| {
                    modal_s.update(cx, |state, cx| {
                        state.close_command_palette(cx);
                    });
                }
            })
            .on_key_down({
                let modal_s = modal_clone.clone();
                move |event, _, cx| {
                    if event.keystroke.key.as_str() == "escape" {
                        modal_s.update(cx, |state, cx| {
                            state.close_command_palette(cx);
                        });
                    }
                }
            })
            .child(
                div()
                    .id("cmd-palette-dialog")
                    .occlude()
                    .w(px(560.))
                    .max_h(px(400.))
                    .rounded(px(8.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .on_key_down({
                        let modal_s = modal_clone.clone();
                        move |event, _, cx| {
                            if event.keystroke.key.as_str() == "escape" {
                                modal_s.update(cx, |state, cx| {
                                    state.close_command_palette(cx);
                                });
                            }
                        }
                    })
                    // Search bar
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(t.border)
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(Icon::new(IconName::Search).size(px(14.)))
                            .child(
                                div()
                                    .flex_1()
                                    .on_key_down({
                                        let modal_s = modal_clone.clone();
                                        move |event, _, cx| {
                                            if event.keystroke.key.as_str() == "escape" {
                                                modal_s.update(cx, |state, cx| {
                                                    state.close_command_palette(cx);
                                                });
                                            }
                                        }
                                    })
                                    .child(
                                        Input::new(&search_input)
                                            .id("cmd-palette-input")
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .cleanable(true),
                                    ),
                            )
                            .child({
                                let modal_s = modal_clone.clone();
                                div()
                                    .id("close-cmd-pal")
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .hover(|s| s.text_color(t.text_primary))
                                    .on_click(move |_, _, cx| {
                                        modal_s.update(cx, |state, cx| {
                                            state.close_command_palette(cx);
                                        });
                                    })
                                    .child("ESC")
                            }),
                    )
                    // Actions & Items List
                    .child(
                        div()
                            .flex_1()
                            .overflow_y_scrollbar()
                            .p(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            // Action: Send active request
                            .child({
                                let modal_s = modal_clone.clone();
                                let on_send_cb = on_send.clone();
                                div()
                                    .id("cmd-send-req")
                                    .px(px(8.))
                                    .py(px(6.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(t.bg_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .on_click(move |_, window, cx| {
                                        modal_s.update(cx, |state, cx| {
                                            state.close_command_palette(cx);
                                        });
                                        on_send_cb(window, cx);
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(Icon::new(IconName::Send).size(px(14.)))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(t.text_primary)
                                                    .child("Send Active Request"),
                                            ),
                                    )
                                    .child(div().text_xs().text_color(t.text_muted).child("Ctrl+Enter"))
                            })
                            // Action: Open Settings
                            .when(q.is_empty() || "settings".contains(&q) || "preferences".contains(&q), |this| {
                                let modal_s = modal_clone.clone();
                                this.child(
                                    div()
                                        .id("cmd-open-settings")
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |state, cx| {
                                                state.close_command_palette(cx);
                                                state.open_settings_modal(cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(Icon::new(IconName::Settings).size(px(14.)))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(t.text_primary)
                                                        .child("Open Settings"),
                                                ),
                                        )
                                        .child(div().text_xs().text_color(t.text_muted).child("Ctrl+,"))
                                )
                            })
                            // Action: Manage Cookies (Settings -> Cookies tab)
                            .when(q.is_empty() || "cookies".contains(&q) || "cookie jar".contains(&q) || "session".contains(&q), |this| {
                                let modal_s = modal_clone.clone();
                                this.child(
                                    div()
                                        .id("cmd-open-cookies")
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |state, cx| {
                                                state.close_command_palette(cx);
                                                state.open_settings_tab(SettingsTab::Cookies, cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(Icon::new(IconName::Cookie).size(px(14.)))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(t.text_primary)
                                                        .child("Settings: Manage Cookies"),
                                                ),
                                        )
                                        .child(div().text_xs().text_color(t.text_muted).child("Settings"))
                                )
                            })
                            // Action: Git Source Control
                            .when(q.is_empty() || "git".contains(&q) || "source control".contains(&q) || "commit".contains(&q) || "diff".contains(&q), |this| {
                                let modal_s = modal_clone.clone();
                                let git_s = git_clone.clone();
                                this.child(
                                    div()
                                        .id("cmd-open-git")
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |state, cx| {
                                                state.close_command_palette(cx);
                                            });
                                            git_s.update(cx, |state, cx| {
                                                state.set_tab(GitTab::Changes, cx);
                                                state.open_modal(GitActiveModal::Commit, cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(Icon::new(IconName::GitBranch).size(px(14.)))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(t.text_primary)
                                                        .child("Git: Open Source Control (Commit & Diff)"),
                                                ),
                                        )
                                        .child(div().text_xs().text_color(t.text_muted).child("Ctrl+Shift+G"))
                                )
                            })
                            // Action: Git Switch Branch / Checkout
                            .when(q.is_empty() || "git".contains(&q) || "branch".contains(&q) || "checkout".contains(&q), |this| {
                                let modal_s = modal_clone.clone();
                                let git_s = git_clone.clone();
                                this.child(
                                    div()
                                        .id("cmd-git-checkout")
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |state, cx| {
                                                state.close_command_palette(cx);
                                            });
                                            git_s.update(cx, |state, cx| {
                                                state.set_tab(GitTab::Branches, cx);
                                                state.open_modal(GitActiveModal::NewBranch, cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(Icon::new(IconName::GitBranch).size(px(14.)))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(t.text_primary)
                                                        .child("Git: Switch Branch / Checkout"),
                                                ),
                                        )
                                        .child(div().text_xs().text_color(t.text_muted).child("Git"))
                                )
                            })
                            // Action: Import OpenAPI or Postman
                            .when(q.is_empty() || "import".contains(&q) || "openapi".contains(&q) || "postman".contains(&q) || "swagger".contains(&q), |this| {
                                let modal_s = modal_clone.clone();
                                this.child(
                                    div()
                                        .id("cmd-import-spec")
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |state, cx| {
                                                state.close_command_palette(cx);
                                                state.open_import_modal(None, cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(Icon::new(IconName::Download).size(px(14.)))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(t.text_primary)
                                                        .child("Import: OpenAPI (YAML/JSON) or Postman"),
                                                ),
                                        )
                                        .child(div().text_xs().text_color(t.text_muted).child("Ctrl+I"))
                                )
                            })
                            // Action: Git Commit History
                            .when(q.is_empty() || "git".contains(&q) || "history".contains(&q) || "log".contains(&q), |this| {
                                let modal_s = modal_clone.clone();
                                let git_s = git_clone.clone();
                                this.child(
                                    div()
                                        .id("cmd-git-history")
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |state, cx| {
                                                state.close_command_palette(cx);
                                            });
                                            git_s.update(cx, |state, cx| {
                                                state.set_tab(GitTab::History, cx);
                                                state.open_modal(GitActiveModal::History, cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child("📜")
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(t.text_primary)
                                                        .child("Git: View Commit History"),
                                                ),
                                        )
                                        .child(div().text_xs().text_color(t.text_muted).child("Git"))
                                )
                            })
                            // List Requests
                            .child(
                                div()
                                    .px(px(8.))
                                    .pt(px(4.))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(t.text_muted)
                                    .child("SWITCH REQUEST"),
                            )
                            .children(matched_requests.into_iter().map(
                                |(_, req_id, req_name, req_method)| {
                                    let modal_s = modal_clone.clone();
                                    let sb_s = sb_clone.clone();
                                    let ed_s = ed_clone.clone();
                                    let r_id = req_id.clone();

                                    div()
                                        .id(format!("cmd-req-{}", req_id))
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .on_click(move |_, _, cx| {
                                            sb_s.update(cx, |state, _cx| {
                                                state.record_navigation(&r_id);
                                            });
                                            modal_s.update(cx, |state, cx| {
                                                state.close_command_palette(cx);
                                            });
                                            ed_s.update(cx, |state, cx| {
                                                state.set_active_request_id(&r_id, cx);
                                            });
                                        })
                                        .child(MethodBadge::new(req_method))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_family(".SystemMonoFont")
                                                .text_color(t.text_primary)
                                                .child(req_name),
                                        )
                                },
                            ))
                            // Switch Environment
                            .child(
                                div()
                                    .px(px(8.))
                                    .pt(px(6.))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(t.text_muted)
                                    .child("SWITCH ENVIRONMENT"),
                            )
                            .children(
                                envs.into_iter()
                                    .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
                                    .map(|env| {
                                        let env_s = env_clone.clone();
                                        let modal_s = modal_clone.clone();
                                        let env_id = env.id.clone();

                                        div()
                                            .id(format!("cmd-env-{}", env.id))
                                            .px(px(8.))
                                            .py(px(6.))
                                            .rounded(px(4.))
                                            .hover(|s| s.bg(t.bg_hover))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .on_click(move |_, _, cx| {
                                                env_s.update(cx, |state, cx| {
                                                    state.set_active_environment(
                                                        Some(env_id.clone()),
                                                        cx,
                                                    );
                                                });
                                                modal_s.update(cx, |state, cx| {
                                                    state.close_command_palette(cx);
                                                });
                                            })
                                            .child("●")
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(t.text_primary)
                                                    .child(format!("Switch to: {}", env.name)),
                                             )
                                    }),
                            ),
                    ),
            )
    }
}
