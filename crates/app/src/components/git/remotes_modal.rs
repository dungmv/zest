use super::common::{render_error_banner, render_not_a_repo_view};
use crate::services::GitService;
use crate::state::GitState;
use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;

#[derive(IntoElement)]
pub struct GitRemotesModal {
    git_state: Entity<GitState>,
    git_service: GitService,
}

impl GitRemotesModal {
    pub fn new(git_state: Entity<GitState>, git_service: GitService) -> Self {
        Self {
            git_state,
            git_service,
        }
    }
}

pub fn render_remotes_view(
    git_service: GitService,
    remotes: Vec<zest_core::git::GitRemote>,
    remote_name_input: Entity<InputState>,
    remote_url_input: Entity<InputState>,
    is_loading: bool,
) -> gpui_kit::AnyElement {
    let t = theme();

    div()
        .size_full()
        .flex()
        .flex_col()
        // Top/Middle: Remotes List (Scrollable)
        .child(
            div()
                .flex_1()
                .overflow_y_scrollbar()
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .when(remotes.is_empty(), |this| {
                    this.child(
                        div()
                            .py(px(24.))
                            .text_center()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child("No git remotes configured yet."),
                    )
                })
                .children(remotes.into_iter().map(|r| {
                    let svc_del = git_service.clone();
                    let r_name = r.name.clone();

                    div()
                        .id(format!("remote-row-{}", r.name))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .bg(t.bg_sidebar)
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .font_family(".SystemMonoFont")
                                        .text_color(t.primary)
                                        .child(r.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(".SystemMonoFont")
                                        .text_color(t.text_secondary)
                                        .truncate()
                                        .child(r.fetch_url.clone()),
                                ),
                        )
                        .child(
                            div()
                                .id(format!("btn-remove-remote-{}", r.name))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .when(!is_loading, |s| {
                                    s.cursor_pointer()
                                        .text_color(t.text_muted)
                                        .hover(|s| s.bg(t.bg_card).text_color(t.status_5xx))
                                })
                                .when(is_loading, |s| s.text_color(t.text_muted))
                                .text_xs()
                                .on_click(move |_, _, cx| {
                                    if is_loading {
                                        return;
                                    }
                                    svc_del.remove_remote(&r_name, cx);
                                })
                                .child("Remove"),
                        )
                })),
        )
        // Bottom: Add Remote Form
        .child(
            div()
                .border_t_1()
                .border_color(t.border)
                .p(px(12.))
                .bg(t.bg_sidebar)
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(84.))
                        .bg(t.bg_card)
                        .border_1()
                        .border_color(t.border)
                        .rounded(px(6.))
                        .p(px(4.))
                        .child(Input::new(&remote_name_input)),
                )
                .child(
                    div()
                        .flex_1()
                        .bg(t.bg_card)
                        .border_1()
                        .border_color(t.border)
                        .rounded(px(6.))
                        .p(px(4.))
                        .child(Input::new(&remote_url_input)),
                )
                .child({
                    let svc_add = git_service.clone();
                    let inp_n = remote_name_input.clone();
                    let inp_u = remote_url_input.clone();
                    div()
                        .id("btn-add-remote-action")
                        .px(px(12.))
                        .h(px(30.))
                        .rounded(px(6.))
                        .when(!is_loading, |s| {
                            s.bg(t.primary)
                                .cursor_pointer()
                                .text_color(t.primary_foreground)
                                .hover(|s| s.opacity(0.9))
                        })
                        .when(is_loading, |s| s.bg(t.bg_hover).text_color(t.text_muted))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .on_click(move |_, window, cx| {
                            if is_loading {
                                return;
                            }
                            let name = inp_n.read(cx).value().trim().to_string();
                            let url = inp_u.read(cx).value().trim().to_string();
                            if name.is_empty() || url.is_empty() {
                                return;
                            }
                            inp_u.update(cx, |inp_st, cx| {
                                inp_st.set_value("", window, cx);
                            });
                            svc_add.add_remote(&name, &url, cx);
                        })
                        .when(is_loading, |s| s.child("Adding..."))
                        .when(!is_loading, |s| s.child("+ Add"))
                }),
        )
        .into_any_element()
}

impl RenderOnce for GitRemotesModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let git_clone = self.git_state.clone();
        let svc_clone = self.git_service.clone();

        let (is_repo, remotes, git_error, is_loading) = {
            let git = self.git_state.read(cx);
            (
                git.status.as_ref().map(|s| s.is_repo).unwrap_or(false),
                git.remotes.clone(),
                git.error.clone(),
                git.loading,
            )
        };

        let remote_name_holder = window.use_keyed_state(
            gpui_kit::SharedString::from("git-remote-name-input"),
            cx,
            |window, cx| {
                let inp = cx.new(|cx| {
                    let mut s = InputState::new(window, cx);
                    s.set_value("origin", window, cx);
                    s
                });
                (inp, ())
            },
        );
        let remote_name_input = remote_name_holder.read(cx).0.clone();

        let remote_url_holder = window.use_keyed_state(
            gpui_kit::SharedString::from("git-remote-url-input"),
            cx,
            |window, cx| {
                let inp = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("https://github.com/owner/repository.git")
                });
                (inp, ())
            },
        );
        let remote_url_input = remote_url_holder.read(cx).0.clone();

        div()
            .id("git-remotes-modal-overlay")
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
                    .id("git-remotes-modal-dialog")
                    .occlude()
                    .w(px(480.))
                    .h(px(340.))
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
                                    .child(div().text_base().text_color(t.primary).child("⇄"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .child("Manage Remotes"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("btn-git-remotes-close")
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
                    .child(div().flex_1().overflow_hidden().child(if !is_repo {
                        render_not_a_repo_view(svc_clone.clone(), is_loading)
                    } else {
                        render_remotes_view(
                            svc_clone.clone(),
                            remotes,
                            remote_name_input,
                            remote_url_input,
                            is_loading,
                        )
                    })),
            )
            .into_any_element()
    }
}
