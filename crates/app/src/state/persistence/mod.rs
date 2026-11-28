//! Workspace persistence, including ordered writes and watcher reconciliation.

mod conflicts;
mod environment;
mod queue;
#[cfg(test)]
mod tests;
mod watcher;

use super::CollectionState;
use conflicts::PersistenceChanges;
use environment::EnvironmentOperations;
use gpui_kit::{Context, Entity};
use queue::WriteQueue;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use watcher::WatcherSubscription;
use zest_core::models::Environment;
use zest_core::storage::coordinator::{PersistPayload, PersistTarget, TargetStatus};
use zest_core::storage::models::{FolderFile, LocalState, WorkspaceMeta};
use zest_core::storage::WorkspaceStore;

pub struct PersistenceState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_path: PathBuf,
    pub local_state: LocalState,
    changes: PersistenceChanges,
    writes: WriteQueue,
    watcher: WatcherSubscription,
    environments: EnvironmentOperations,
}

impl PersistenceState {
    #[cfg(test)]
    pub fn new(store: Arc<dyn WorkspaceStore>, workspace_path: PathBuf) -> Self {
        let local_state = store.load_state().unwrap_or_default();
        let mut state = Self::empty(store, workspace_path);
        state.local_state = local_state;
        state
    }

    pub(crate) fn empty(store: Arc<dyn WorkspaceStore>, workspace_path: PathBuf) -> Self {
        Self {
            store,
            workspace_path,
            local_state: LocalState::default(),
            changes: PersistenceChanges::default(),
            writes: WriteQueue::default(),
            watcher: WatcherSubscription::default(),
            environments: EnvironmentOperations::default(),
        }
    }

    pub fn set_workspace(
        &mut self,
        store: Arc<dyn WorkspaceStore>,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.watcher = WatcherSubscription::default();
        self.store = store;
        self.workspace_path = path;
        self.changes.reset();
        self.writes.reset_for_workspace();
        self.environments.clear();
        self.local_state = LocalState::default();
        cx.notify();
    }

    pub fn unsaved_requests(&self) -> &HashSet<String> {
        &self.changes.unsaved_requests
    }

    pub fn unsaved_version(&self) -> usize {
        self.changes.unsaved_version
    }

    pub fn request_conflict(&self, id: &str) -> Option<&str> {
        self.changes.requests.get(id).map(String::as_str)
    }

    pub fn environment_conflict(&self, id: &str) -> Option<&str> {
        self.changes.environments.get(id).map(String::as_str)
    }

    pub fn clear_request_conflict(&mut self, id: &str, cx: &mut Context<Self>) {
        self.changes.deleted_requests.remove(id);
        if self.changes.requests.remove(id).is_some() {
            cx.notify();
        }
    }

    fn schedule_state_save(&mut self, cx: &mut Context<Self>) {
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
        if self.changes.unsaved_requests.insert(req_id) {
            self.changes.bump();
            cx.notify();
        }
    }

    pub fn mark_clean(&mut self, req_id: &str, cx: &mut Context<Self>) {
        self.changes.restoring_requests.remove(req_id);
        let target = PersistTarget::Request(req_id.to_string());
        self.writes.cancel(&target);
        if self.changes.unsaved_requests.remove(req_id) {
            self.changes.bump();
            cx.notify();
        }
        self.finish_workspace_flush(Ok(()));
    }

    pub fn is_dirty(&self, req_id: &str) -> bool {
        self.changes.unsaved_requests.contains(req_id)
            || self
                .writes
                .coordinator
                .is_dirty(&PersistTarget::Request(req_id.to_string()))
    }

    pub(crate) fn dirty_request_ids(&self) -> HashSet<String> {
        let mut dirty = self.changes.unsaved_requests.clone();
        for target in self.writes.coordinator.dirty_targets() {
            if let PersistTarget::Request(id) = target {
                dirty.insert(id);
            }
        }
        dirty
    }

    #[allow(dead_code)] // Target status query
    pub fn target_status(&self, target: &PersistTarget) -> TargetStatus {
        self.writes.coordinator.status(target)
    }

    pub(crate) fn can_apply_workspace_meta(&self, store: &Arc<dyn WorkspaceStore>) -> bool {
        Arc::ptr_eq(store, &self.store)
            && !self
                .writes
                .coordinator
                .is_dirty(&PersistTarget::WorkspaceMeta)
    }

    #[cfg(test)]
    fn is_target_dirty(&self, target: &PersistTarget) -> bool {
        self.writes.coordinator.is_dirty(target)
    }

    #[allow(dead_code)] // Target error query
    pub fn target_error(&self, target: &PersistTarget) -> Option<&str> {
        self.writes.coordinator.last_error(target)
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

        for (old_id, _) in pairs {
            let old_target = PersistTarget::Request(old_id.clone());
            self.writes.cancel(&old_target);
        }

        self.changes.unsaved_requests = self
            .changes
            .unsaved_requests
            .iter()
            .map(|id| {
                lookup
                    .get(id.as_str())
                    .map(|n| (*n).to_string())
                    .unwrap_or_else(|| id.clone())
            })
            .collect();
        self.changes.bump();
        if let Some(active) = &self.local_state.active_request {
            if let Some(new_id) = lookup.get(active.as_str()) {
                self.local_state.active_request = Some((*new_id).to_string());
                self.schedule_state_save(cx);
            }
        }
    }

    /// Enqueues write to the coordinator and schedules/resets debounce timer for the target.
    pub fn schedule_save_active_request(
        &mut self,
        req_id: String,
        workspace_data: Entity<CollectionState>,
        cx: &mut Context<Self>,
    ) {
        if req_id == "scratch" || req_id.is_empty() {
            return;
        }
        let req = workspace_data.read(cx).find_request(&req_id).cloned();

        let Some(req) = req else {
            self.changes.unsaved_requests.remove(&req_id);
            self.writes.cancel(&PersistTarget::Request(req_id));
            self.changes.bump();
            cx.notify();
            return;
        };

        let restore = self.changes.restoring_requests.contains(&req_id);
        let target = PersistTarget::Request(req_id);
        let payload = if restore {
            PersistPayload::RestoreRequest(Box::new(req))
        } else {
            PersistPayload::Request(Box::new(req))
        };
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
        self.changes.deleted_requests.remove(req_id);
        self.changes.restoring_requests.remove(req_id);
        let target = PersistTarget::Request(req_id.to_string());
        self.writes.cancel(&target);
        self.changes.unsaved_requests.remove(req_id);
        self.changes.bump();
        cx.notify();
        self.finish_workspace_flush(Ok(()));
    }

    fn cancel_environment(&mut self, env_name: &str, cx: &mut Context<Self>) {
        let target = if env_name == "globals" {
            PersistTarget::Globals
        } else {
            PersistTarget::Environment(env_name.to_string())
        };
        self.writes.cancel(&target);
        self.changes.environments.remove(env_name);
        self.changes.bump();
        cx.notify();
        self.finish_workspace_flush(Ok(()));
    }
}
