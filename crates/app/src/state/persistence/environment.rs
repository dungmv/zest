//! Serializes environment file moves and deletions with write-behind saves.

use super::{PersistPayload, PersistTarget, PersistenceState};
use crate::state::EnvironmentState;
use gpui_kit::{Context, Entity};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct EnvironmentOperations {
    pending: HashMap<String, EnvironmentOperation>,
    pub(super) epoch: u64,
}

impl EnvironmentOperations {
    pub(super) fn clear(&mut self) {
        self.pending.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub(super) fn contains(&self, name: &str) -> bool {
        self.pending.contains_key(name)
    }

    fn finished(&mut self, id: &str) -> Option<EnvironmentOperation> {
        let operation = self.pending.remove(id)?;
        self.epoch = self.epoch.wrapping_add(1);
        Some(operation)
    }
}

struct EnvironmentOperation {
    environment: Entity<EnvironmentState>,
    new_name: Option<String>,
    running: bool,
}

impl PersistenceState {
    pub(crate) fn queue_environment_operation(
        &mut self,
        environment: Entity<EnvironmentState>,
        id: &str,
        new_name: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let env = environment.read(cx);
        if !env.environments.iter().any(|env| env.id == id) {
            return;
        }
        let reserved = |name: &str| {
            self.environments.pending.iter().any(|(source, operation)| {
                source.eq_ignore_ascii_case(name)
                    || operation
                        .new_name
                        .as_deref()
                        .is_some_and(|dest| dest.eq_ignore_ascii_case(name))
            })
        };
        if reserved(id)
            || new_name.as_deref().is_some_and(|name| {
                reserved(name)
                    || env.reserved_names.contains(&name.to_ascii_lowercase())
                    || env
                        .environments
                        .iter()
                        .any(|env| env.id != id && env.name.eq_ignore_ascii_case(name))
            })
        {
            crate::notify::error(
                "Could Not Change Environment",
                "The environment name is already in use",
                cx,
            );
            return;
        }

        let target = PersistTarget::Environment(id.to_string());
        if let Some(name) = &new_name {
            environment.update(cx, |env, _| {
                env.reserved_names.insert(name.to_ascii_lowercase());
            });
        }
        self.writes.debounce_tasks.remove(&target);
        self.environments.pending.insert(
            id.to_string(),
            EnvironmentOperation {
                environment,
                new_name,
                running: false,
            },
        );
        self.flush_target_if_ready(&target, cx);
    }

    pub(super) fn start_environment_operation(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(operation) = self.environments.pending.get_mut(id) else {
            return;
        };
        if operation.running {
            return;
        }
        operation.running = true;
        let new_name = operation.new_name.clone();
        let target = PersistTarget::Environment(id.to_string());
        let revision = self.writes.coordinator.revision(&target);
        let snapshot = if new_name.is_some() && self.writes.coordinator.is_dirty(&target) {
            operation
                .environment
                .read(cx)
                .environments
                .iter()
                .find(|env| env.id == id)
                .cloned()
                .map(|env| PersistPayload::Environment {
                    name: id.to_string(),
                    env: Box::new(env),
                })
        } else {
            None
        };
        let store = self.store.clone();
        let generation = self.writes.coordinator.workspace_generation();
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let source = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    if let Some(name) = new_name {
                        if let Some(snapshot) = snapshot {
                            snapshot.write_to_store(store.as_ref())?;
                        }
                        store.rename_environment(&source, &name)
                    } else {
                        store.delete_environment(&source)
                    }
                })
                .await;
            let _ = this.update(cx, |state, cx| {
                if state.writes.coordinator.workspace_generation() == generation {
                    state.finish_environment_operation(id, target, revision, result, cx);
                }
            });
        })
        .detach();
    }

    fn finish_environment_operation(
        &mut self,
        id: String,
        target: PersistTarget,
        revision: Option<u64>,
        result: anyhow::Result<()>,
        cx: &mut Context<Self>,
    ) {
        let Some(operation) = self.environments.finished(&id) else {
            return;
        };
        if let Some(name) = &operation.new_name {
            operation.environment.update(cx, |env, _| {
                env.reserved_names.remove(&name.to_ascii_lowercase());
            });
        }
        match result {
            Ok(()) => {
                let edited_during_operation = self.writes.coordinator.revision(&target) != revision;
                self.writes.cancel(&target);
                self.changes.environments.remove(&id);
                let renamed = operation.environment.update(cx, |env, cx| {
                    env.finish_environment_operation(&id, operation.new_name.as_deref(), cx)
                });
                if let Some(env) = renamed.filter(|_| edited_during_operation) {
                    self.schedule_save_environment(env.name.clone(), env, cx);
                }
                self.finish_workspace_flush(Ok(()));
            }
            Err(error) => {
                let error = error.to_string();
                self.finish_workspace_flush(Err(error.clone()));
                self.flush_target_if_ready(&target, cx);
                crate::notify::error("Could Not Change Environment", error, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn changing_environment_names(&self) -> impl Iterator<Item = &str> {
        self.environments
            .pending
            .iter()
            .flat_map(|(id, operation)| {
                std::iter::once(id.as_str()).chain(operation.new_name.as_deref())
            })
    }
}
