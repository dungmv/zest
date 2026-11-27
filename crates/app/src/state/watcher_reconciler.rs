use super::{
    CollectionEvent, CollectionState, EnvironmentSnapshot, EnvironmentState, GitState,
    PersistTarget, PersistenceState,
};
use gpui_kit::{Context, Entity};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use zest_core::storage::{models::RequestFile, FsEvent, WorkspaceStore};

#[derive(Default)]
struct WatchBatch {
    git_changed: bool,
    tree_changed: bool,
    changed_envs: HashSet<String>,
    changed_requests: HashSet<String>,
    deleted_requests: HashSet<String>,
    request_files: Vec<RequestReload>,
    environment: Option<EnvironmentSnapshot>,
}

struct RequestReload {
    id: String,
    file: Result<RequestFile, String>,
    last_status: Option<u16>,
}

impl WatchBatch {
    fn read(events: Vec<FsEvent>, store: &dyn WorkspaceStore, workspace_name: &str) -> Self {
        let mut batch = Self::default();
        for event in events {
            match event {
                FsEvent::GitChanged => batch.git_changed = true,
                FsEvent::TreeChanged => batch.tree_changed = true,
                FsEvent::EnvironmentChanged(name) => {
                    batch.changed_envs.insert(name);
                }
                FsEvent::RequestChanged(id) => {
                    batch.changed_requests.insert(id);
                }
                FsEvent::RequestDeleted(id) => {
                    batch.deleted_requests.insert(id);
                }
            }
        }

        // An atomic replace can deliver both removal and creation. Recognize
        // the replacement even when the events arrive in different batches.
        let collections = store.workspace_root().join("collections");
        batch.deleted_requests.retain(|id| {
            if collections.join(id).is_file() {
                batch.tree_changed = true;
                batch.changed_requests.insert(id.clone());
                false
            } else {
                batch.changed_requests.remove(id);
                true
            }
        });

        if !batch.tree_changed {
            batch.request_files = batch
                .changed_requests
                .iter()
                .map(|id| {
                    let file = store.read_request(id).map_err(|error| error.to_string());
                    let last_status = store
                        .load_response_cache(id)
                        .ok()
                        .flatten()
                        .map(|cache| cache.response.status);
                    RequestReload {
                        id: id.clone(),
                        file,
                        last_status,
                    }
                })
                .collect();
        }
        if !batch.changed_envs.is_empty() {
            batch.environment = Some(EnvironmentSnapshot::load(store, workspace_name));
        }
        batch
    }
}

#[derive(Clone)]
struct WatchTargets {
    collection: Entity<CollectionState>,
    environment: Entity<EnvironmentState>,
    git: Entity<GitState>,
}

impl PersistenceState {
    pub fn start_watcher(
        &mut self,
        collection: Entity<CollectionState>,
        environment: Entity<EnvironmentState>,
        git: Entity<GitState>,
        cx: &mut Context<Self>,
    ) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FsEvent>();
        self.watcher_events = Some(tx.clone());
        let store = self.store.clone();
        let path = self.workspace_path.clone();
        let fingerprints = store.disk_fingerprints();
        let targets = WatchTargets {
            collection,
            environment,
            git,
        };

        self.watcher_task = Some(cx.spawn(async move |this, cx| {
            let _watcher = cx
                .background_executor()
                .spawn(async move {
                    zest_core::storage::start_workspace_watcher(&path, fingerprints, tx)
                })
                .await
                .ok();

            while let Some(event) = rx.recv().await {
                let mut events = vec![event];
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }

                let snapshot = this.update(cx, |state, cx| {
                    let name = targets.environment.read(cx).workspace_name.clone();
                    let revisions = state.coordinator.revisions();
                    (name, revisions)
                });
                let Ok((name, revisions)) = snapshot else {
                    break;
                };
                let read_store = store.clone();
                let batch = cx
                    .background_executor()
                    .spawn(async move { WatchBatch::read(events, read_store.as_ref(), &name) })
                    .await;
                let applied = this.update(cx, |state, cx| {
                    if Arc::ptr_eq(&store, &state.store) {
                        state.apply_watch_batch(batch, &targets, &revisions, cx);
                    }
                });
                if applied.is_err() {
                    break;
                }
            }
        }));
    }

    pub fn reconcile_import(&self, environments: Vec<String>) {
        if let Some(events) = &self.watcher_events {
            let _ = events.send(FsEvent::TreeChanged);
            for environment in environments {
                let _ = events.send(FsEvent::EnvironmentChanged(environment));
            }
        }
    }

    #[cfg(test)]
    pub fn handle_fs_event(
        &mut self,
        event: FsEvent,
        collection: Entity<CollectionState>,
        environment: Entity<EnvironmentState>,
        git: Entity<GitState>,
        cx: &mut Context<Self>,
    ) {
        self.handle_fs_events_batch(vec![event], collection, environment, git, cx);
    }

    #[cfg(test)]
    pub fn handle_fs_events_batch(
        &mut self,
        events: Vec<FsEvent>,
        collection: Entity<CollectionState>,
        environment: Entity<EnvironmentState>,
        git: Entity<GitState>,
        cx: &mut Context<Self>,
    ) {
        let name = &environment.read(cx).workspace_name;
        let batch = WatchBatch::read(events, self.store.as_ref(), name);
        let revisions = self.coordinator.revisions();
        self.apply_watch_batch(
            batch,
            &WatchTargets {
                collection,
                environment,
                git,
            },
            &revisions,
            cx,
        );
    }

    fn apply_watch_batch(
        &mut self,
        batch: WatchBatch,
        targets: &WatchTargets,
        before: &HashMap<PersistTarget, u64>,
        cx: &mut Context<Self>,
    ) {
        let mut dirty_requests = self.dirty_request_ids();
        let mut dirty_envs = HashSet::new();
        let dirty_targets = self.coordinator.dirty_targets();
        let revisions = self.coordinator.revisions();
        for target in
            dirty_targets
                .iter()
                .chain(revisions.iter().filter_map(|(target, revision)| {
                    (before.get(target) != Some(revision)).then_some(target)
                }))
        {
            match target {
                PersistTarget::Request(id) => {
                    dirty_requests.insert(id.clone());
                }
                PersistTarget::Environment(name) => {
                    dirty_envs.insert(name.clone());
                }
                PersistTarget::Globals => {
                    dirty_envs.insert("globals".to_string());
                }
                _ => {}
            }
        }

        let mut self_changed = false;
        for id in &batch.changed_requests {
            if dirty_requests.contains(id) {
                self.conflicts.insert(
                    id.clone(),
                    "File was modified externally while you have unsaved changes.".to_string(),
                );
                self_changed = true;
            }
        }
        for id in &batch.deleted_requests {
            let target = PersistTarget::Request(id.clone());
            self.coordinator.cancel_target(&target);
            self.debounce_tasks.remove(&target);
            if self.unsaved_requests.remove(id) {
                self.unsaved_version = self.unsaved_version.wrapping_add(1);
                self_changed = true;
            }
            self_changed |= self.conflicts.remove(id).is_some();
        }

        let working_tree_changed = batch.tree_changed
            || batch.environment.is_some()
            || !batch.changed_requests.is_empty()
            || !batch.deleted_requests.is_empty();
        let persistence = cx.entity().downgrade();
        if batch.tree_changed {
            targets.collection.update(cx, |collection, cx| {
                collection.reload_tree_preserving_async(persistence, dirty_requests, cx);
            });
        } else if !batch.request_files.is_empty() || !batch.deleted_requests.is_empty() {
            targets.collection.update(cx, |collection, cx| {
                let mut reloaded_ids = Vec::with_capacity(batch.request_files.len());
                let mut needs_rescan = false;
                for reload in batch.request_files {
                    if dirty_requests.contains(&reload.id) {
                        continue;
                    }
                    let file = match reload.file {
                        Ok(file) => file,
                        Err(error) => {
                            collection.parse_errors.insert(reload.id, error);
                            continue;
                        }
                    };
                    collection.parse_errors.remove(&reload.id);
                    let workspace_name = collection.workspace_scope.name.clone();
                    if let Some(request) = collection.find_request_mut(&reload.id) {
                        let status = request.last_status_code.or(reload.last_status);
                        let folder = request.folder_id.clone();
                        *request = file.into_item(&reload.id, &workspace_name, folder);
                        request.last_status_code = status;
                        reloaded_ids.push(reload.id);
                    } else {
                        needs_rescan = true;
                    }
                }

                for id in &batch.deleted_requests {
                    collection.parse_errors.remove(id);
                }
                collection
                    .requests
                    .retain(|request| !batch.deleted_requests.contains(&request.id));
                collection.bump_tree_version();
                if needs_rescan {
                    collection.reload_tree_preserving_async(persistence, dirty_requests, cx);
                } else if !reloaded_ids.is_empty() {
                    cx.emit(CollectionEvent::RequestsReloaded {
                        req_ids: Some(reloaded_ids),
                    });
                }
                cx.notify();
            });
        }

        if let Some(snapshot) = batch.environment {
            let conflicts = targets.environment.update(cx, |env, cx| {
                let conflicts =
                    env.apply_disk_changes(snapshot, &batch.changed_envs, &dirty_envs, cx);
                cx.notify();
                conflicts
            });
            for name in conflicts {
                self.env_conflicts.insert(
                    name,
                    "Environment file was modified externally while you have unsaved changes."
                        .to_string(),
                );
                self_changed = true;
            }
        }

        if self_changed {
            cx.notify();
        }
        if batch.git_changed || working_tree_changed {
            targets.git.update(cx, |git, cx| git.refresh_status(cx));
        }
    }
}
