use super::common::{
    collect_unified_changes, render_checkbox, render_error_banner, render_not_a_repo_view,
    render_status_badge, UnifiedChangeItem,
};
use super::diff_view::{render_diff_content, render_diff_view_toggle, DiffViewMode};
use crate::services::GitService;
use crate::state::{GitState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use std::collections::HashSet;
use zest_core::git::GitStatusResult;

#[derive(IntoElement)]
pub struct GitCommitModal {
    git_state: Entity<GitState>,
    workspace_state: Entity<WorkspaceDataState>,
    git_service: GitService,
}

impl GitCommitModal {
    pub fn new(
        git_state: Entity<GitState>,
        workspace_state: Entity<WorkspaceDataState>,
        git_service: GitService,
    ) -> Self {
        Self {
            git_state,
            workspace_state,
            git_service,
        }
    }
}

pub struct ChangesViewProps {
    pub git_clone: Entity<GitState>,
    pub ws_clone: Entity<WorkspaceDataState>,
    pub status_opt: Option<GitStatusResult>,
    pub selected_file: Option<(String, bool)>,
    pub diff_content: Option<String>,
    pub commit_input: Entity<InputState>,
    pub view_mode: DiffViewMode,
    pub mode_entity: Entity<DiffViewMode>,
    pub selected_files: HashSet<String>,
    pub commit_and_push: bool,
    pub service: GitService,
    pub is_loading: bool,
}

pub fn render_changes_view(props: ChangesViewProps) -> gpui_kit::AnyElement {
    let t = theme();
    let ChangesViewProps {
        git_clone,
        ws_clone,
        status_opt,
        selected_file,
        diff_content,
        commit_input,
        view_mode,
        mode_entity,
        selected_files,
        commit_and_push,
        service,
        is_loading,
    } = props;

    let all_files = status_opt
        .as_ref()
        .map(collect_unified_changes)
        .unwrap_or_default();
    let total_files = all_files.len();
    let selected_count = all_files
        .iter()
        .filter(|f| selected_files.contains(&f.path))
        .count();
    let all_selected = total_files > 0 && selected_count == total_files;
    let partial_selected = selected_count > 0 && !all_selected;

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
                .border_r_1()
                .border_color(t.border)
                .bg(t.bg_sidebar)
                .flex()
                .flex_col()
                // 1. Unified Changes List (Scrollable top area)
                .child(
                    div()
                        .flex_1()
                        .overflow_y_scrollbar()
                        .p(px(10.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .px(px(4.))
                                .when(total_files > 0, |this| {
                                    let git_toggle_all = git_clone.clone();
                                    this.child(render_checkbox(
                                        "chk-select-all-files",
                                        all_selected,
                                        partial_selected,
                                        move |_, _, cx| {
                                            let select = !all_selected;
                                            git_toggle_all.update(cx, |git, cx| {
                                                git.toggle_all_files_selection(select, cx);
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
                        .children(all_files.into_iter().map(|item| {
                            let git_sel = git_clone.clone();
                            let git_chk = git_clone.clone();
                            let p_chk = item.path.clone();
                            let p_sel = item.path.clone();
                            let is_checked = selected_files.contains(&item.path);
                            // Diff mode: working tree when the file has unstaged or
                            // untracked changes, staged otherwise.
                            let staged_diff = !item.has_worktree_changes;
                            let is_selected = selected_file
                                .as_ref()
                                .map(|(sel, st)| sel == &item.path && *st == staged_diff)
                                .unwrap_or(false);

                            div()
                                .id(format!("change-item-{}", item.path))
                                .px(px(6.))
                                .py(px(4.))
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
                                    let ws_disc = ws_clone.clone();
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
                                                svc_disc.discard_file(
                                                    &p_disc,
                                                    true,
                                                    cx,
                                                    move |cx| {
                                                        ws.update(cx, |ws, cx| {
                                                            ws.reload_all();
                                                            cx.notify();
                                                        });
                                                    },
                                                );
                                            })
                                            .child("↺"),
                                    )
                                })
                        })),
                )
                // 2. Commit Section (Message input + commit controls)
                .child(
                    div()
                        .p(px(10.))
                        .border_t_1()
                        .border_color(t.border)
                        .bg(t.bg_card)
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .bg(t.bg_app)
                                .border_1()
                                .border_color(t.border)
                                .rounded(px(6.))
                                .p(px(6.))
                                .h(px(68.))
                                .flex()
                                .flex_col()
                                .child(Input::new(&commit_input).appearance(false).bordered(false)),
                        )
                        .child({
                            let svc_commit = service.clone();
                            let git_push_flag = git_clone.clone();
                            let git_push_label = git_clone.clone();
                            let inp = commit_input.clone();
                            let can_commit = selected_count > 0 && !is_loading;

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
                                    div()
                                        .id("btn-commit-action")
                                        .h(px(28.))
                                        .px(px(16.))
                                        .rounded(px(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .when(can_commit, |s| {
                                            s.bg(t.primary)
                                                .cursor_pointer()
                                                .text_color(t.primary_foreground)
                                                .hover(|h| h.opacity(0.9))
                                        })
                                        .when(!can_commit, |s| {
                                            s.bg(t.bg_hover).text_color(t.text_muted)
                                        })
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .on_click(move |_, window, cx| {
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
                                        })
                                        .when(is_loading, |s| {
                                            s.child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.))
                                                    .child(
                                                        Icon::new(IconName::RefreshCw)
                                                            .size(px(14.)),
                                                    )
                                                    .child("Committing..."),
                                            )
                                        })
                                        .when(!is_loading, |s| s.child("✓ Commit")),
                                )
                        }),
                ),
        )
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
                                .when_some(selected_file.as_ref(), |this, (p, staged)| {
                                    let is_untracked = status_opt
                                        .as_ref()
                                        .map(|s| s.untracked_files.iter().any(|f| &f.path == p))
                                        .unwrap_or(false);
                                    let (tag_text, tag_color) = if *staged {
                                        ("staged", t.primary)
                                    } else if is_untracked {
                                        ("untracked", gpui_kit::rgb(0x4ade80).into())
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
                        .child(render_diff_content(diff_content, view_mode)),
                ),
        )
        .into_any_element()
}

impl RenderOnce for GitCommitModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let git_clone = self.git_state.clone();
        let ws_clone = self.workspace_state.clone();
        let svc_clone = self.git_service.clone();

        let (
            is_repo,
            branch_name,
            status_opt,
            selected_file,
            diff_content,
            git_error,
            selected_files,
            commit_and_push,
            is_loading,
        ) = {
            let git = self.git_state.read(cx);
            (
                git.status.as_ref().map(|s| s.is_repo).unwrap_or(false),
                git.branch.clone().unwrap_or_else(|| "main".to_string()),
                git.status.clone(),
                git.selected_file.clone(),
                git.diff_content.clone(),
                git.error.clone(),
                git.selected_commit_files.clone(),
                git.commit_and_push,
                git.loading,
            )
        };

        let total_changes = status_opt
            .as_ref()
            .map(|s| s.staged_files.len() + s.unstaged_files.len() + s.untracked_files.len())
            .unwrap_or(0);

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
        let view_mode = *view_mode_entity.read(cx);

        div()
            .id("git-commit-modal-overlay")
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
                    .id("git-commit-modal-dialog")
                    .occlude()
                    .w(px(1060.))
                    .h(px(680.))
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
                                    .child(div().text_base().text_color(t.primary).child("⎇"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .child("Git Commit"),
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
                                div().flex().items_center().gap(px(8.)).child(
                                    div()
                                        .id("btn-git-commit-close")
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
                        render_changes_view(ChangesViewProps {
                            git_clone: git_clone.clone(),
                            ws_clone: ws_clone.clone(),
                            status_opt,
                            selected_file,
                            diff_content,
                            commit_input,
                            view_mode,
                            mode_entity: view_mode_entity,
                            selected_files,
                            commit_and_push,
                            service: svc_clone.clone(),
                            is_loading,
                        })
                    })),
            )
            .into_any_element()
    }
}
