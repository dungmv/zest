use crate::services::GitService;
use crate::state::GitState;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};
use std::collections::HashMap;
use zest_core::git::{GitFileStatus, GitStatusResult};

/// A single pending change shown in the unified file list. A path that has both
/// staged and working-tree changes collapses into one item; the working-tree
/// state wins because that is what a commit includes.
pub struct UnifiedChangeItem {
    pub path: String,
    pub status: GitFileStatus,
    pub has_worktree_changes: bool,
}

pub fn collect_unified_changes(status: &GitStatusResult) -> Vec<UnifiedChangeItem> {
    let mut items: Vec<UnifiedChangeItem> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for f in &status.staged_files {
        index.insert(f.path.clone(), items.len());
        items.push(UnifiedChangeItem {
            path: f.path.clone(),
            status: f.status,
            has_worktree_changes: false,
        });
    }
    for f in status.unstaged_files.iter().chain(&status.untracked_files) {
        match index.get(&f.path) {
            Some(&i) => {
                items[i].status = f.status;
                items[i].has_worktree_changes = true;
            }
            None => {
                index.insert(f.path.clone(), items.len());
                items.push(UnifiedChangeItem {
                    path: f.path.clone(),
                    status: f.status,
                    has_worktree_changes: true,
                });
            }
        }
    }
    items
}

pub fn render_status_badge(status: GitFileStatus) -> gpui_kit::AnyElement {
    let color = match status {
        GitFileStatus::Modified => gpui_kit::rgb(0xfacc15),
        GitFileStatus::Added | GitFileStatus::Untracked => gpui_kit::rgb(0x4ade80),
        GitFileStatus::Deleted => gpui_kit::rgb(0xef4444),
        _ => gpui_kit::rgb(0x9ca3af),
    };

    div()
        .w(px(16.))
        .h(px(16.))
        .rounded(px(3.))
        .bg(gpui_kit::Hsla {
            a: 0.15,
            ..color.into()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_weight(gpui_kit::FontWeight::BOLD)
        .text_color(color)
        .child(status.code())
        .into_any_element()
}

pub fn render_checkbox(
    id: &str,
    checked: bool,
    partial: bool,
    on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> gpui_kit::AnyElement {
    if partial {
        let t = theme();
        div()
            .id(gpui_kit::SharedString::from(id.to_string()))
            .w(px(16.))
            .h(px(16.))
            .flex_shrink_0()
            .rounded(px(3.))
            .border_1()
            .border_color(t.primary)
            .bg(t.primary)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_xs()
            .font_weight(gpui_kit::FontWeight::BOLD)
            .text_color(t.primary_foreground)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                on_toggle(&true, window, cx);
            })
            .child("−")
            .into_any_element()
    } else {
        Checkbox::new(id.to_string())
            .checked(checked)
            .on_click(move |val, window, cx| {
                on_toggle(val, window, cx);
            })
            .into_any_element()
    }
}

pub fn render_error_banner(git_state: Entity<GitState>, err: String) -> gpui_kit::AnyElement {
    let t = theme();
    let git_err = git_state.clone();

    div()
        .px(px(16.))
        .py(px(8.))
        .bg(gpui_kit::Hsla {
            h: 0.0,
            s: 0.7,
            l: 0.45,
            a: 0.2,
        })
        .border_b_1()
        .border_color(t.status_5xx)
        .flex()
        .items_center()
        .justify_between()
        .child(
            div().text_xs().text_color(t.status_5xx).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(Icon::new(IconName::TriangleAlert).size(px(14.)))
                    .child(format!("Git: {}", err)),
            ),
        )
        .child(
            div()
                .id("btn-dismiss-git-error")
                .cursor_pointer()
                .text_xs()
                .text_color(t.text_muted)
                .hover(|s| s.text_color(t.text_primary))
                .on_click(move |_, _, cx| {
                    git_err.update(cx, |git, cx| {
                        git.error = None;
                        cx.notify();
                    });
                })
                .child("✕"),
        )
        .into_any_element()
}

pub fn render_not_a_repo_view(git_service: GitService, is_loading: bool) -> gpui_kit::AnyElement {
    let t = theme();
    let svc_init = git_service;

    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.))
        .child(div().text_2xl().text_color(t.text_muted).child("📦"))
        .child(
            div()
                .text_base()
                .font_weight(gpui_kit::FontWeight::BOLD)
                .text_color(t.text_primary)
                .child("Not a Git Repository"),
        )
        .child(
            div()
                .max_w(px(400.))
                .text_center()
                .text_xs()
                .text_color(t.text_muted)
                .child(
                    "This workspace directory does not have Git version control initialized yet.",
                ),
        )
        .child(
            div()
                .id("btn-git-init")
                .mt(px(8.))
                .px(px(16.))
                .py(px(8.))
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
                .on_click(move |_, _, cx| {
                    if is_loading {
                        return;
                    }
                    svc_init.init_repo(cx);
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .when(is_loading, |s| {
                            s.child(Icon::new(IconName::RefreshCw).size(px(14.)))
                                .child("Initializing...")
                        })
                        .when(!is_loading, |s| {
                            s.child(Icon::new(IconName::GitBranch).size(px(14.)))
                                .child("Initialize Git Repository")
                        }),
                ),
        )
        .into_any_element()
}
