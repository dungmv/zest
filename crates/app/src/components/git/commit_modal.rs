use super::common::{
    render_checkbox, render_error_banner, render_not_a_repo_view, render_status_badge,
};
use super::diff_parser::DiffViewMode;
use super::diff_view::{render_diff_content, render_diff_view_toggle};
use crate::modals::{DialogHeader, DialogTitle, Modal};
use crate::services::GitService;
use crate::state::{CollectionState, GitState, UnifiedChangeItem};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::separator::Separator;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, uniform_list, AnyElement, App, AppContext as _, Entity, InteractiveElement as _,
    IntoElement, ListSizingBehavior, ParentElement, RenderOnce, StatefulInteractiveElement as _,
    Styled, UniformListScrollHandle, Window,
};
use zest_core::git::GitFileStatus;

#[derive(IntoElement)]
pub struct GitCommitModal {
    git_state: Entity<GitState>,
    workspace_state: Entity<CollectionState>,
    git_service: GitService,
}

impl GitCommitModal {
    pub fn new(
        git_state: Entity<GitState>,
        workspace_state: Entity<CollectionState>,
        git_service: GitService,
    ) -> Self {
        Self {
            git_state,
            workspace_state,
            git_service,
        }
    }

    fn render_changes_list(&self, scroll: &UniformListScrollHandle, cx: &App) -> impl IntoElement {
        let t = theme();
        let git = self.git_state.read(cx);
        let total_files = git.changes().len();
        let selected_count = git.modal.selected_commit_files.len();
        let all_selected = total_files > 0 && selected_count == total_files;
        let partial_selected = selected_count > 0 && !all_selected;
        let git_toggle_all = self.git_state.clone();
        let git_state = self.git_state.clone();
        let workspace_state = self.workspace_state.clone();
        let service = self.git_service.clone();

        div()
            .flex_1()
            .min_h(px(0.))
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(6.))
                    .px(px(4.))
                    .when(total_files > 0, |this| {
                        this.child(render_checkbox(
                            "chk-select-all-files",
                            all_selected,
                            partial_selected,
                            move |_, _, cx| {
                                git_toggle_all.update(cx, |git, cx| {
                                    git.toggle_all_files_selection(!all_selected, cx);
                                });
                            },
                        ))
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(t.text_secondary)
                            .child("Changes"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .relative()
                    .overflow_hidden()
                    .child(
                        uniform_list("git-commit-changes", total_files, move |range, _, cx| {
                            let git = git_state.read(cx);
                            range
                                .filter_map(|index| git.changes().get(index))
                                .map(|item| {
                                    Self::render_change_row(
                                        item,
                                        git,
                                        &git_state,
                                        &workspace_state,
                                        &service,
                                    )
                                })
                                .collect()
                        })
                        .size_full()
                        .with_sizing_behavior(ListSizingBehavior::Auto)
                        .track_scroll(scroll),
                    )
                    .vertical_scrollbar(scroll),
            )
    }

    fn render_change_row(
        item: &UnifiedChangeItem,
        git: &GitState,
        git_state: &Entity<GitState>,
        workspace_state: &Entity<CollectionState>,
        service: &GitService,
    ) -> AnyElement {
        let t = theme();
        let git_sel = git_state.clone();
        let git_chk = git_state.clone();
        let p_chk = item.path.clone();
        let p_sel = item.path.clone();
        let is_checked = git.modal.selected_commit_files.contains(&item.path);
        // Diff mode: working tree when the file has unstaged or
        // untracked changes, staged otherwise.
        let staged_diff = !item.has_worktree_changes;
        let is_selected = git
            .modal
            .selected_file
            .as_ref()
            .is_some_and(|(path, staged)| path == &item.path && *staged == staged_diff);

        let row = div().id(format!("change-item-{}", item.path));
        #[cfg(all(test, debug_assertions))]
        let row = row.debug_selector(|| format!("git-change-row-{}", item.path));

        row.h(px(26.))
            .px(px(6.))
            .rounded(px(4.))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .when(is_selected, |s| s.bg(t.bg_hover))
            .hover(|s| s.bg(t.bg_hover))
            .on_click(move |_, _, cx| {
                git_sel.update(cx, |git, cx| {
                    git.select_file(p_sel.clone(), staged_diff, cx);
                });
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .overflow_hidden()
                    .child(render_checkbox(
                        &format!("chk-file-{}", item.path),
                        is_checked,
                        false,
                        move |_, _, cx| {
                            git_chk.update(cx, |git, cx| {
                                git.toggle_file_selection(&p_chk, cx);
                            });
                        },
                    ))
                    .child(render_status_badge(item.status))
                    .child(
                        div()
                            .text_xs()
                            .font_family(".SystemMonoFont")
                            .text_color(t.text_primary)
                            .truncate()
                            .child(item.path.clone()),
                    ),
            )
            .when(item.has_worktree_changes, |this| {
                let svc_disc = service.clone();
                let ws_disc = workspace_state.clone();
                let p_disc = item.path.clone();

                this.child(
                    // Discard button (↺)
                    div()
                        .id(format!("btn-discard-{}", item.path))
                        .cursor_pointer()
                        .p(px(2.))
                        .rounded(px(3.))
                        .text_xs()
                        .text_color(t.text_muted)
                        .hover(|s| s.bg(t.bg_card).text_color(t.status_5xx))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            let ws = ws_disc.clone();
                            svc_disc.discard_file(&p_disc, true, cx, move |cx| {
                                ws.update(cx, |ws, cx| {
                                    ws.request_rescan(cx);
                                });
                            });
                        })
                        .child("↺"),
                )
            })
            .into_any_element()
    }
}

fn render_changes_view(
    modal: &GitCommitModal,
    commit_input: Entity<InputState>,
    mode_entity: Entity<DiffViewMode>,
    scroll: &UniformListScrollHandle,
    cx: &App,
) -> AnyElement {
    let t = theme();
    let git = modal.git_state.read(cx);
    let selected_file = git.modal.selected_file.as_ref();
    let commit_and_push = git.modal.commit_and_push;
    let is_loading = git.operation.is_loading();
    let can_commit = !git.modal.selected_commit_files.is_empty() && !is_loading;
    let view_mode = *mode_entity.read(cx);

    div()
        .size_full()
        .flex()
        // Left Column: Unified file list and Commit input (width: 350px)
        .child(
            div()
                .w(px(350.))
                .min_w(px(350.))
                .max_w(px(350.))
                .flex_shrink_0()
                .h_full()
                .bg(t.bg_sidebar)
                .flex()
                .flex_col()
                .child(modal.render_changes_list(scroll, cx))
                // Separator before commit section
                .child(Separator::horizontal())
                // 2. Commit Section (Message input + commit controls)
                .child(
                    div()
                        .p(px(10.))
                        .bg(t.bg_card)
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child({
                            let input = div()
                                .bg(t.bg_app)
                                .border_1()
                                .border_color(t.border)
                                .rounded(px(6.))
                                .p(px(6.))
                                .h(px(68.))
                                .flex()
                                .flex_col()
                                .child(Input::new(&commit_input).appearance(false).bordered(false));
                            #[cfg(all(test, debug_assertions))]
                            let input = input.debug_selector(|| "git-commit-input".into());
                            input
                        })
                        .child({
                            let svc_commit = modal.git_service.clone();
                            let git_push_flag = modal.git_state.clone();
                            let git_push_label = modal.git_state.clone();
                            let inp = commit_input.clone();

                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    // "commit and push" toggle: commit also pushes to remote.
                                    // The whole cluster (checkbox + label) toggles together.
                                    div()
                                        .id("btn-toggle-commit-and-push")
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.85))
                                        .on_click(move |_, _, cx| {
                                            let next = !commit_and_push;
                                            git_push_label.update(cx, |git, cx| {
                                                git.set_commit_and_push(next, cx);
                                            });
                                        })
                                        .child(render_checkbox(
                                            "chk-commit-and-push",
                                            commit_and_push,
                                            false,
                                            move |_, _, cx| {
                                                let next = !commit_and_push;
                                                git_push_flag.update(cx, |git, cx| {
                                                    git.set_commit_and_push(next, cx);
                                                });
                                            },
                                        ))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(t.text_secondary)
                                                .child("commit and push"),
                                        ),
                                )
                                .child(
                                    Button::new("btn-commit-action")
                                        .primary()
                                        .small()
                                        .disabled(!can_commit)
                                        .loading(is_loading)
                                        .label(if is_loading {
                                            "Committing..."
                                        } else {
                                            "✓ Commit"
                                        })
                                        .on_click(move |_, window, cx: &mut App| {
                                            if !can_commit {
                                                return;
                                            }
                                            let msg = inp.read(cx).value().to_string();
                                            if msg.trim().is_empty() {
                                                return;
                                            }
                                            inp.update(cx, |inp_st, cx| {
                                                inp_st.set_value("", window, cx);
                                            });
                                            svc_commit.commit(&msg, cx);
                                        }),
                                )
                        }),
                ),
        )
        // Separator between file list and diff view
        .child(Separator::vertical())
        // Right Column: Diff Viewer (Flex-1)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .overflow_hidden()
                .bg(t.bg_app)
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(40.))
                        .px(px(14.))
                        .border_b_1()
                        .border_color(t.border)
                        .bg(t.bg_card)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .when_some(selected_file, |this, (p, staged)| {
                                    let is_untracked = git.change(p).is_some_and(|file| {
                                        file.status == GitFileStatus::Untracked
                                    });
                                    let (tag_text, tag_color) = if *staged {
                                        ("staged", t.primary)
                                    } else if is_untracked {
                                        ("untracked", t.git_untracked)
                                    } else {
                                        ("working tree", t.text_secondary)
                                    };

                                    this.child(
                                        div()
                                            .px(px(6.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(t.bg_hover)
                                            .border_1()
                                            .border_color(t.border)
                                            .text_xs()
                                            .text_color(tag_color)
                                            .child(tag_text),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(".SystemMonoFont")
                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                            .text_color(t.text_primary)
                                            .child(p.clone()),
                                    )
                                })
                                .when(selected_file.is_none(), |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(t.text_muted)
                                            .child("No file selected"),
                                    )
                                }),
                        )
                        .child(render_diff_view_toggle(view_mode, mode_entity.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .overflow_y_scrollbar()
                        .p(px(8.))
                        .child(render_diff_content(
                            git.modal.diff_content.as_deref(),
                            view_mode,
                        )),
                ),
        )
        .into_any_element()
}

impl RenderOnce for GitCommitModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let git_clone = self.git_state.clone();
        let git = self.git_state.read(cx);
        let is_repo = git.status().is_some_and(|status| status.is_repo);
        let branch_name = git
            .branch()
            .map(str::to_owned)
            .unwrap_or_else(|| "main".to_string());
        let is_loading = git.operation.is_loading();
        let git_error = git.operation.error().map(str::to_owned);
        let total_changes = git.status().map_or(0, |status| {
            status.staged_files.len() + status.unstaged_files.len() + status.untracked_files.len()
        });

        let commit_input_holder = window.use_keyed_state(
            gpui_kit::SharedString::from("git-commit-msg-input"),
            cx,
            |window, cx| {
                let inp = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Commit message (e.g. feat: add login endpoint)")
                });
                (inp, ())
            },
        );
        let commit_input = commit_input_holder.read(cx).0.clone();

        let view_mode_holder = window.use_keyed_state(
            gpui_kit::SharedString::from("git-diff-view-mode"),
            cx,
            |_window, cx| {
                let st = cx.new(|_cx| DiffViewMode::Split);
                (st, ())
            },
        );
        let view_mode_entity = view_mode_holder.read(cx).0.clone();
        let scroll = window.use_keyed_state("git-commit-changes-scroll", cx, |_, _| {
            UniformListScrollHandle::new()
        });

        let header_el = div()
            .h(px(46.))
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
                    .gap(px(10.))
                    .child(Icon::new(IconName::GitCommitHorizontal).size(px(18.)))
                    .child(
                        DialogHeader::new().child(
                            DialogTitle::new()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_primary)
                                .child("Git Commit"),
                        ),
                    )
                    .when(is_repo, |this| {
                        this.child(
                            div()
                                .px(px(6.))
                                .py(px(2.))
                                .rounded(px(4.))
                                .bg(t.bg_hover)
                                .border_1()
                                .border_color(t.border)
                                .text_xs()
                                .font_family(".SystemMonoFont")
                                .text_color(t.text_secondary)
                                .child(branch_name),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_muted)
                                .child(format!("({} changes)", total_changes)),
                        )
                    }),
            )
            .child(
                Button::new("btn-git-commit-close")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .on_click({
                        let git_s = git_clone.clone();
                        move |_, _, cx| {
                            git_s.update(cx, |state, cx| {
                                state.close_modal(cx);
                            });
                        }
                    }),
            );

        Modal::new("git-commit-modal")
            .w(px(1060.))
            .h(px(680.))
            .header(header_el)
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
            .child(div().flex_1().overflow_hidden().child(if !is_repo {
                render_not_a_repo_view(self.git_service.clone(), is_loading)
            } else {
                render_changes_view(&self, commit_input, view_mode_entity, scroll.read(cx), cx)
            }))
            .into_any_element()
    }
}
