//! Repository state and Git modal selections.

mod operation;
mod queries;
mod working_tree;

use gpui_kit::Context;
use operation::GitOperation;
use std::collections::HashSet;
use std::path::PathBuf;
use working_tree::GitWorkingTree;
pub use working_tree::UnifiedChangeItem;
use zest_core::git::GitStatusResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitActiveModal {
    Commit,
    History,
    NewBranch,
    Remotes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitTab {
    #[default]
    Changes,
    History,
    Branches,
}

/// Modal-local UI state of the Git panel (commit / history / branch /
/// remotes dialogs). Grouped so [`GitState`] stays under 15 fields.
#[derive(Debug, Clone, Default)]
pub struct GitModalUiState {
    pub is_open: bool,
    pub active: Option<GitActiveModal>,
    pub tab: GitTab,
    pub selected_file: Option<(String, bool)>,
    pub diff_content: Option<String>,
    pub selected_commit: Option<zest_core::git::GitCommitDetail>,
    pub expanded_history_files: HashSet<String>,
    pub selected_commit_files: HashSet<String>,
    pub commit_and_push: bool,
}

#[derive(Debug, Clone, Default)]
struct GitQueries {
    repo: u64,
    status: u64,
    diff: u64,
    history: u64,
    commit: u64,
    branches: u64,
    remotes: u64,
}

#[derive(Debug, Clone)]
pub struct GitState {
    pub repo_path: PathBuf,
    pub tracked_warning: bool,
    pub history: Vec<zest_core::git::GitCommitInfo>,
    pub branches: Vec<zest_core::git::GitBranchInfo>,
    pub remotes: Vec<zest_core::git::GitRemote>,
    pub modal: GitModalUiState,
    pub operation: GitOperation,
    working_tree: GitWorkingTree,
    queries: GitQueries,
}

impl Default for GitState {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::new(),
            tracked_warning: false,
            history: Vec::new(),
            branches: Vec::new(),
            remotes: Vec::new(),
            modal: GitModalUiState {
                tab: GitTab::Changes,
                ..Default::default()
            },
            operation: GitOperation::default(),
            working_tree: GitWorkingTree::default(),
            queries: GitQueries::default(),
        }
    }
}

impl GitState {
    pub fn new(repo_path: PathBuf) -> Self {
        let fast = zest_core::git::detect_git_repo_fast(&repo_path);
        let (branch, status) = match fast {
            Some((_root, branch_opt)) => {
                let branch_name = branch_opt.unwrap_or_else(|| "main".to_string());
                (
                    Some(branch_name.clone()),
                    Some(GitStatusResult {
                        is_repo: true,
                        branch: branch_name,
                        ..Default::default()
                    }),
                )
            }
            None => (
                None,
                Some(GitStatusResult {
                    is_repo: false,
                    ..Default::default()
                }),
            ),
        };

        Self {
            repo_path,
            working_tree: GitWorkingTree {
                branch,
                status,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn set_repo_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        *self = Self {
            repo_path: path,
            working_tree: self.working_tree.reset(),
            queries: GitQueries {
                repo: self.queries.repo.wrapping_add(1),
                ..Default::default()
            },
            ..Default::default()
        };
        self.refresh_all(cx);
        cx.notify();
    }

    pub(crate) fn repo_generation(&self) -> u64 {
        self.queries.repo
    }

    pub fn open_modal(&mut self, modal: GitActiveModal, cx: &mut Context<Self>) {
        self.modal.active = Some(modal);
        self.modal.is_open = true;
        self.operation.clear_error();
        match modal {
            GitActiveModal::Commit => {
                self.modal.selected_commit_files.clear();
                self.modal.commit_and_push = false;
                self.refresh_status(cx);
            }
            GitActiveModal::History => {
                self.modal.expanded_history_files.clear();
                self.refresh_history(cx);
            }
            GitActiveModal::NewBranch => {
                self.refresh_branches(cx);
            }
            GitActiveModal::Remotes => {
                self.refresh_remotes(cx);
            }
        }
        cx.notify();
    }

    pub fn close_modal(&mut self, cx: &mut Context<Self>) -> bool {
        if self.modal.is_open || self.modal.active.is_some() {
            self.queries.diff = self.queries.diff.wrapping_add(1);
            self.queries.commit = self.queries.commit.wrapping_add(1);
            self.modal.active = None;
            self.modal.is_open = false;
            self.operation.clear_error();
            self.modal.selected_commit_files.clear();
            self.modal.commit_and_push = false;
            self.modal.expanded_history_files.clear();
            cx.notify();
            true
        } else {
            false
        }
    }

    pub fn toggle_modal(&mut self, cx: &mut Context<Self>) {
        if self.modal.is_open || self.modal.active.is_some() {
            self.close_modal(cx);
        } else {
            self.open_modal(GitActiveModal::Commit, cx);
        }
    }

    pub fn set_tab(&mut self, tab: GitTab, cx: &mut Context<Self>) {
        self.modal.tab = tab;
        match tab {
            GitTab::Changes => self.refresh_status(cx),
            GitTab::History => self.refresh_history(cx),
            GitTab::Branches => self.refresh_branches(cx),
        }
        cx.notify();
    }

    pub fn toggle_history_file_expansion(&mut self, file_path: &str, cx: &mut Context<Self>) {
        if self.modal.expanded_history_files.contains(file_path) {
            self.modal.expanded_history_files.remove(file_path);
        } else {
            self.modal
                .expanded_history_files
                .insert(file_path.to_string());
        }
        cx.notify();
    }

    pub fn branch(&self) -> Option<&str> {
        self.working_tree.branch.as_deref()
    }

    pub fn status(&self) -> Option<&GitStatusResult> {
        self.working_tree.status.as_ref()
    }

    pub fn status_version(&self) -> usize {
        self.working_tree.version
    }

    pub(crate) fn set_branch(&mut self, branch: String) {
        self.working_tree.branch = Some(branch);
    }

    pub fn changes(&self) -> &[UnifiedChangeItem] {
        &self.working_tree.changes.rows
    }

    pub fn change(&self, path: &str) -> Option<&UnifiedChangeItem> {
        self.working_tree.changes.get(path)
    }

    pub fn toggle_file_selection(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.modal.selected_commit_files.remove(path) && self.change(path).is_some() {
            self.modal.selected_commit_files.insert(path.to_string());
        }
        cx.notify();
    }

    pub fn toggle_all_files_selection(&mut self, select: bool, cx: &mut Context<Self>) {
        if select {
            self.modal.selected_commit_files = self
                .working_tree
                .changes
                .rows
                .iter()
                .map(|file| file.path.clone())
                .collect();
        } else {
            self.modal.selected_commit_files.clear();
        }
        cx.notify();
    }

    pub fn set_commit_and_push(&mut self, value: bool, cx: &mut Context<Self>) {
        self.modal.commit_and_push = value;
        cx.notify();
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, debug_assertions))]
mod query_tests;
