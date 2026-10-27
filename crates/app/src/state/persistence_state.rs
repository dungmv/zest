use super::{
    EnvironmentState, GitState, PersistPayload, PersistTarget, PersistenceCoordinator,
    TargetStatus, WorkspaceDataState, WorkspaceEvent,
};
use gpui::{Context, Entity, Task};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use zest_core::models::Environment;
use zest_core::storage::models::{FolderFile, LocalState, WorkspaceMeta};
use zest_core::storage::{FsEvent, WorkspaceStore, WorkspaceWatcher};

struct WriteCompletion {
    target: PersistTarget,
    gen: u64,
    revision: u64,
    payload: PersistPayload,
    debounce: Duration,
    result: anyhow::Result<()>,
}

pub struct PersistenceState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_path: PathBuf,
    pub local_state: LocalState,
    pub conflicts: HashMap<String, String>,
    pub unsaved_requests: HashSet<String>,
    pub unsaved_version: usize,
    pub coordinator: PersistenceCoordinator,
    pub debounce_tasks: HashMap<PersistTarget, Task<()>>,
    pub watcher: Option<WorkspaceWatcher>,
    pub watcher_task: Option<Task<()>>,
}

impl PersistenceState {
    pub fn new(store: Arc<dyn WorkspaceStore>, workspace_path: PathBuf) -> Self {
        let local_state = store.load_state().unwrap_or_default();
        Self {
            store,
            workspace_path,
            local_state,
            conflicts: HashMap::new(),
            unsaved_requests: HashSet::new(),
            unsaved_version: 0,
            coordinator: PersistenceCoordinator::new(),
            debounce_tasks: HashMap::new(),
            watcher: None,
            watcher_task: None,
        }
    }

    pub fn set_workspace(
        &mut self,
        store: Arc<dyn WorkspaceStore>,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.store = store;
        self.workspace_path = path;
        self.conflicts.clear();
        self.unsaved_requests.clear();
        self.debounce_tasks.clear();
        self.coordinator.bump_workspace_generation();
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
        self.local_state = self.store.load_state().unwrap_or_default();
        cx.notify();
    }

    pub fn schedule_state_save(&mut self, cx: &mut Context<Self>) {
        let target = PersistTarget::LocalState;
        let payload = PersistPayload::LocalState(Box::new(self.local_state.clone()));
        self.enqueue_write(target, payload, Duration::from_millis(1500), cx);
    }

    pub fn set_active_request(&mut self, req_id: Option<String>, cx: &mut Context<Self>) {
        self.local_state.active_request = req_id;
        self.schedule_state_save(cx);
        cx.notify();
    }

    pub fn set_active_environment(&mut self, env_id: Option<String>, cx: &mut Context<Self>) {
        self.local_state.active_environment = env_id;
        self.schedule_state_save(cx);
        cx.notify();
    }

    pub fn mark_dirty(&mut self, req_id: String, cx: &mut Context<Self>) {
        if self.unsaved_requests.insert(req_id) {
            self.unsaved_version = self.unsaved_version.wrapping_add(1);
            cx.notify();
        }
    }

    pub fn mark_clean(&mut self, req_id: &str, cx: &mut Context<Self>) {
        let target = PersistTarget::Request(req_id.to_string());
        self.coordinator.cancel_target(&target);
        self.debounce_tasks.remove(&target);
        if self.unsaved_requests.remove(req_id) {
            self.unsaved_version = self.unsaved_version.wrapping_add(1);
            cx.notify();
        }
    }

    pub fn is_dirty(&self, req_id: &str) -> bool {
        self.unsaved_requests.contains(req_id)
            || self
                .coordinator
                .is_dirty(&PersistTarget::Request(req_id.to_string()))
    }

    pub fn target_status(&self, target: &PersistTarget) -> TargetStatus {
        self.coordinator.status(target)
    }

    pub fn is_target_dirty(&self, target: &PersistTarget) -> bool {
        self.coordinator.is_dirty(target)
    }

    pub fn target_error(&self, target: &PersistTarget) -> Option<&str> {
        self.coordinator.last_error(target)
    }

    /// Remaps request ids after a rename/move changed their on-disk paths:
    /// updates the coordinator queue, dirty map, and persisted active-request selection.
    pub fn remap_request_ids(&mut self, pairs: &[(String, String)], cx: &mut Context<Self>) {
        if pairs.is_empty() {
            return;
        }
        let lookup: HashMap<&str, &str> = pairs
            .iter()
            .map(|(old, new)| (old.as_str(), new.as_str()))
            .collect();

        for (old_id, new_id) in pairs {
            let old_target = PersistTarget::Request(old_id.clone());
            let new_target = PersistTarget::Request(new_id.clone());
            self.coordinator
                .remap_target(&old_target, new_target.clone());
            if let Some(task) = self.debounce_tasks.remove(&old_target) {
                self.debounce_tasks.insert(new_target, task);
            }
        }

        self.unsaved_requests = self
            .unsaved_requests
            .iter()
            .map(|id| {
                lookup
                    .get(id.as_str())
                    .map(|n| (*n).to_string())
                    .unwrap_or_else(|| id.clone())
            })
            .collect();
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
        if let Some(active) = &self.local_state.active_request {
            if let Some(new_id) = lookup.get(active.as_str()) {
                self.local_state.active_request = Some((*new_id).to_string());
                self.schedule_state_save(cx);
            }
        }
    }

    /// Enqueues write to the coordinator and schedules/resets debounce timer for the target.
    pub fn enqueue_write(
        &mut self,
        target: PersistTarget,
        payload: PersistPayload,
        debounce: Duration,
        cx: &mut Context<Self>,
    ) {
        if let PersistTarget::Request(ref req_id) = target {
            self.unsaved_requests.insert(req_id.clone());
        }
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
        self.coordinator.enqueue(target.clone(), payload, debounce);
        cx.notify();

        let target_clone = target.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(debounce).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |p, cx| {
                    p.flush_target_if_ready(&target_clone, cx);
                });
            }
        });
        self.debounce_tasks.insert(target, task);
    }

    pub fn flush_target_if_ready(&mut self, target: &PersistTarget, cx: &mut Context<Self>) {
        if self.coordinator.is_in_flight(target) {
            return;
        }

        let Some(pending) = self.coordinator.begin_in_flight(target) else {
            return;
        };

        let store = self.store.clone();
        let payload = pending.payload;
        let target_clone = target.clone();
        let revision = pending.revision;
        let gen = pending.workspace_gen;
        let debounce = pending.debounce_duration;

        cx.spawn(async move |this, cx| {
            let write_res = cx
                .background_executor()
                .spawn({
                    let payload = payload.clone();
                    let store = store.clone();
                    async move { payload.write_to_store(store.as_ref()) }
                })
                .await;

            if let Some(this) = this.upgrade() {
                this.update(cx, |p, cx| {
                    p.handle_write_result(
                        WriteCompletion {
                            target: target_clone,
                            gen,
                            revision,
                            payload,
                            debounce,
                            result: write_res,
                        },
                        cx,
                    );
                });
            }
        })
        .detach();
    }

    fn handle_write_result(&mut self, completion: WriteCompletion, cx: &mut Context<Self>) {
        if completion.gen != self.coordinator.workspace_generation() {
            return;
        }

        match completion.result {
            Ok(()) => {
                let is_clean = self
                    .coordinator
                    .mark_success(&completion.target, completion.revision);
                if is_clean {
                    if let PersistTarget::Request(ref req_id) = completion.target {
                        self.unsaved_requests.remove(req_id);
                    }
                    self.debounce_tasks.remove(&completion.target);
                    self.unsaved_version = self.unsaved_version.wrapping_add(1);
                } else {
                    self.flush_target_if_ready(&completion.target, cx);
                }
                cx.notify();
            }
            Err(e) => {
                self.coordinator.mark_failure(
                    &completion.target,
                    completion.revision,
                    completion.payload,
                    e.to_string(),
                    completion.debounce,
                );
                self.unsaved_version = self.unsaved_version.wrapping_add(1);
                cx.notify();
            }
        }
    }

    pub fn schedule_save_active_request(
        &mut self,
        req_id: String,
        workspace_data: Entity<WorkspaceDataState>,
        cx: &mut Context<Self>,
    ) {
        if req_id == "scratch" || req_id.is_empty() {
            return;
        }
        let req = workspace_data.read(cx).find_request(&req_id).cloned();

        let Some(req) = req else {
            self.unsaved_requests.remove(&req_id);
            self.coordinator
                .cancel_target(&PersistTarget::Request(req_id.clone()));
            self.debounce_tasks.remove(&PersistTarget::Request(req_id));
            self.unsaved_version = self.unsaved_version.wrapping_add(1);
            cx.notify();
            return;
        };

        let target = PersistTarget::Request(req_id);
        let payload = PersistPayload::Request(Box::new(req));
        self.enqueue_write(target, payload, Duration::from_millis(500), cx);
    }

    pub fn schedule_save_environment(
        &mut self,
        name: String,
        env: Environment,
        cx: &mut Context<Self>,
    ) {
        let target = PersistTarget::Environment(name.clone());
        let payload = PersistPayload::Environment {
            name,
            env: Box::new(env),
        };
        self.enqueue_write(target, payload, Duration::from_millis(500), cx);
    }

    pub fn schedule_save_globals(&mut self, env: Environment, cx: &mut Context<Self>) {
        let target = PersistTarget::Globals;
        let payload = PersistPayload::Globals(Box::new(env));
        self.enqueue_write(target, payload, Duration::from_millis(500), cx);
    }

    pub fn schedule_save_workspace_meta(&mut self, meta: WorkspaceMeta, cx: &mut Context<Self>) {
        let target = PersistTarget::WorkspaceMeta;
        let payload = PersistPayload::WorkspaceMeta(Box::new(meta));
        self.enqueue_write(target, payload, Duration::from_millis(500), cx);
    }

    pub fn schedule_save_folder_meta(
        &mut self,
        folder_rel: String,
        meta: FolderFile,
        cx: &mut Context<Self>,
    ) {
        let target = PersistTarget::FolderMeta(folder_rel.clone());
        let payload = PersistPayload::FolderMeta {
            folder_rel,
            meta: Box::new(meta),
        };
        self.enqueue_write(target, payload, Duration::from_millis(500), cx);
    }

    pub fn schedule_save_pins(&mut self, pins: Vec<String>, cx: &mut Context<Self>) {
        let target = PersistTarget::Pins;
        let payload = PersistPayload::Pins(pins);
        self.enqueue_write(target, payload, Duration::from_millis(500), cx);
    }

    pub fn cancel_request(&mut self, req_id: &str, cx: &mut Context<Self>) {
        let target = PersistTarget::Request(req_id.to_string());
        self.coordinator.cancel_target(&target);
        self.debounce_tasks.remove(&target);
        self.unsaved_requests.remove(req_id);
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
        cx.notify();
    }

    pub fn cancel_environment(&mut self, env_name: &str, cx: &mut Context<Self>) {
        let target = PersistTarget::Environment(env_name.to_string());
        self.coordinator.cancel_target(&target);
        self.debounce_tasks.remove(&target);
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
        cx.notify();
    }

    pub fn flush_all(&mut self, cx: &mut Context<Self>) {
        let targets = self.coordinator.pending_targets();
        for target in targets {
            self.debounce_tasks.remove(&target);
            self.flush_target_if_ready(&target, cx);
        }
    }

    pub fn start_watcher(
        &mut self,
        workspace_data: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        git_state: Entity<GitState>,
        cx: &mut Context<Self>,
    ) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FsEvent>();
        let sup = self.store.suppressed_paths();
        if let Ok(watcher) =
            zest_core::storage::start_workspace_watcher(&self.workspace_path, sup, tx)
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
        let mut env_changed = false;
        let mut req_changed = std::collections::HashSet::new();
        let mut req_deleted = std::collections::HashSet::new();

        for event in events {
            match event {
                FsEvent::GitChanged => git_changed = true,
                FsEvent::TreeChanged => tree_changed = true,
                FsEvent::EnvironmentChanged(_) => env_changed = true,
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
        // again by the time the batch is handled. A pending deletion whose file
        // is still on disk must not drop the request from the workspace; it is
        // downgraded to a change and re-read via a full tree rescan.
        let mut verified_deleted = std::collections::HashSet::new();
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

        if tree_changed {
            workspace_data.update(cx, |ws, cx| {
                ws.reload_tree();
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
                            // The file exists on disk but the request is absent
                            // from memory: it was removed by an earlier deletion
                            // batch and has been recreated since. Rescan so it is
                            // re-added with correct ordering instead of ignored.
                            needs_rescan = true;
                        }
                    }

                    for rel_path in &req_deleted {
                        ws.parse_errors.remove(rel_path);
                        ws.requests.retain(|r| &r.id != rel_path);
                    }

                    ws.bump_tree_version();
                    if needs_rescan {
                        ws.reload_tree();
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

        if env_changed {
            env_state.update(cx, |env, cx| {
                env.reload_environments();
                cx.notify();
            });
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

    pub fn resolve_conflict_reload(
        &mut self,
        rel_path: &str,
        workspace_data: Entity<WorkspaceDataState>,
        cx: &mut Context<Self>,
    ) {
        self.conflicts.remove(rel_path);
        let target = PersistTarget::Request(rel_path.to_string());
        self.coordinator.cancel_target(&target);
        self.debounce_tasks.remove(&target);
        if self.unsaved_requests.remove(rel_path) {
            self.unsaved_version = self.unsaved_version.wrapping_add(1);
        }
        if let Ok(req_file) = self.store.read_request(rel_path) {
            let path = rel_path.to_string();
            workspace_data.update(cx, |ws, cx| {
                ws.parse_errors.remove(&path);
                let ws_name = ws.workspace_meta.name.clone();
                if let Some(r) = ws.requests.iter_mut().find(|r| r.id == path) {
                    let prev_status = r.last_status_code.or_else(|| {
                        ws.store
                            .load_response_cache(&r.id)
                            .ok()
                            .flatten()
                            .map(|l| l.response.status)
                    });
                    *r = req_file.into_item(&r.id, &ws_name, r.folder_id.clone());
                    r.last_status_code = prev_status;
                }
                ws.bump_tree_version();
                cx.emit(WorkspaceEvent::RequestsReloaded {
                    req_ids: Some(vec![path]),
                });
                cx.notify();
            });
        }
        cx.notify();
    }

    pub fn resolve_conflict_keep(
        &mut self,
        rel_path: &str,
        workspace_data: Entity<WorkspaceDataState>,
        cx: &mut Context<Self>,
    ) {
        self.conflicts.remove(rel_path);
        if self.unsaved_requests.remove(rel_path) {
            self.unsaved_version = self.unsaved_version.wrapping_add(1);
        }
        let req_opt = workspace_data
            .read(cx)
            .requests
            .iter()
            .find(|r| r.id == rel_path)
            .cloned();
        if let Some(r) = req_opt {
            let target = PersistTarget::Request(rel_path.to_string());
            let payload = PersistPayload::Request(Box::new(r));
            self.enqueue_write(target, payload, Duration::from_millis(0), cx);
        }
        cx.notify();
    }
}
