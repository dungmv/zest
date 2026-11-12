use super::{
    EnvironmentState, GitState, PersistTarget, PersistenceState, WorkspaceDataState, WorkspaceEvent,
};
use gpui_kit::{Context, Entity};
use std::collections::HashSet;
use std::time::Duration;
use zest_core::storage::FsEvent;

/// File-watcher plumbing and on-disk change reconciliation of
/// [`PersistenceState`], split from `persistence_state.rs` to keep each file
/// under the 650 LOC budget.
impl PersistenceState {
    pub fn start_watcher(
        &mut self,
        workspace_data: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        git_state: Entity<GitState>,
        cx: &mut Context<Self>,
    ) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FsEvent>();
        let fp = self.store.disk_fingerprints();
        if let Ok(watcher) =
            zest_core::storage::start_workspace_watcher(&self.workspace_path, fp, tx)
        {
            self.watcher = Some(watcher);
            let ws_clone = workspace_data.clone();
            let env_clone = env_state.clone();
            let git_clone = git_state.clone();
            let task = cx.spawn(async move |this, cx| {
                while let Some(event) = rx.recv().await {
                    let mut batch = vec![event];
                    while let Ok(ev) = rx.try_recv() {
                        batch.push(ev);
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    while let Ok(ev) = rx.try_recv() {
                        batch.push(ev);
                    }

                    if let Some(this) = this.upgrade() {
                        let ws = ws_clone.clone();
                        let env = env_clone.clone();
                        let git = git_clone.clone();
                        this.update(cx, |p, cx| {
                            p.handle_fs_events_batch(batch, ws, env, git, cx);
                        });
                    }
                }
            });
            self.watcher_task = Some(task);
        }
    }

    #[allow(dead_code)] // Single fs event handler for testing
    pub fn handle_fs_event(
        &mut self,
        event: FsEvent,
        workspace_data: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        git_state: Entity<GitState>,
        cx: &mut Context<Self>,
    ) {
        self.handle_fs_events_batch(vec![event], workspace_data, env_state, git_state, cx);
    }

    pub fn handle_fs_events_batch(
        &mut self,
        events: Vec<FsEvent>,
        workspace_data: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        git_state: Entity<GitState>,
        cx: &mut Context<Self>,
    ) {
        let mut git_changed = false;
        let mut tree_changed = false;
        let mut changed_envs = HashSet::new();
        let mut req_changed = HashSet::new();
        let mut req_deleted = HashSet::new();

        for event in events {
            match event {
                FsEvent::GitChanged => git_changed = true,
                FsEvent::TreeChanged => tree_changed = true,
                FsEvent::EnvironmentChanged(name) => {
                    changed_envs.insert(name);
                }
                FsEvent::RequestDeleted(rel_path) => {
                    req_deleted.insert(rel_path.clone());
                    req_changed.remove(&rel_path);
                }
                FsEvent::RequestChanged(rel_path) => {
                    if !req_deleted.contains(&rel_path) {
                        req_changed.insert(rel_path);
                    }
                }
            }
        }

        // Deletion events are verified against the filesystem: `git restore`
        // and editors replace files via remove+recreate, so the file may exist
        // again by the time the batch is handled.
        let mut verified_deleted = HashSet::new();
        for rel_path in &req_deleted {
            if self
                .workspace_path
                .join("collections")
                .join(rel_path)
                .is_file()
            {
                tree_changed = true;
                req_changed.insert(rel_path.clone());
            } else {
                verified_deleted.insert(rel_path.clone());
            }
        }
        let req_deleted = verified_deleted;

        let mut self_changed = false;

        for rel_path in &req_changed {
            if self.is_dirty(rel_path) {
                self.conflicts.insert(
                    rel_path.clone(),
                    "File was modified externally while you have unsaved changes.".to_string(),
                );
                self_changed = true;
            }
        }

        for rel_path in &req_deleted {
            let target = PersistTarget::Request(rel_path.clone());
            self.coordinator.cancel_target(&target);
            self.debounce_tasks.remove(&target);
            if self.unsaved_requests.remove(rel_path) {
                self.unsaved_version = self.unsaved_version.wrapping_add(1);
                self_changed = true;
            }
            if self.conflicts.remove(rel_path).is_some() {
                self_changed = true;
            }
        }

        let mut dirty_requests = self.unsaved_requests.clone();
        let mut dirty_envs = HashSet::new();
        for target in self.coordinator.dirty_targets() {
            match target {
                PersistTarget::Request(id) => {
                    dirty_requests.insert(id);
                }
                PersistTarget::Environment(name) => {
                    dirty_envs.insert(name);
                }
                PersistTarget::Globals => {
                    dirty_envs.insert("globals".to_string());
                }
                _ => {}
            }
        }

        if tree_changed {
            workspace_data.update(cx, |ws, cx| {
                ws.reload_tree_preserving(&dirty_requests);
                cx.emit(WorkspaceEvent::RequestsReloaded { req_ids: None });
                cx.notify();
            });
        } else {
            let mut reqs_to_update = Vec::new();
            let mut parse_errors = Vec::new();

            for rel_path in &req_changed {
                if !self.is_dirty(rel_path) {
                    match self.store.read_request(rel_path) {
                        Ok(req_file) => reqs_to_update.push((rel_path.clone(), req_file)),
                        Err(e) => parse_errors.push((rel_path.clone(), e.to_string())),
                    }
                }
            }

            if !reqs_to_update.is_empty() || !parse_errors.is_empty() || !req_deleted.is_empty() {
                workspace_data.update(cx, |ws, cx| {
                    for (path, err) in parse_errors {
                        ws.parse_errors.insert(path, err);
                    }

                    let mut reloaded_ids = Vec::with_capacity(reqs_to_update.len());
                    let mut needs_rescan = false;
                    for (path, req_file) in reqs_to_update {
                        ws.parse_errors.remove(&path);
                        if let Some(r) = ws.requests.iter_mut().find(|r| r.id == path) {
                            let ws_name = ws.workspace_meta.name.clone();
                            let prev_status = r.last_status_code.or_else(|| {
                                ws.store
                                    .load_response_cache(&r.id)
                                    .ok()
                                    .flatten()
                                    .map(|l| l.response.status)
                            });
                            *r = req_file.into_item(&r.id, &ws_name, r.folder_id.clone());
                            r.last_status_code = prev_status;
                            reloaded_ids.push(path);
                        } else {
                            needs_rescan = true;
                        }
                    }

                    for rel_path in &req_deleted {
                        ws.parse_errors.remove(rel_path);
                        ws.requests.retain(|r| &r.id != rel_path);
                    }

                    ws.bump_tree_version();
                    if needs_rescan {
                        ws.reload_tree_preserving(&dirty_requests);
                        cx.emit(WorkspaceEvent::RequestsReloaded { req_ids: None });
                    } else if !reloaded_ids.is_empty() {
                        cx.emit(WorkspaceEvent::RequestsReloaded {
                            req_ids: Some(reloaded_ids),
                        });
                    }
                    cx.notify();
                });
            }
        }

        let env_changed = !changed_envs.is_empty();
        if env_changed {
            let new_conflicts = env_state.update(cx, |env, cx| {
                let c = env.apply_disk_changes(&changed_envs, &dirty_envs, cx);
                cx.notify();
                c
            });
            for env_name in new_conflicts {
                self.env_conflicts.insert(
                    env_name,
                    "Environment file was modified externally while you have unsaved changes."
                        .to_string(),
                );
                self_changed = true;
            }
        }

        if self_changed {
            cx.notify();
        }

        let working_tree_changed =
            tree_changed || env_changed || !req_changed.is_empty() || !req_deleted.is_empty();
        if git_changed || working_tree_changed {
            git_state.update(cx, |git, cx| git.refresh_status(cx));
        }
    }
}
