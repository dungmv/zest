use super::{PersistTarget, PersistenceState};
use crate::services::workspace_watcher::WatchBatch;
use crate::state::{CollectionEvent, CollectionState, EnvironmentState, GitState};
use gpui_kit::{Context, Entity, Task};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use zest_core::storage::FsEvent;

#[derive(Default)]
pub(super) struct WatcherSubscription {
    task: Option<Task<()>>,
    events: Option<tokio::sync::mpsc::UnboundedSender<FsEvent>>,
}

#[derive(Clone)]
struct WatchTargets {
    collection: Entity<CollectionState>,
    environment: Entity<EnvironmentState>,
    git: Entity<GitState>,
}

struct WatchReadVersion {
    revisions: HashMap<PersistTarget, u64>,
    environment_operation_epoch: u64,
    collection_revision: u64,
}

impl WatchReadVersion {
    fn capture(state: &PersistenceState, collection: &CollectionState) -> Self {
        Self {
            revisions: state.writes.coordinator.revisions(),
            environment_operation_epoch: state.environments.epoch,
            collection_revision: collection.disk_revision(),
        }
    }
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
        self.watcher.events = Some(tx.clone());
        let store = self.store.clone();
        let path = self.workspace_path.clone();
        let fingerprints = store.disk_fingerprints();
        let targets = WatchTargets {
            collection,
            environment,
            git,
        };

        self.watcher.task = Some(cx.spawn(async move |this, cx| {
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
                    (
                        name,
                        WatchReadVersion::capture(state, targets.collection.read(cx)),
                    )
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
        if let Some(events) = &self.watcher.events {
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
        let revisions = WatchReadVersion::capture(self, collection.read(cx));
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
        mut batch: WatchBatch,
        targets: &WatchTargets,
        before: &WatchReadVersion,
        cx: &mut Context<Self>,
    ) {
        if before.collection_revision != targets.collection.read(cx).disk_revision()
            && (!batch.changed_requests.is_empty() || !batch.deleted_requests.is_empty())
        {
            batch.tree_changed = true;
            batch.changed_requests.clear();
            batch.deleted_requests.clear();
            batch.request_files.clear();
        }
        let mut dirty_requests = self.dirty_request_ids();
        let mut dirty_envs = HashSet::new();
        let dirty_targets = self.writes.coordinator.dirty_targets();
        let revisions = self.writes.coordinator.revisions();
        for target in
            dirty_targets
                .iter()
                .chain(revisions.iter().filter_map(|(target, revision)| {
                    (before.revisions.get(target) != Some(revision)).then_some(target)
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
        dirty_envs.extend(self.changing_environment_names().map(str::to_string));

        let meta_target = PersistTarget::WorkspaceMeta;
        if let Some(meta) = batch.workspace_meta {
            if !dirty_targets.contains(&meta_target)
                && revisions.get(&meta_target) == before.revisions.get(&meta_target)
            {
                let store = self.store.clone();
                targets.collection.update(cx, |_, cx| {
                    cx.emit(CollectionEvent::WorkspaceMetaReloaded {
                        meta: Box::new(meta),
                        store,
                    });
                });
            }
        }

        let mut self_changed = false;
        for id in &batch.changed_requests {
            if dirty_requests.contains(id) {
                self.changes.requests.insert(
                    id.clone(),
                    "File was modified externally while you have unsaved changes.".to_string(),
                );
                self_changed = true;
            }
        }
        let mut clean_deletions = HashSet::new();
        for id in &batch.deleted_requests {
            if dirty_requests.contains(id) {
                self.changed_request_deleted(id);
                self_changed = true;
                continue;
            }
            clean_deletions.insert(id.clone());
            let target = PersistTarget::Request(id.clone());
            self.writes.cancel(&target);
            if self.changes.unsaved_requests.remove(id) {
                self.changes.bump();
                self_changed = true;
            }
            self_changed |= self.changes.requests.remove(id).is_some();
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
                let files = batch
                    .request_files
                    .into_iter()
                    .filter(|reload| !dirty_requests.contains(&reload.id));
                if collection.apply_request_disk_changes(files, &clean_deletions, cx) {
                    collection.reload_tree_preserving_async(persistence, dirty_requests, cx);
                }
            });
        }

        if let Some(snapshot) = batch.environment {
            if before.environment_operation_epoch != self.environments.epoch {
                if let Some(events) = &self.watcher.events {
                    for name in batch.changed_envs {
                        let _ = events.send(FsEvent::EnvironmentChanged(name));
                    }
                }
            } else {
                let conflicts = targets.environment.update(cx, |env, cx| {
                    let conflicts =
                        env.apply_disk_changes(snapshot, &batch.changed_envs, &dirty_envs, cx);
                    cx.notify();
                    conflicts
                });
                for name in conflicts {
                    self.changes.environments.insert(
                        name,
                        "Environment file was modified externally while you have unsaved changes."
                            .to_string(),
                    );
                    self_changed = true;
                }
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

#[cfg(test)]
#[path = "watcher_tests.rs"]
mod tests;
