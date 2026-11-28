use super::{working_tree::GitChanges, GitState};
use gpui_kit::Context;

impl GitState {
    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        self.refresh_status(cx);
        self.refresh_history(cx);
        self.refresh_branches(cx);
        self.refresh_remotes(cx);
    }

    pub fn refresh_remotes(&mut self, cx: &mut Context<Self>) {
        self.queries.remotes = self.queries.remotes.wrapping_add(1);
        let version = (self.queries.repo, self.queries.remotes);
        let ws_path = self.repo_path.clone();
        cx.spawn(async move |this, cx| {
            let remotes = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_get_remotes(&ws_path).ok() })
                .await;
            if let Some(remotes) = remotes {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |git, cx| {
                        if version != (git.queries.repo, git.queries.remotes) {
                            return;
                        }
                        git.remotes = remotes;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn refresh_status(&mut self, cx: &mut Context<Self>) {
        self.queries.status = self.queries.status.wrapping_add(1);
        let status_version = (self.queries.repo, self.queries.status);
        let diff_version = self.queries.diff;
        let ws_path = self.repo_path.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    zest_core::git::git_status(&ws_path).map(|status| {
                        let changes = GitChanges::from_status(&status);
                        (status, changes)
                    })
                })
                .await;

            let _ = this.update(cx, |git, cx| {
                if status_version != (git.queries.repo, git.queries.status) {
                    return;
                }

                let (status, changes) = match result {
                    Ok(result) => result,
                    Err(error) => {
                        git.operation.set_error(error.to_string());
                        cx.notify();
                        return;
                    }
                };
                git.modal
                    .selected_commit_files
                    .retain(|path| changes.by_path.contains_key(path));

                if git.modal.is_open && diff_version == git.queries.diff {
                    // Follow the selected file across staging changes and fall
                    // back to the first changed file when it is no longer listed.
                    let selected = git
                        .modal
                        .selected_file
                        .as_ref()
                        .and_then(|(path, _)| changes.get(path))
                        .or_else(|| changes.rows.first());
                    if let Some(file) = selected {
                        git.select_file(file.path.clone(), !file.has_worktree_changes, cx);
                    } else {
                        git.clear_file_selection();
                    }
                }

                git.working_tree.refresh(status, changes);
                cx.notify();
            });
        })
        .detach();
    }

    fn load_diff(&mut self, file_path: &str, staged: bool, cx: &mut Context<Self>) {
        self.queries.diff = self.queries.diff.wrapping_add(1);
        let version = (self.queries.repo, self.queries.diff);
        cx.notify();
        let ws_path = self.repo_path.clone();
        let path = file_path.to_string();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_diff(&ws_path, Some(&path), staged) })
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |git, cx| {
                    if version != (git.queries.repo, git.queries.diff) {
                        return;
                    }
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
        self.queries.history = self.queries.history.wrapping_add(1);
        let version = (self.queries.repo, self.queries.history);
        let commit_version = self.queries.commit;
        let ws_path = self.repo_path.clone();
        let selected = self
            .modal
            .selected_commit
            .as_ref()
            .map(|detail| detail.commit.hash.clone());
        cx.spawn(async move |this, cx| {
            let ws_hist = ws_path.clone();
            let commits = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_history(&ws_hist, 50).ok() })
                .await;
            if let Some(commits) = commits {
                let needs_first = !selected
                    .as_ref()
                    .is_some_and(|hash| commits.iter().any(|commit| &commit.hash == hash));
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
                        if version != (git.queries.repo, git.queries.history) {
                            return;
                        }
                        if needs_first && commit_version == git.queries.commit {
                            git.modal.selected_commit = first_detail;
                            git.modal.expanded_history_files.clear();
                        } else if git.modal.selected_commit.as_ref().is_some_and(|detail| {
                            !commits
                                .iter()
                                .any(|commit| commit.hash == detail.commit.hash)
                        }) {
                            git.modal.selected_commit = None;
                            git.modal.expanded_history_files.clear();
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
        self.queries.branches = self.queries.branches.wrapping_add(1);
        let version = (self.queries.repo, self.queries.branches);
        let ws_path = self.repo_path.clone();
        cx.spawn(async move |this, cx| {
            let branches = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_branches(&ws_path).ok() })
                .await;
            if let Some(branches) = branches {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |git, cx| {
                        if version != (git.queries.repo, git.queries.branches) {
                            return;
                        }
                        git.branches = branches;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn select_file(&mut self, path: String, staged: bool, cx: &mut Context<Self>) {
        let selected = (path.clone(), staged);
        if self.modal.selected_file.as_ref() != Some(&selected) {
            self.modal.diff_content = None;
            self.modal.selected_file = Some(selected);
        }
        self.load_diff(&path, staged, cx);
    }

    pub(crate) fn clear_file_selection(&mut self) {
        self.queries.diff = self.queries.diff.wrapping_add(1);
        self.modal.selected_file = None;
        self.modal.diff_content = None;
    }

    pub fn select_commit(&mut self, hash: &str, cx: &mut Context<Self>) {
        self.queries.commit = self.queries.commit.wrapping_add(1);
        let version = (self.queries.repo, self.queries.commit);
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
                        if version != (git.queries.repo, git.queries.commit) {
                            return;
                        }
                        if git
                            .history
                            .iter()
                            .any(|commit| commit.hash == detail.commit.hash)
                        {
                            git.modal.selected_commit = Some(detail);
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
    }
}
