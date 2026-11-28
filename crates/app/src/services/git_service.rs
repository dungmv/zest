use crate::state::{GitState, ModalKind, ModalState, WorkspaceState};
use gpui_kit::{App, Context, Entity};
use std::path::PathBuf;
use zest_core::git::{clone_workspace, CloneCancellation};

/// Command router for Git VCS operations.
///
/// Stateless holder of the git state entity (status cache, reentrancy guards,
/// UI selections). Cheap to clone; construct at point of use.
///
/// State-side transitions (modal/tab switching, refreshes, selections) stay on
/// [`GitState`]; this service only dispatches git CLI commands on the
/// background executor and routes their results back into the entity.
#[derive(Clone)]
pub struct GitService {
    git: Entity<GitState>,
}

impl GitService {
    pub fn new(git: Entity<GitState>) -> Self {
        Self { git }
    }

    pub fn clone_and_open_workspace(
        workspace: Entity<WorkspaceState>,
        modal: Entity<ModalState>,
        source: String,
        parent_folder: PathBuf,
        cx: &mut App,
    ) {
        let cancellation = CloneCancellation::default();
        let started = modal.update(cx, |state, cx| {
            if state.active_modal != Some(ModalKind::OpenGitWorkspace)
                || state.open_git_workspace.operation.is_some()
            {
                return false;
            }
            state.open_git_workspace.error = None;
            state.open_git_workspace.operation = Some(cancellation.clone());
            cx.notify();
            true
        });
        if !started {
            return;
        }

        cx.spawn(async move |cx| {
            let result = cx
                .background_executor()
                .spawn(async move { clone_workspace(&source, &parent_folder, &cancellation) })
                .await;
            cx.update(|cx| {
                let path = modal.update(cx, |state, cx| {
                    state.open_git_workspace.operation = None;
                    match result {
                        Ok(path) => {
                            state.close(ModalKind::OpenGitWorkspace, cx);
                            Some(path)
                        }
                        Err(error) => {
                            state.open_git_workspace.error = Some(error.to_string());
                            cx.notify();
                            None
                        }
                    }
                });
                if let Some(path) = path {
                    workspace.update(cx, |state, cx| state.open_cloned_workspace(path, cx));
                }
            });
        })
        .detach();
    }

    /// Dispatch a `loading`-guarded command. Errors surface in
    /// `GitState::error`; success applies `on_success` on the main thread.
    fn run_command(
        &self,
        cx: &mut App,
        command: impl FnOnce() -> Result<(), String> + Send + 'static,
        on_success: impl FnOnce(&mut GitState, &mut Context<GitState>) + Send + 'static,
    ) {
        if self.git.read(cx).loading {
            return;
        }
        self.git.update(cx, |git, cx| {
            git.loading = true;
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let generation = git.read(cx).repo_generation();
        cx.spawn(async move |cx| {
            let res = cx
                .background_executor()
                .spawn(async move { command() })
                .await;
            cx.update(|cx| {
                if git.read(cx).repo_generation() != generation {
                    return;
                }
                git.update(cx, |git, cx| {
                    git.loading = false;
                    git.is_syncing = false;
                    git.sync_message = None;
                    match res {
                        Ok(()) => {
                            git.error = None;
                            on_success(git, cx);
                            cx.notify();
                        }
                        Err(e) => {
                            git.error = Some(e);
                            cx.notify();
                        }
                    }
                });
            })
        })
        .detach();
    }

    /// Dispatch an `is_syncing`-guarded network operation. Errors surface both
    /// in `GitState::error` and as a toast titled `error_title`; success is
    /// delegated to `on_success` with the command's output message.
    fn run_sync_op(
        &self,
        cx: &mut App,
        error_title: &'static str,
        op_label: &'static str,
        op: impl FnOnce() -> Result<String, String> + Send + 'static,
        on_success: impl FnOnce(&mut GitState, &mut Context<GitState>, String) + Send + 'static,
    ) {
        if self.git.read(cx).is_syncing {
            return;
        }
        self.git.update(cx, |git, cx| {
            git.is_syncing = true;
            git.sync_message = Some(op_label.to_string());
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let generation = git.read(cx).repo_generation();
        cx.spawn(async move |cx| {
            let res = cx.background_executor().spawn(async move { op() }).await;
            cx.update(|cx| {
                if git.read(cx).repo_generation() != generation {
                    return;
                }
                git.update(cx, |git, cx| {
                    git.is_syncing = false;
                    git.sync_message = None;
                    match res {
                        Ok(msg) => {
                            git.error = None;
                            on_success(git, cx, msg);
                            cx.notify();
                        }
                        Err(e) => {
                            crate::notify::error(error_title, e.clone(), cx);
                            git.error = Some(e);
                            cx.notify();
                        }
                    }
                });
            })
        })
        .detach();
    }

    pub fn init_repo(&self, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        self.run_command(
            cx,
            move || zest_core::git::git_init(&ws_path).map_err(|e| e.to_string()),
            |git, cx| git.refresh_all(cx),
        );
    }

    pub fn create_branch(&self, name: &str, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        let n = name.to_string();
        let n_task = n.clone();
        self.run_command(
            cx,
            move || zest_core::git::git_create_branch(&ws_path, &n_task).map_err(|e| e.to_string()),
            move |git, cx| {
                git.branch = Some(n);
                git.close_modal(cx);
                git.refresh_all(cx);
            },
        );
    }

    pub fn add_remote(&self, name: &str, url: &str, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        let n = name.to_string();
        let u = url.to_string();
        self.run_command(
            cx,
            move || zest_core::git::git_add_remote(&ws_path, &n, &u).map_err(|e| e.to_string()),
            |git, cx| git.refresh_remotes(cx),
        );
    }

    pub fn remove_remote(&self, name: &str, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        let n = name.to_string();
        self.run_command(
            cx,
            move || zest_core::git::git_remove_remote(&ws_path, &n).map_err(|e| e.to_string()),
            |git, cx| git.refresh_remotes(cx),
        );
    }

    /// Discards changes in a file. `delete_untracked` opts into git-clean
    /// semantics (deleting never-committed files); only the git panel may pass
    /// `true` — sidebar callers pass `false` to keep workspace data intact.
    pub fn discard_file(
        &self,
        path: &str,
        delete_untracked: bool,
        cx: &mut App,
        on_success: impl FnOnce(&mut App) + Send + 'static,
    ) {
        if self.git.read(cx).loading {
            return;
        }
        let ws_path = self.git.read(cx).repo_path.clone();
        let p = path.to_string();
        self.git.update(cx, |git, cx| {
            git.loading = true;
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let generation = git.read(cx).repo_generation();
        cx.spawn(async move |cx| {
            let p_task = p.clone();
            let res = cx
                .background_executor()
                .spawn(async move {
                    zest_core::git::git_discard_file(&ws_path, &p_task, delete_untracked)
                })
                .await;
            cx.update(|cx| {
                if git.read(cx).repo_generation() != generation {
                    return;
                }
                let success = res.is_ok();
                git.update(cx, |git, cx| {
                    git.loading = false;
                    match res {
                        Ok(()) => {
                            git.error = None;
                            if let Some((sel, _)) = &git.modal.selected_file {
                                if sel == &p {
                                    git.clear_file_selection();
                                }
                            }
                            git.refresh_status(cx);
                            cx.notify();
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            git.error = Some(err_str.clone());
                            crate::notify::error("Git Discard Failed", err_str, cx);
                            cx.notify();
                        }
                    }
                });
                if success {
                    on_success(cx);
                }
            })
        })
        .detach();
    }

    pub fn commit(&self, message: &str, cx: &mut App) {
        let (loading, ws_path) = {
            let git = self.git.read(cx);
            (git.loading, git.repo_path.clone())
        };
        if loading {
            return;
        }
        let paths: Vec<String> = self
            .git
            .read(cx)
            .modal
            .selected_commit_files
            .iter()
            .cloned()
            .collect();
        if paths.is_empty() {
            return;
        }
        let and_push = self.git.read(cx).modal.commit_and_push;
        self.git.update(cx, |git, cx| {
            git.loading = true;
            if and_push {
                git.is_syncing = true;
                git.sync_message = Some("pushing...".to_string());
            }
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let generation = git.read(cx).repo_generation();
        let msg = message.to_string();
        cx.spawn(async move |cx| {
            let ws_commit = ws_path.clone();
            let commit_paths = paths.clone();
            let res = cx
                .background_executor()
                .spawn(async move {
                    let str_paths: Vec<&str> = commit_paths.iter().map(|s| s.as_str()).collect();
                    zest_core::git::git_commit_files(&ws_commit, &msg, &str_paths)
                })
                .await;

            let push_res = match (&res, and_push) {
                (Ok(_), true) => {
                    let ws_push = ws_path.clone();
                    Some(
                        cx.background_executor()
                            .spawn(async move { zest_core::git::git_push(&ws_push) })
                            .await,
                    )
                }
                _ => None,
            };

            cx.update(|cx| {
                if git.read(cx).repo_generation() != generation {
                    return;
                }
                git.update(cx, |git, cx| {
                    git.loading = false;
                    git.is_syncing = false;
                    git.sync_message = None;
                    match res {
                        Ok(_) => {
                            git.error = None;
                            git.clear_file_selection();
                            git.modal.selected_commit_files.clear();
                            git.modal.commit_and_push = false;
                            git.close_modal(cx);
                            git.refresh_all(cx);
                            cx.notify();
                            if let Some(Err(e)) = push_res {
                                crate::notify::error("Git Push Failed", e.to_string(), cx);
                            }
                        }
                        Err(e) => {
                            git.error = Some(e.to_string());
                            cx.notify();
                        }
                    }
                });
            })
        })
        .detach();
    }

    pub fn checkout(&self, branch: &str, cx: &mut App) {
        if self.git.read(cx).loading {
            return;
        }
        let ws_path = self.git.read(cx).repo_path.clone();
        let b = branch.to_string();
        self.git.update(cx, |git, cx| {
            git.loading = true;
            git.is_syncing = true;
            git.sync_message = Some("checking out...".to_string());
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let generation = git.read(cx).repo_generation();
        cx.spawn(async move |cx| {
            let b_task = b.clone();
            let res = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_checkout(&ws_path, &b_task) })
                .await;
            cx.update(|cx| {
                if git.read(cx).repo_generation() != generation {
                    return;
                }
                git.update(cx, |git, cx| {
                    git.loading = false;
                    git.is_syncing = false;
                    git.sync_message = None;
                    match res {
                        Ok(()) => {
                            git.branch = Some(b);
                            git.error = None;
                            git.refresh_all(cx);
                            cx.notify();
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            git.error = Some(err_str.clone());
                            crate::notify::error("Git Checkout Failed", err_str, cx);
                            cx.notify();
                        }
                    }
                });
            })
        })
        .detach();
    }

    pub fn fetch(&self, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        self.run_sync_op(
            cx,
            "Git Fetch Failed",
            "fetching...",
            move || zest_core::git::git_fetch(&ws_path).map_err(|e| e.to_string()),
            move |git, cx, _msg| {
                git.refresh_all(cx);
            },
        );
    }

    pub fn pull(&self, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        self.run_sync_op(
            cx,
            "Git Pull Failed",
            "pulling...",
            move || zest_core::git::git_pull(&ws_path).map_err(|e| e.to_string()),
            move |git, cx, _msg| {
                git.refresh_all(cx);
            },
        );
    }

    pub fn push(&self, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        self.run_sync_op(
            cx,
            "Git Push Failed",
            "pushing...",
            move || zest_core::git::git_push(&ws_path).map_err(|e| e.to_string()),
            move |git, cx, _msg| {
                git.refresh_status(cx);
            },
        );
    }
}
