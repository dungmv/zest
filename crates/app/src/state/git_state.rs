use gpui::Context;
use std::collections::HashSet;
use std::path::PathBuf;

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

#[derive(Debug, Clone)]
pub struct GitState {
    pub repo_path: PathBuf,
    pub branch: Option<String>,
    pub tracked_warning: bool,
    pub show_modal: bool,
    pub active_modal: Option<GitActiveModal>,
    pub tab: GitTab,
    pub status: Option<zest_core::git::GitStatusResult>,
    pub history: Vec<zest_core::git::GitCommitInfo>,
    pub branches: Vec<zest_core::git::GitBranchInfo>,
    pub remotes: Vec<zest_core::git::GitRemote>,
    pub selected_commit_files: HashSet<String>,
    pub commit_and_push: bool,
    pub selected_file: Option<(String, bool)>,
    pub diff_content: Option<String>,
    pub selected_commit: Option<zest_core::git::GitCommitDetail>,
    pub expanded_history_files: HashSet<String>,
    pub error: Option<String>,
    pub loading: bool,
    pub is_syncing: bool,
}

impl Default for GitState {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::new(),
            branch: None,
            tracked_warning: false,
            show_modal: false,
            active_modal: None,
            tab: GitTab::Changes,
            status: None,
            history: Vec::new(),
            branches: Vec::new(),
            remotes: Vec::new(),
            selected_commit_files: HashSet::new(),
            commit_and_push: false,
            selected_file: None,
            diff_content: None,
            selected_commit: None,
            expanded_history_files: HashSet::new(),
            error: None,
            loading: false,
            is_syncing: false,
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
                    Some(zest_core::git::GitStatusResult {
                        is_repo: true,
                        branch: branch_name,
                        ..Default::default()
                    }),
                )
            }
            None => (
                None,
                Some(zest_core::git::GitStatusResult {
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
                self.status = Some(zest_core::git::GitStatusResult {
                    is_repo: true,
                    branch: branch_name,
                    ..Default::default()
                });
            }
            None => {
                self.branch = None;
                self.status = Some(zest_core::git::GitStatusResult {
                    is_repo: false,
                    ..Default::default()
                });
            }
        }
        self.refresh_all(cx);
    }

    pub fn open_modal(&mut self, modal: GitActiveModal, cx: &mut Context<Self>) {
        self.active_modal = Some(modal);
        self.show_modal = true;
        self.error = None;
        match modal {
            GitActiveModal::Commit => {
                self.selected_commit_files.clear();
                self.commit_and_push = false;
                self.refresh_status(cx);
            }
            GitActiveModal::History => {
                self.expanded_history_files.clear();
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
        if self.show_modal || self.active_modal.is_some() {
            self.active_modal = None;
            self.show_modal = false;
            self.error = None;
            self.selected_commit_files.clear();
            self.commit_and_push = false;
            self.expanded_history_files.clear();
            cx.notify();
            true
        } else {
            false
        }
    }

    pub fn toggle_modal(&mut self, cx: &mut Context<Self>) {
        if self.show_modal || self.active_modal.is_some() {
            self.close_modal(cx);
        } else {
            self.open_modal(GitActiveModal::Commit, cx);
        }
    }

    pub fn set_tab(&mut self, tab: GitTab, cx: &mut Context<Self>) {
        self.tab = tab;
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
        let show_modal = self.show_modal;
        let selected_file = self.selected_file.clone();

        cx.spawn(async move |this, cx| {
            let ws_status = ws_path.clone();
            let status_res = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_status(&ws_status) })
                .await;

            let st = match status_res {
                Ok(st) => st,
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
                let staged_paths: HashSet<String> =
                    st.staged_files.iter().map(|f| f.path.clone()).collect();
                let worktree_paths: HashSet<String> = st
                    .unstaged_files
                    .iter()
                    .chain(&st.untracked_files)
                    .map(|f| f.path.clone())
                    .collect();

                // Keep the selected file if it still has pending changes; the diff
                // mode follows its current category (working tree takes priority).
                let picked_path = selected_file
                    .filter(|(path, _)| {
                        staged_paths.contains(path) || worktree_paths.contains(path)
                    })
                    .map(|(path, _)| path)
                    .or_else(|| st.staged_files.first().map(|f| f.path.clone()))
                    .or_else(|| st.unstaged_files.first().map(|f| f.path.clone()))
                    .or_else(|| st.untracked_files.first().map(|f| f.path.clone()));

                if let Some(path) = picked_path {
                    let staged = !worktree_paths.contains(&path);
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
                    let valid_paths: HashSet<String> = st
                        .staged_files
                        .iter()
                        .chain(&st.unstaged_files)
                        .chain(&st.untracked_files)
                        .map(|f| f.path.clone())
                        .collect();
                    git.selected_commit_files
                        .retain(|p| valid_paths.contains(p));

                    if let Some((file_sel, diff_content)) = diff_result {
                        git.selected_file = file_sel;
                        git.diff_content = diff_content;
                    }

                    if let Some(branches) = branches_res {
                        git.branches = branches;
                    }
                    git.status = Some(st);
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
                        Ok(d) => git.diff_content = Some(d),
                        Err(e) => git.diff_content = Some(format!("Error loading diff: {}", e)),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn refresh_history(&mut self, cx: &mut Context<Self>) {
        let ws_path = self.repo_path.clone();
        let needs_first = self.selected_commit.is_none();
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
                            git.selected_commit = Some(detail);
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
        self.selected_file = Some((path.clone(), staged));
        self.load_diff(&path, staged, cx);
    }

    pub fn select_commit(&mut self, hash: &str, cx: &mut Context<Self>) {
        self.expanded_history_files.clear();
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
                        git.selected_commit = Some(detail);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn toggle_history_file_expansion(&mut self, file_path: &str, cx: &mut Context<Self>) {
        if self.expanded_history_files.contains(file_path) {
            self.expanded_history_files.remove(file_path);
        } else {
            self.expanded_history_files.insert(file_path.to_string());
        }
        cx.notify();
    }

    pub fn toggle_file_selection(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.selected_commit_files.contains(path) {
            self.selected_commit_files.remove(path);
        } else {
            self.selected_commit_files.insert(path.to_string());
        }
        cx.notify();
    }

    pub fn toggle_all_files_selection(&mut self, select: bool, cx: &mut Context<Self>) {
        if select {
            if let Some(st) = &self.status {
                self.selected_commit_files = st
                    .staged_files
                    .iter()
                    .chain(&st.unstaged_files)
                    .chain(&st.untracked_files)
                    .map(|f| f.path.clone())
                    .collect();
            }
        } else {
            self.selected_commit_files.clear();
        }
        cx.notify();
    }

    pub fn set_commit_and_push(&mut self, value: bool, cx: &mut Context<Self>) {
        self.commit_and_push = value;
        cx.notify();
    }
}
