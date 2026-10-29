use crate::services::GitService;
use crate::state::GitState;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
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
    let git_err = git_state.clone();

    Alert::error("git-error-banner", format!("Git: {}", err.trim()))
        .banner()
        .on_close(move |_, _, cx| {
            git_err.update(cx, |git, cx| {
                git.error = None;
                cx.notify();
            });
        })
        .into_any_element()
}

pub fn render_not_a_repo_view(git_service: GitService, is_loading: bool) -> gpui_kit::AnyElement {
    let svc_init = git_service;

    Empty::new()
        .header(
            EmptyHeader::new()
                .media(
                    EmptyMedia::new()
                        .with_variant(EmptyMediaVariant::Icon)
                        .child(Icon::new(IconName::GitBranch).size(px(24.))),
                )
                .title(EmptyTitle::new().child("Not a Git Repository"))
                .description(EmptyDescription::new().child(
                    "This workspace directory does not have Git version control initialized yet.",
                )),
        )
        .content(
            EmptyContent::new().child(
                Button::new("btn-git-init")
                    .primary()
                    .loading(is_loading)
                    .icon(IconName::GitBranch)
                    .label(if is_loading {
                        "Initializing..."
                    } else {
                        "Initialize Git Repository"
                    })
                    .on_click(move |_, _, cx| {
                        if is_loading {
                            return;
                        }
                        svc_init.init_repo(cx);
                    }),
            ),
        )
        .into_any_element()
}
