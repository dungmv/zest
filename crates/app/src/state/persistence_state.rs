use super::{EnvironmentState, WorkspaceDataState, WorkspaceEvent};
use gpui::{Context, Entity, Task};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::storage::models::{LocalState, RequestFile};
use zest_core::storage::{FsEvent, WorkspaceStore, WorkspaceWatcher};

pub struct PersistenceState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_path: PathBuf,
    pub local_state: LocalState,
    pub conflicts: HashMap<String, String>,
    pub unsaved_requests: HashSet<String>,
    pub unsaved_version: usize,
    pub request_save_tasks: HashMap<String, Task<()>>,
    pub state_save_task: Option<Task<()>>,
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
            request_save_tasks: HashMap::new(),
            state_save_task: None,
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
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
        self.local_state = self.store.load_state().unwrap_or_default();
        cx.notify();
    }

    pub fn schedule_state_save(&mut self, cx: &mut Context<Self>) {
        self.state_save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |p, _| {
                    let _ = p.store.save_state(&p.local_state);
                });
            }
        }));
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
        if self.unsaved_requests.remove(req_id) {
            self.unsaved_version = self.unsaved_version.wrapping_add(1);
            cx.notify();
        }
    }

    pub fn is_dirty(&self, req_id: &str) -> bool {
        self.unsaved_requests.contains(req_id)
    }

    /// Remaps request ids after a rename/move changed their on-disk paths:
    /// updates the dirty map and the persisted active-request selection.
    pub fn remap_request_ids(&mut self, pairs: &[(String, String)], cx: &mut Context<Self>) {
        if pairs.is_empty() {
            return;
        }
        let lookup: HashMap<&str, &str> = pairs
            .iter()
            .map(|(old, new)| (old.as_str(), new.as_str()))
            .collect();
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

    pub fn schedule_save_active_request(
        &mut self,
        req_id: String,
        workspace_data: Entity<WorkspaceDataState>,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let task_req_id = req_id.clone();
        if self.unsaved_requests.insert(req_id.clone()) {
            self.unsaved_version = self.unsaved_version.wrapping_add(1);
            cx.notify();
        }

        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;

            let req = workspace_data.read_with(cx, |ws, _| ws.find_request(&task_req_id).cloned());

            let Some(req) = req else {
                if let Some(this_ent) = this.upgrade() {
                    this_ent.update(cx, |p, cx| {
                        if p.unsaved_requests.remove(&task_req_id) {
                            p.unsaved_version = p.unsaved_version.wrapping_add(1);
                            cx.notify();
                        }
                    });
                }
                return;
            };

            let store_clone = store.clone();
            let r_id = task_req_id.clone();
            let r_clone = req;
            cx.background_executor()
                .spawn(async move {
                    let mut req_file =
                        store_clone
                            .read_request(&r_id)
                            .unwrap_or_else(|_| RequestFile {
                                name: r_clone.name.clone(),
                                method: r_clone.method,
                                url: r_clone.url.clone(),
                                ..Default::default()
                            });
                    req_file.update_from_item(&r_clone);
                    let _ = store_clone.write_request(&r_id, &req_file);
                })
                .await;

            let final_req_id = task_req_id.clone();
            if let Some(this_ent) = this.upgrade() {
                this_ent.update(cx, |p, cx| {
                    if p.unsaved_requests.remove(&final_req_id) {
                        p.unsaved_version = p.unsaved_version.wrapping_add(1);
                        cx.notify();
                    }
                });
            }
        });
        self.request_save_tasks.insert(req_id, task);
    }

    pub fn start_watcher(
        &mut self,
        workspace_data: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
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
            let task = cx.spawn(async move |this, cx| {
                while let Some(event) = rx.recv().await {
                    let mut batch = vec![event];
                    while let Ok(ev) = rx.try_recv() {
                        batch.push(ev);
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;
                    while let Ok(ev) = rx.try_recv() {
                        batch.push(ev);
                    }

                    if let Some(this) = this.upgrade() {
                        let ws = ws_clone.clone();
                        let env = env_clone.clone();
                        this.update(cx, |p, cx| {
                            p.handle_fs_events_batch(batch, ws, env, cx);
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
        cx: &mut Context<Self>,
    ) {
        self.handle_fs_events_batch(vec![event], workspace_data, env_state, cx);
    }

    pub fn handle_fs_events_batch(
        &mut self,
        events: Vec<FsEvent>,
        workspace_data: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        cx: &mut Context<Self>,
    ) {
        let mut tree_changed = false;
        let mut env_changed = false;
        let mut req_changed = std::collections::HashSet::new();
        let mut req_deleted = std::collections::HashSet::new();

        for event in events {
            match event {
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

        let mut self_changed = false;

        for rel_path in &req_changed {
            if self.unsaved_requests.contains(rel_path) {
                self.conflicts.insert(
                    rel_path.clone(),
                    "File was modified externally while you have unsaved changes.".to_string(),
                );
                self_changed = true;
            }
        }

        for rel_path in &req_deleted {
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

            for rel_path in req_changed {
                if !self.unsaved_requests.contains(&rel_path) {
                    match self.store.read_request(&rel_path) {
                        Ok(req_file) => reqs_to_update.push((rel_path, req_file)),
                        Err(e) => parse_errors.push((rel_path, e.to_string())),
                    }
                }
            }

            if !reqs_to_update.is_empty() || !parse_errors.is_empty() || !req_deleted.is_empty() {
                workspace_data.update(cx, |ws, cx| {
                    for (path, err) in parse_errors {
                        ws.parse_errors.insert(path, err);
                    }

                    let mut reloaded_ids = Vec::with_capacity(reqs_to_update.len());
                    for (path, req_file) in reqs_to_update {
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
                        reloaded_ids.push(path);
                    }

                    for rel_path in req_deleted {
                        ws.parse_errors.remove(&rel_path);
                        ws.requests.retain(|r| r.id != rel_path);
                    }

                    ws.bump_tree_version();
                    if !reloaded_ids.is_empty() {
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
    }

    pub fn resolve_conflict_reload(
        &mut self,
        rel_path: &str,
        workspace_data: Entity<WorkspaceDataState>,
        cx: &mut Context<Self>,
    ) {
        self.conflicts.remove(rel_path);
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
            let mut req_file = self
                .store
                .read_request(rel_path)
                .unwrap_or_else(|_| RequestFile {
                    name: r.name.clone(),
                    method: r.method,
                    url: r.url.clone(),
                    ..Default::default()
                });
            req_file.update_from_item(&r);
            let _ = self.store.write_request(rel_path, &req_file);
        }
        cx.notify();
    }
}
