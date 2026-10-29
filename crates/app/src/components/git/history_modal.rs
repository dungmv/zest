use super::common::{render_error_banner, render_not_a_repo_view};
use super::diff_view::{
    render_diff_content, render_diff_view_toggle, split_diff_into_files, DiffViewMode,
};
use crate::components::modal_dialog::{DialogHeader, DialogTitle, ModalDialog};
use crate::services::GitService;
use crate::state::GitState;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use std::collections::HashSet;

#[derive(IntoElement)]
pub struct GitHistoryModal {
    git_state: Entity<GitState>,
    git_service: GitService,
}

impl GitHistoryModal {
    pub fn new(git_state: Entity<GitState>, git_service: GitService) -> Self {
        Self {
            git_state,
            git_service,
        }
    }
}

pub fn render_history_view(
    git_clone: Entity<GitState>,
    history: Vec<zest_core::git::GitCommitInfo>,
    selected_commit: Option<zest_core::git::GitCommitDetail>,
    view_mode: DiffViewMode,
    mode_entity: Entity<DiffViewMode>,
    expanded_files: HashSet<String>,
) -> gpui_kit::AnyElement {
    let t = theme();

    div()
        .size_full()
        .flex()
        // Left Column: Commit Log (rigidly fixed width: 350px)
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
                .child(
                    div()
                        .flex_1()
                        .overflow_y_scrollbar()
                        .children(history.into_iter().map(|item| {
                            let git_sel = git_clone.clone();
                            let h = item.hash.clone();
                            let is_selected = selected_commit.as_ref().map(|c| c.commit.hash == item.hash).unwrap_or(false);

                            div()
                                .id(format!("commit-item-{}", item.hash))
                                .px(px(12.))
                                .py(px(7.))
                                .border_b_1()
                                .border_color(t.border)
                                .cursor_pointer()
                                .when(is_selected, |s| s.bg(t.bg_hover))
                                .hover(|s| s.bg(t.bg_hover))
                                .on_click(move |_, _, cx| {
                                    git_sel.update(cx, |git, cx| {
                                        git.select_commit(&h, cx);
                                    });
                                })
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                // Row 1: Commit Message (single line, truncate)
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(t.text_primary)
                                        .truncate()
                                        .child(item.message),
                                )
                                // Row 2: Metadata (Username, hash on left; time on right)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(6.))
                                                .min_w(px(0.))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(t.text_secondary)
                                                        .truncate()
                                                        .child(item.author_name),
                                                )
                                                .child(
                                                    div()
                                                        .px(px(4.))
                                                        .py(px(0.5))
                                                        .rounded(px(3.))
                                                        .bg(t.bg_card)
                                                        .border_1()
                                                        .border_color(t.border)
                                                        .text_xs()
                                                        .font_family(".SystemMonoFont")
                                                        .text_color(t.primary)
                                                        .flex_shrink_0()
                                                        .child(item.short_hash),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(t.text_muted)
                                                .flex_shrink_0()
                                                .child(item.relative_date),
                                        ),
                                )
                        })),
                ),
        )
        // Right Column: Commit Details & Collapsible File Diffs (Flex-1)
        .child({
            if let Some(commit_detail) = selected_commit {
                let parsed_diffs = split_diff_into_files(&commit_detail.diff);
                let mut diff_map: std::collections::HashMap<String, String> = parsed_diffs.into_iter().collect();

                let mut file_entries: Vec<(String, String)> = Vec::new();
                for f in &commit_detail.changed_files {
                    let diff = diff_map.remove(f).unwrap_or_default();
                    file_entries.push((f.clone(), diff));
                }
                for (f, diff) in diff_map {
                    file_entries.push((f, diff));
                }
                if file_entries.is_empty() && !commit_detail.diff.trim().is_empty() {
                    file_entries.push(("Changes".to_string(), commit_detail.diff.clone()));
                }

                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .overflow_hidden()
                    .bg(t.bg_app)
                    .flex()
                    .flex_col()
                    .child(
                        // Header details (Without duplicate chip badges)
                        div()
                            .p(px(14.))
                            .border_b_1()
                            .border_color(t.border)
                            .bg(t.bg_card)
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .truncate()
                                            .child(commit_detail.commit.message.clone()),
                                    )
                                    .child(render_diff_view_toggle(view_mode, mode_entity.clone())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(12.))
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(format!("Commit: {}", commit_detail.commit.hash))
                                    .child(format!("Author: {} <{}>", commit_detail.commit.author_name, commit_detail.commit.author_email))
                                    .child(format!("Date: {}", commit_detail.commit.relative_date))
                                    .child(format!("• {} file{}", file_entries.len(), if file_entries.len() == 1 { "" } else { "s" })),
                            ),
                    )
                    .child(
                        // Collapsible file changes (default closed)
                        div()
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scrollbar()
                            .p(px(12.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .when(file_entries.is_empty(), |this| {
                                this.child(
                                    div()
                                        .py(px(24.))
                                        .text_center()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .child("No file changes in this commit."),
                                )
                            })
                            .children(file_entries.into_iter().map(|(file_path, file_diff)| {
                                let is_expanded = expanded_files.contains(&file_path);
                                let git_toggle = git_clone.clone();
                                let p = file_path.clone();

                                div()
                                    .id(format!("history-file-{}", file_path))
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(t.border)
                                    .bg(t.bg_card)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .id(format!("btn-toggle-file-{}", file_path))
                                            .px(px(10.))
                                            .py(px(7.))
                                            .bg(if is_expanded { t.bg_hover } else { t.bg_sidebar })
                                            .hover(|s| s.bg(t.bg_hover))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .on_click(move |_, _, cx| {
                                                git_toggle.update(cx, |git, cx| {
                                                    git.toggle_history_file_expansion(&p, cx);
                                                });
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(t.text_muted)
                                                            .child(if is_expanded { "▼" } else { "▶" }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_family(".SystemMonoFont")
                                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                            .text_color(t.text_primary)
                                                            .child(format!("📄 {}", file_path)),
                                                    ),
                                            ),
                                    )
                                    .when(is_expanded, |this| {
                                        this.child(
                                            div()
                                                .border_t_1()
                                                .border_color(t.border)
                                                .bg(t.bg_app)
                                                .p(px(6.))
                                                .child(if file_diff.trim().is_empty() {
                                                    div()
                                                        .p(px(8.))
                                                        .text_xs()
                                                        .text_color(t.text_muted)
                                                        .child("No text changes (binary file or empty change).")
                                                        .into_any_element()
                                                } else {
                                                    render_diff_content(Some(file_diff), view_mode)
                                                }),
                                        )
                                    })
                            })),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .bg(t.bg_app)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("Select a commit from the list to view changes.")
                    .into_any_element()
            }
        })
        .into_any_element()
}

impl RenderOnce for GitHistoryModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let git_clone = self.git_state.clone();
        let svc_clone = self.git_service.clone();

        let (is_repo, branch_name, history, selected_commit, git_error, expanded_files, is_loading) = {
            let git = self.git_state.read(cx);
            (
                git.status.as_ref().map(|s| s.is_repo).unwrap_or(false),
                git.branch.clone().unwrap_or_else(|| "main".to_string()),
                git.history.clone(),
                git.selected_commit.clone(),
                git.error.clone(),
                git.expanded_history_files.clone(),
                git.loading,
            )
        };

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
                    .child(Icon::new(IconName::Clock).size(px(18.)))
                    .child(
                        DialogHeader::new().child(
                            DialogTitle::new()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_primary)
                                .child("Git History"),
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
                    }),
            )
            .child(
                Button::new("btn-git-history-close")
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

        ModalDialog::new("git-history-modal")
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
                render_not_a_repo_view(svc_clone.clone(), is_loading)
            } else {
                render_history_view(
                    git_clone.clone(),
                    history,
                    selected_commit,
                    view_mode,
                    view_mode_entity,
                    expanded_files,
                )
            }))
            .into_any_element()
    }
}
