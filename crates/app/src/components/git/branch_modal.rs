use super::common::{render_error_banner, render_not_a_repo_view};
use crate::components::ModalDialog;
use crate::services::GitService;
use crate::state::GitState;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Sizable as _;
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
                    Button::new("btn-cancel-create-branch")
                        .ghost()
                        .small()
                        .label("Cancel")
                        .on_click(move |_, _, cx| {
                            git_cancel.update(cx, |git, cx| {
                                git.close_modal(cx);
                            });
                        }),
                )
                .child(
                    Button::new("btn-confirm-create-branch")
                        .primary()
                        .small()
                        .disabled(is_loading)
                        .loading(is_loading)
                        .label(if is_loading {
                            "Creating..."
                        } else {
                            "Create Branch"
                        })
                        .on_click(move |_, window, cx: &mut App| {
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
                        }),
                ),
        )
        .into_any_element()
}

impl RenderOnce for GitBranchModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let _t = theme();
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

        ModalDialog::new("git-branch-modal")
            .w(px(420.))
            .title("Create New Branch")
            .icon(IconName::GitBranch)
            .on_close({
                let git_s = git_clone.clone();
                move |_, cx| {
                    git_s.update(cx, |state, cx| {
                        state.close_modal(cx);
                    });
                }
            })
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
            })
            .into_any_element()
    }
}
