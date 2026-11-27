use super::*;
use crate::components::git::GitCommitModal;
use crate::services::GitService;
use crate::state::CollectionState;
use gpui_kit::component::input::{Copy, SelectAll};
use gpui_kit::{
    div, point, px, size, AppContext as _, Bounds, Entity, IntoElement, Modifiers, ParentElement,
    Render, ScrollDelta, ScrollEvent, Styled, TestAppContext, VisualTestContext, Window,
    WindowBounds, WindowOptions,
};
use std::sync::Arc;
use zest_core::git::GitStatusItem;
use zest_core::storage::FsWorkspaceStore;

struct CommitModalHost {
    git: Entity<GitState>,
    collection: Entity<CollectionState>,
}

impl Render for CommitModalHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(GitCommitModal::new(
            self.git.clone(),
            self.collection.clone(),
            GitService::new(self.git.clone()),
        ))
    }
}

#[gpui_kit::test]
fn commit_modal_virtualizes_1589_changes(cx: &mut TestAppContext) {
    let status = GitStatusResult {
        is_repo: true,
        staged_files: vec![GitStatusItem {
            path: "file-0".into(),
            status: GitFileStatus::Added,
            staged: true,
        }],
        unstaged_files: vec![GitStatusItem {
            path: "file-0".into(),
            status: GitFileStatus::Modified,
            staged: false,
        }],
        untracked_files: (1..1589)
            .map(|i| GitStatusItem {
                path: format!("file-{i}"),
                status: GitFileStatus::Untracked,
                staged: false,
            })
            .collect(),
        ..Default::default()
    };
    let changes = GitChanges::from_status(&status);
    assert_eq!(changes.rows.len(), 1589);
    assert_eq!(changes.rows[0].status, GitFileStatus::Modified);
    assert!(changes.rows[0].has_worktree_changes);

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().to_path_buf();
    let git = cx.new(|_| GitState {
        status: Some(status),
        changes,
        ..Default::default()
    });
    let collection = cx.new(|_| {
        CollectionState::new(
            Arc::new(FsWorkspaceStore::new(&path)),
            path,
            "Test Workspace".into(),
        )
    });
    let window = cx.update(|cx| {
        gpui_kit::component::init(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1200.), px(800.)),
                ))),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| CommitModalHost {
                    git: git.clone(),
                    collection,
                })
            },
        )
        .unwrap()
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    let first_row = cx.debug_bounds("git-change-row-file-0").unwrap();
    assert!(cx.debug_bounds("git-change-row-file-1588").is_none());

    cx.simulate_event(ScrollEvent {
        position: first_row.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-50_000.))),
        ..Default::default()
    });

    assert!(cx.debug_bounds("git-change-row-file-0").is_none());
    let last_row = cx.debug_bounds("git-change-row-file-1588").unwrap();

    let input = cx.debug_bounds("git-commit-input").unwrap();
    cx.simulate_click(input.origin + point(px(20.), px(15.)), Modifiers::none());
    cx.simulate_input("Update API requests");
    cx.dispatch_action(SelectAll);
    cx.dispatch_action(Copy);

    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("Update API requests")
    );
    assert_eq!(cx.debug_bounds("git-change-row-file-1588"), Some(last_row));

    git.update(&mut cx, |git, cx| {
        git.toggle_all_files_selection(true, cx);
        assert_eq!(git.modal.selected_commit_files.len(), 1589);
        git.toggle_file_selection("file-1588", cx);
        assert_eq!(git.modal.selected_commit_files.len(), 1588);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("git-change-row-file-1588").is_some());
}
