use crate::state::{GitState, ToastState};
use gpui::{App, Context, Entity};

/// Command router for Git VCS operations.
///
/// Stateless holder of the two handles git commands need: the state entity
/// (status cache, reentrancy guards, UI selections) and the toast entity
/// (user-facing feedback). Cheap to clone; construct at point of use.
///
/// State-side transitions (modal/tab switching, refreshes, selections) stay on
/// [`GitState`]; this service only dispatches git CLI commands on the
/// background executor and routes their results back into the entities.
#[derive(Clone)]
pub struct GitService {
    git: Entity<GitState>,
    toast: Entity<ToastState>,
}

impl GitService {
    pub fn new(git: Entity<GitState>, toast: Entity<ToastState>) -> Self {
        Self { git, toast }
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
        cx.spawn(async move |cx| {
            let res = cx
                .background_executor()
                .spawn(async move { command() })
                .await;
            cx.update(|cx| {
                git.update(cx, |git, cx| {
                    git.loading = false;
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
        op: impl FnOnce() -> Result<String, String> + Send + 'static,
        on_success: impl FnOnce(&mut GitState, &mut Context<GitState>, String) + Send + 'static,
    ) {
        if self.git.read(cx).is_syncing {
            return;
        }
        self.git.update(cx, |git, cx| {
            git.is_syncing = true;
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let toast = self.toast.clone();
        cx.spawn(async move |cx| {
            let res = cx.background_executor().spawn(async move { op() }).await;
            cx.update(|cx| {
                git.update(cx, |git, cx| {
                    git.is_syncing = false;
                    match res {
                        Ok(msg) => {
                            git.error = None;
                            on_success(git, cx, msg);
                            cx.notify();
                        }
                        Err(e) => {
                            toast.update(cx, |ts, cx| {
                                ts.push_error(error_title, e.clone(), cx);
                            });
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
        let toast = self.toast.clone();
        cx.spawn(async move |cx| {
            let p_task = p.clone();
            let res = cx
                .background_executor()
                .spawn(async move {
                    zest_core::git::git_discard_file(&ws_path, &p_task, delete_untracked)
                })
                .await;
            cx.update(|cx| {
                let success = res.is_ok();
                git.update(cx, |git, cx| {
                    git.loading = false;
                    match res {
                        Ok(()) => {
                            git.error = None;
                            if let Some((sel, _)) = &git.selected_file {
                                if sel == &p {
                                    git.selected_file = None;
                                    git.diff_content = None;
                                }
                            }
                            git.refresh_status(cx);
                            cx.notify();
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            git.error = Some(err_str.clone());
                            toast.update(cx, |ts, cx| {
                                ts.push_error("Git Discard Failed", err_str, cx);
                            });
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
            .selected_commit_files
            .iter()
            .cloned()
            .collect();
        if paths.is_empty() {
            return;
        }
        let and_push = self.git.read(cx).commit_and_push;
        self.git.update(cx, |git, cx| {
            git.loading = true;
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let toast = self.toast.clone();
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
                git.update(cx, |git, cx| {
                    git.loading = false;
                    match res {
                        Ok(_) => {
                            git.error = None;
                            git.selected_file = None;
                            git.diff_content = None;
                            git.selected_commit_files.clear();
                            git.commit_and_push = false;
                            git.close_modal(cx);
                            git.refresh_all(cx);
                            cx.notify();
                            if let Some(push_res) = push_res {
                                match push_res {
                                    Ok(msg) => {
                                        let trimmed = msg.trim();
                                        toast.update(cx, |ts, cx| {
                                            ts.push_success(
                                                "Git Commit",
                                                if trimmed.is_empty() {
                                                    "Committed and pushed to remote.".to_string()
                                                } else {
                                                    trimmed.to_string()
                                                },
                                                cx,
                                            );
                                        });
                                    }
                                    Err(e) => {
                                        toast.update(cx, |ts, cx| {
                                            ts.push_error("Git Push Failed", e.to_string(), cx);
                                        });
                                    }
                                }
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
            git.error = None;
            cx.notify();
        });

        let git = self.git.clone();
        let toast = self.toast.clone();
        cx.spawn(async move |cx| {
            let b_task = b.clone();
            let res = cx
                .background_executor()
                .spawn(async move { zest_core::git::git_checkout(&ws_path, &b_task) })
                .await;
            cx.update(|cx| {
                git.update(cx, |git, cx| {
                    git.loading = false;
                    match res {
                        Ok(()) => {
                            git.branch = Some(b.clone());
                            git.error = None;
                            git.refresh_all(cx);
                            cx.notify();
                            toast.update(cx, |ts, cx| {
                                ts.push_success(
                                    "Git Checkout",
                                    format!("Switched to branch '{}'", b),
                                    cx,
                                );
                            });
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            git.error = Some(err_str.clone());
                            toast.update(cx, |ts, cx| {
                                ts.push_error("Git Checkout Failed", err_str, cx);
                            });
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
        let toast = self.toast.clone();
        self.run_sync_op(
            cx,
            "Git Fetch Failed",
            move || zest_core::git::git_fetch(&ws_path).map_err(|e| e.to_string()),
            move |git, cx, msg| {
                git.refresh_all(cx);
                let trimmed = msg.trim();
                toast.update(cx, |ts, cx| {
                    ts.push_success(
                        "Git Fetch",
                        if trimmed.is_empty() {
                            "Fetched latest references.".to_string()
                        } else {
                            trimmed.to_string()
                        },
                        cx,
                    );
                });
            },
        );
    }

    pub fn pull(&self, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        let toast = self.toast.clone();
        self.run_sync_op(
            cx,
            "Git Pull Failed",
            move || zest_core::git::git_pull(&ws_path).map_err(|e| e.to_string()),
            move |git, cx, msg| {
                git.refresh_all(cx);
                let trimmed = msg.trim();
                toast.update(cx, |ts, cx| {
                    if trimmed.contains("Already up to date") {
                        ts.push_info("Git Sync", "Already up to date.", cx);
                    } else {
                        ts.push_success(
                            "Git Pull",
                            format!("Successfully pulled: {}", trimmed),
                            cx,
                        );
                    }
                });
            },
        );
    }

    pub fn push(&self, cx: &mut App) {
        let ws_path = self.git.read(cx).repo_path.clone();
        let toast = self.toast.clone();
        self.run_sync_op(
            cx,
            "Git Push Failed",
            move || zest_core::git::git_push(&ws_path).map_err(|e| e.to_string()),
            move |git, cx, msg| {
                git.refresh_status(cx);
                let trimmed = msg.trim();
                toast.update(cx, |ts, cx| {
                    ts.push_success(
                        "Git Push",
                        if trimmed.is_empty() {
                            "Pushed commits to remote.".to_string()
                        } else {
                            trimmed.to_string()
                        },
                        cx,
                    );
                });
            },
        );
    }
}
