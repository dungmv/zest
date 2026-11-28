use super::{PersistPayload, PersistTarget, PersistenceState};
use crate::services::workspace_watcher::RequestReload;
use crate::state::{CollectionState, EnvironmentState};
use gpui_kit::{Context, Entity};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
pub(super) struct PersistenceChanges {
    pub(super) requests: HashMap<String, String>,
    pub(super) deleted_requests: HashSet<String>,
    pub(super) restoring_requests: HashSet<String>,
    pub(super) environments: HashMap<String, String>,
    pub(super) unsaved_requests: HashSet<String>,
    pub(super) unsaved_version: usize,
}

impl PersistenceChanges {
    pub(super) fn bump(&mut self) {
        self.unsaved_version = self.unsaved_version.wrapping_add(1);
    }

    pub(super) fn reset(&mut self) {
        self.requests.clear();
        self.deleted_requests.clear();
        self.restoring_requests.clear();
        self.environments.clear();
        self.unsaved_requests.clear();
        self.bump();
    }
}

impl PersistenceState {
    pub(crate) fn changed_request_deleted(&mut self, id: &str) {
        self.changes.deleted_requests.insert(id.to_string());
        self.changes.requests.insert(
            id.to_string(),
            "File was deleted externally while you have unsaved changes.".to_string(),
        );
        if self.changes.unsaved_requests.insert(id.to_string()) {
            self.changes.bump();
        }
        self.writes
            .debounce_tasks
            .remove(&PersistTarget::Request(id.to_string()));
        self.finish_workspace_flush(Err(
            "Resolve deleted request conflicts before switching workspaces.".into(),
        ));
    }

    pub fn resolve_env_conflict_reload(
        &mut self,
        env_name: &str,
        env_state: Entity<EnvironmentState>,
        cx: &mut Context<Self>,
    ) {
        if self.environments.contains(env_name) {
            return;
        }
        self.cancel_environment(env_name, cx);
        let name = env_name.to_string();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn({
                    let store = store.clone();
                    let name = name.clone();
                    async move { store.read_environment(&name) }
                })
                .await;
            let _ = this.update(cx, |state, cx| {
                let target = if name == "globals" {
                    PersistTarget::Globals
                } else {
                    PersistTarget::Environment(name.clone())
                };
                if !Arc::ptr_eq(&store, &state.store)
                    || state.writes.coordinator.is_dirty(&target)
                    || state.environments.contains(&name)
                {
                    return;
                }
                match read {
                    Ok(read) => {
                        env_state.update(cx, |env, cx| env.apply_environment_read(&name, read, cx))
                    }
                    Err(error) => {
                        crate::notify::error("Could Not Reload Environment", error.to_string(), cx)
                    }
                }
            });
        })
        .detach();
    }

    pub fn resolve_env_conflict_keep(
        &mut self,
        env_name: &str,
        env_state: Entity<EnvironmentState>,
        cx: &mut Context<Self>,
    ) {
        self.changes.environments.remove(env_name);
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
        workspace_data: Entity<CollectionState>,
        cx: &mut Context<Self>,
    ) {
        self.changes.requests.remove(rel_path);
        self.changes.deleted_requests.remove(rel_path);
        self.changes.restoring_requests.remove(rel_path);
        let target = PersistTarget::Request(rel_path.to_string());
        self.writes.cancel(&target);
        if self.changes.unsaved_requests.remove(rel_path) {
            self.changes.bump();
        }
        let path = rel_path.to_string();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn({
                    let store = store.clone();
                    let path = path.clone();
                    async move { RequestReload::read_if_present(path, store.as_ref()) }
                })
                .await;
            let _ = this.update(cx, |state, cx| {
                if !Arc::ptr_eq(&store, &state.store) || state.is_dirty(&path) {
                    return;
                }
                workspace_data.update(cx, |ws, cx| {
                    if let Some(read) = read {
                        ws.apply_request_disk_changes([read], &HashSet::new(), cx);
                    } else {
                        ws.apply_request_disk_changes([], &HashSet::from([path]), cx);
                    }
                });
            });
        })
        .detach();
        cx.notify();
    }

    pub fn resolve_conflict_keep(
        &mut self,
        rel_path: &str,
        workspace_data: Entity<CollectionState>,
        cx: &mut Context<Self>,
    ) {
        self.changes.requests.remove(rel_path);
        let restore = self.changes.deleted_requests.remove(rel_path);
        if self.changes.unsaved_requests.remove(rel_path) {
            self.changes.bump();
        }
        let req_opt = workspace_data.read(cx).find_request(rel_path).cloned();
        if let Some(r) = req_opt {
            let target = PersistTarget::Request(rel_path.to_string());
            let payload = if restore {
                self.changes.restoring_requests.insert(rel_path.to_string());
                PersistPayload::RestoreRequest(Box::new(r))
            } else {
                PersistPayload::Request(Box::new(r))
            };
            self.enqueue_write(target, payload, Duration::from_millis(0), cx);
        }
        cx.notify();
    }
}
