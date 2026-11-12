use super::{
    EnvironmentState, PersistPayload, PersistTarget, PersistenceCoordinator, TargetStatus,
    WorkspaceDataState, WorkspaceEvent,
};
use gpui_kit::{Context, Entity, Task};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use zest_core::models::Environment;
use zest_core::storage::models::{FolderFile, LocalState, WorkspaceMeta};
use zest_core::storage::{WorkspaceStore, WorkspaceWatcher};

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
    pub env_conflicts: HashMap<String, String>,
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
            env_conflicts: HashMap::new(),
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
        self.env_conflicts.clear();
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

    #[allow(dead_code)] // Legacy / testing manual dirty marker
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

    #[allow(dead_code)] // Target status query
    pub fn target_status(&self, target: &PersistTarget) -> TargetStatus {
        self.coordinator.status(target)
    }

    #[allow(dead_code)] // Target dirty query
    pub fn is_target_dirty(&self, target: &PersistTarget) -> bool {
        self.coordinator.is_dirty(target)
    }

    #[allow(dead_code)] // Target error query
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
        let target = if env_name == "globals" {
            PersistTarget::Globals
        } else {
            PersistTarget::Environment(env_name.to_string())
        };
        self.coordinator.cancel_target(&target);
        self.debounce_tasks.remove(&target);
        self.env_conflicts.remove(env_name);
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
        cx.notify();
    }

    #[allow(dead_code)] // Useful for synchronous tests and manual sync
    pub fn flush_all(&mut self, cx: &mut Context<Self>) {
        let targets = self.coordinator.pending_targets();
        for target in targets {
            self.debounce_tasks.remove(&target);
            self.flush_target_if_ready(&target, cx);
        }
    }

    pub fn resolve_env_conflict_reload(
        &mut self,
        env_name: &str,
        env_state: Entity<EnvironmentState>,
        cx: &mut Context<Self>,
    ) {
        self.cancel_environment(env_name, cx);
        let name = env_name.to_string();
        env_state.update(cx, |env, cx| {
            env.reload_environment_from_disk(&name, cx);
        });
    }

    pub fn resolve_env_conflict_keep(
        &mut self,
        env_name: &str,
        env_state: Entity<EnvironmentState>,
        cx: &mut Context<Self>,
    ) {
        self.env_conflicts.remove(env_name);
        if env_name == "globals" {
            let env = env_state.read(cx).global_environment.clone();
            self.enqueue_write(
                PersistTarget::Globals,
                PersistPayload::Globals(Box::new(env)),
                Duration::from_millis(0),
                cx,
            );
        } else if let Some(env) = env_state
            .read(cx)
            .environments
            .iter()
            .find(|e| e.name == env_name || e.id == env_name)
            .cloned()
        {
            let name = env.name.clone();
            self.enqueue_write(
                PersistTarget::Environment(name.clone()),
                PersistPayload::Environment {
                    name,
                    env: Box::new(env),
                },
                Duration::from_millis(0),
                cx,
            );
        }
        cx.notify();
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
