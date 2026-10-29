use super::common::{render_error_banner, render_not_a_repo_view};
use crate::services::GitService;
use crate::state::GitState;
use crate::theme::theme;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};

#[derive(IntoElement)]
pub struct GitBranchModal {
    git_state: Entity<GitState>,
    git_service: GitService,
}

impl GitBranchModal {
    pub fn new(git_state: Entity<GitState>, git_service: GitService) -> Self {
        Self {
            git_state,
            git_service,
        }
    }
}

pub fn render_new_branch_view(
    git_clone: Entity<GitState>,
    git_service: GitService,
    current_branch: String,
    branch_input: Entity<InputState>,
    is_loading: bool,
) -> gpui_kit::AnyElement {
    let t = theme();
    let git_cancel = git_clone.clone();
    let svc_create = git_service;
    let inp = branch_input.clone();
    let inp_val = branch_input.clone();

    div()
        .p(px(20.))
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(t.text_primary)
                        .child("Branch Name"),
                )
                .child(
                    div()
                        .bg(t.bg_input)
                        .border_1()
                        .border_color(t.border)
                        .rounded(px(6.))
                        .p(px(6.))
                        .child(Input::new(&branch_input)),
                )
                .child(div().text_xs().text_color(t.text_muted).child(format!(
                    "Branch will be created based on \"{}\"",
                    current_branch
                ))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(8.))
                .child(
                    div()
                        .id("btn-cancel-create-branch")
                        .px(px(14.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(t.border)
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(t.text_primary)
                        .hover(|s| s.bg(t.bg_hover))
                        .on_click(move |_, _, cx| {
                            git_cancel.update(cx, |git, cx| {
                                git.close_modal(cx);
                            });
                        })
                        .child("Cancel"),
                )
                .child(
                    div()
                        .id("btn-confirm-create-branch")
                        .px(px(14.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .when(!is_loading, |s| {
                            s.bg(t.primary)
                                .cursor_pointer()
                                .text_color(t.primary_foreground)
                                .hover(|s| s.opacity(0.9))
                        })
                        .when(is_loading, |s| s.bg(t.bg_hover).text_color(t.text_muted))
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .on_click(move |_, window, cx| {
                            if is_loading {
                                return;
                            }
                            let name = inp_val.read(cx).value().trim().to_string();
                            if name.is_empty() {
                                return;
                            }
                            inp.update(cx, |inp_st, cx| {
                                inp_st.set_value("", window, cx);
                            });
                            svc_create.create_branch(&name, cx);
                        })
                        .when(is_loading, |s| s.child("Creating..."))
                        .when(!is_loading, |s| s.child("Create Branch")),
                ),
        )
        .into_any_element()
}

impl RenderOnce for GitBranchModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let git_clone = self.git_state.clone();
        let svc_clone = self.git_service.clone();

        let (is_repo, current_branch, git_error, is_loading) = {
            let git = self.git_state.read(cx);
            (
                git.status.as_ref().map(|s| s.is_repo).unwrap_or(false),
                git.branch.clone().unwrap_or_else(|| "main".to_string()),
                git.error.clone(),
                git.loading,
            )
        };

        let branch_input_holder = window.use_keyed_state(
            gpui_kit::SharedString::from("git-new-branch-input"),
            cx,
            |window, cx| {
                let inp = cx.new(|cx| {
                    InputState::new(window, cx).placeholder("New branch name (e.g. feature/auth)")
                });
                (inp, ())
            },
        );
        let branch_input = branch_input_holder.read(cx).0.clone();

        div()
            .id("git-branch-modal-overlay")
            .absolute()
            .inset_0()
            .bg(gpui_kit::Hsla {
                a: 0.65,
                ..t.bg_app
            })
            .flex()
            .items_center()
            .justify_center()
            .on_click({
                let git_s = git_clone.clone();
                move |_, _, cx| {
                    git_s.update(cx, |state, cx| {
                        state.close_modal(cx);
                    });
                }
            })
            .on_key_down({
                let git_s = git_clone.clone();
                move |event, _, cx| {
                    if event.keystroke.key.as_str() == "escape" {
                        git_s.update(cx, |state, cx| {
                            state.close_modal(cx);
                        });
                    }
                }
            })
            .child(
                div()
                    .id("git-branch-modal-dialog")
                    .occlude()
                    .w(px(420.))
                    .rounded(px(8.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_key_down({
                        let git_s = git_clone.clone();
                        move |event, _, cx| {
                            if event.keystroke.key.as_str() == "escape" {
                                git_s.update(cx, |state, cx| {
                                    state.close_modal(cx);
                                });
                            }
                        }
                    })
                    // Header
                    .child(
                        div()
                            .h(px(44.))
                            .px(px(16.))
                            .border_b_1()
                            .border_color(t.border)
                            .bg(t.bg_titlebar)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().text_base().text_color(t.primary).child("🌱"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .child("Create New Branch"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("btn-git-branch-close")
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_sm()
                                    .text_color(t.text_muted)
                                    .hover(|s| s.text_color(t.text_primary))
                                    .on_click({
                                        let git_s = git_clone.clone();
                                        move |_, _, cx| {
                                            git_s.update(cx, |state, cx| {
                                                state.close_modal(cx);
                                            });
                                        }
                                    })
                                    .child("✕"),
                            ),
                    )
                    // Error banner
                    .when_some(git_error, |this, err| {
                        this.child(render_error_banner(git_clone.clone(), err))
                    })
                    // Body
                    .child(if !is_repo {
                        render_not_a_repo_view(svc_clone.clone(), is_loading)
                    } else {
                        render_new_branch_view(
                            git_clone.clone(),
                            svc_clone.clone(),
                            current_branch,
                            branch_input,
                            is_loading,
                        )
                    }),
            )
            .into_any_element()
    }
}
