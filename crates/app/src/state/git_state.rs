use gpui_kit::Context;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use zest_core::git::{GitFileStatus, GitStatusResult};

/// One row per path, in Git status order. Worktree changes take precedence over
/// staged changes for the badge and diff mode.
#[derive(Debug, Clone)]
pub struct UnifiedChangeItem {
    pub path: String,
    pub status: GitFileStatus,
    pub has_worktree_changes: bool,
}

#[derive(Debug, Clone, Default)]
struct GitChanges {
    rows: Vec<UnifiedChangeItem>,
    by_path: HashMap<String, usize>,
}

impl GitChanges {
    fn from_status(status: &GitStatusResult) -> Self {
        let mut changes = Self::default();
        let entries = status.staged_files.iter().map(|file| (file, false)).chain(
            status
                .unstaged_files
                .iter()
                .chain(&status.untracked_files)
                .map(|file| (file, true)),
        );

        for (file, has_worktree_changes) in entries {
            if let Some(&index) = changes.by_path.get(&file.path) {
                changes.rows[index].status = file.status;
                changes.rows[index].has_worktree_changes = has_worktree_changes;
            } else {
                changes
                    .by_path
                    .insert(file.path.clone(), changes.rows.len());
                changes.rows.push(UnifiedChangeItem {
                    path: file.path.clone(),
                    status: file.status,
                    has_worktree_changes,
                });
            }
        }

        changes
    }

    fn get(&self, path: &str) -> Option<&UnifiedChangeItem> {
        self.by_path.get(path).map(|&index| &self.rows[index])
    }
}

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

#[derive(Debug, Clone)]
pub struct GitState {
    pub repo_path: PathBuf,
    pub branch: Option<String>,
    pub tracked_warning: bool,
    pub status: Option<GitStatusResult>,
    changes: GitChanges,
    pub history: Vec<zest_core::git::GitCommitInfo>,
    pub branches: Vec<zest_core::git::GitBranchInfo>,
    pub remotes: Vec<zest_core::git::GitRemote>,
    pub error: Option<String>,
    pub loading: bool,
    pub is_syncing: bool,
    pub sync_message: Option<String>,
    pub status_version: usize,
    pub modal: GitModalUiState,
}

impl Default for GitState {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::new(),
            branch: None,
            tracked_warning: false,
            status: None,
            changes: GitChanges::default(),
            history: Vec::new(),
            branches: Vec::new(),
            remotes: Vec::new(),
            error: None,
            loading: false,
            is_syncing: false,
            sync_message: None,
            status_version: 0,
            modal: GitModalUiState {
                tab: GitTab::Changes,
                ..Default::default()
            },
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
            branch,
            status,
            ..Default::default()
        }
    }

    pub fn set_repo_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.repo_path = path;
        let fast = zest_core::git::detect_git_repo_fast(&self.repo_path);
        match fast {
            Some((_root, branch_opt)) => {
                let branch_name = branch_opt.unwrap_or_else(|| "main".to_string());
                self.branch = Some(branch_name.clone());
                self.status = Some(GitStatusResult {
                    is_repo: true,
                    branch: branch_name,
                    ..Default::default()
                });
            }
            None => {
                self.branch = None;
                self.status = Some(GitStatusResult {
                    is_repo: false,
                    ..Default::default()
                });
            }
        }
        self.changes = GitChanges::default();
        self.modal.selected_commit_files.clear();
        self.status_version = self.status_version.wrapping_add(1);
        self.refresh_all(cx);
    }

    pub fn open_modal(&mut self, modal: GitActiveModal, cx: &mut Context<Self>) {
        self.modal.active = Some(modal);
        self.modal.is_open = true;
        self.error = None;
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
            self.modal.active = None;
            self.modal.is_open = false;
            self.error = None;
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

    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        self.refresh_status(cx);
        self.refresh_history(cx);
        self.refresh_branches(cx);
        self.refresh_remotes(cx);
    }

    pub fn refresh_remotes(&mut self, cx: &mut Context<Self>) {
        let ws_path = self.repo_path.clone();
        cx.spawn(async move |this, cx| {
            let remotes = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_get_remotes(&ws_path).ok() })
                .await;
            if let Some(remotes) = remotes {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |git, cx| {
                        git.remotes = remotes;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn refresh_status(&mut self, cx: &mut Context<Self>) {
        let ws_path = self.repo_path.clone();
        let show_modal = self.modal.is_open;
        let selected_file = self.modal.selected_file.clone();

        cx.spawn(async move |this, cx| {
            let ws_status = ws_path.clone();
            let status_res = cx
                .background_executor()
                .spawn(async move {
                    zest_core::git::git_status(&ws_status).map(|status| {
                        let changes = GitChanges::from_status(&status);
                        (status, changes)
                    })
                })
                .await;

            let (st, changes) = match status_res {
                Ok(result) => result,
                Err(e) => {
                    if let Some(this_ent) = this.upgrade() {
                        this_ent.update(cx, |git, cx| {
                            git.error = Some(e.to_string());
                            cx.notify();
                        });
                    }
                    return;
                }
            };

            let diff_result = if show_modal {
                // Keep the selected file if it still has pending changes; the diff
                // mode follows its current category (working tree takes priority).
                let picked_file = selected_file
                    .and_then(|(path, _)| changes.get(&path))
                    .or_else(|| changes.rows.first());

                if let Some(file) = picked_file {
                    let path = file.path.clone();
                    let staged = !file.has_worktree_changes;
                    let ws_diff = ws_path.clone();
                    let p = path.clone();
                    let diff_content = cx
                        .background_executor()
                        .spawn(async move {
                            match zest_core::git::git_diff(&ws_diff, Some(&p), staged) {
                                Ok(d) => Some(d),
                                Err(e) => Some(format!("Error loading diff: {}", e)),
                            }
                        })
                        .await;
                    Some((Some((path, staged)), diff_content))
                } else {
                    Some((None, None))
                }
            } else {
                None
            };

            let branches_res = if let Some(this_ent) = this.upgrade() {
                let need_branches = this_ent.read_with(cx, |this, _| this.branches.is_empty());
                if need_branches {
                    let ws_b = ws_path.clone();
                    cx.background_executor()
                        .spawn(async move { zest_core::git::git_branches(&ws_b).ok() })
                        .await
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(this_ent) = this.upgrade() {
                this_ent.update(cx, |git, cx| {
                    if st.is_repo && !st.branch.is_empty() {
                        git.branch = Some(st.branch.clone());
                    }
                    git.modal
                        .selected_commit_files
                        .retain(|path| changes.by_path.contains_key(path));

                    if let Some((file_sel, diff_content)) = diff_result {
                        git.modal.selected_file = file_sel;
                        git.modal.diff_content = diff_content;
                    }

                    if let Some(branches) = branches_res {
                        git.branches = branches;
                    }
                    git.status = Some(st);
                    git.changes = changes;
                    git.status_version = git.status_version.wrapping_add(1);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn load_diff(&mut self, file_path: &str, staged: bool, cx: &mut Context<Self>) {
        let ws_path = self.repo_path.clone();
        let path = file_path.to_string();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_diff(&ws_path, Some(&path), staged) })
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |git, cx| {
                    match res {
                        Ok(d) => git.modal.diff_content = Some(d),
                        Err(e) => {
                            git.modal.diff_content = Some(format!("Error loading diff: {}", e))
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn refresh_history(&mut self, cx: &mut Context<Self>) {
        let ws_path = self.repo_path.clone();
        let needs_first = self.modal.selected_commit.is_none();
        cx.spawn(async move |this, cx| {
            let ws_hist = ws_path.clone();
            let commits = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_history(&ws_hist, 50).ok() })
                .await;
            if let Some(commits) = commits {
                let first_detail = if needs_first {
                    if let Some(first) = commits.first() {
                        let ws_p = ws_path.clone();
                        let h = first.hash.clone();
                        cx.background_executor()
                            .spawn(async move { zest_core::git::git_commit_detail(&ws_p, &h).ok() })
                            .await
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(this) = this.upgrade() {
                    this.update(cx, |git, cx| {
                        if let Some(detail) = first_detail {
                            git.modal.selected_commit = Some(detail);
                        }
                        git.history = commits;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn refresh_branches(&mut self, cx: &mut Context<Self>) {
        let ws_path = self.repo_path.clone();
        cx.spawn(async move |this, cx| {
            let branches = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_branches(&ws_path).ok() })
                .await;
            if let Some(branches) = branches {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |git, cx| {
                        git.branches = branches;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn select_file(&mut self, path: String, staged: bool, cx: &mut Context<Self>) {
        self.modal.selected_file = Some((path.clone(), staged));
        self.load_diff(&path, staged, cx);
    }

    pub fn select_commit(&mut self, hash: &str, cx: &mut Context<Self>) {
        self.modal.expanded_history_files.clear();
        let ws_path = self.repo_path.clone();
        let h = hash.to_string();
        cx.spawn(async move |this, cx| {
            let detail = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_commit_detail(&ws_path, &h).ok() })
                .await;
            if let Some(detail) = detail {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |git, cx| {
                        git.modal.selected_commit = Some(detail);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
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

    pub fn changes(&self) -> &[UnifiedChangeItem] {
        &self.changes.rows
    }

    pub fn change(&self, path: &str) -> Option<&UnifiedChangeItem> {
        self.changes.get(path)
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

#[cfg(all(test, debug_assertions))]
#[path = "git_state_tests.rs"]
mod tests;
