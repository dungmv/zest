use super::EnvironmentState;
use crate::state::EnvironmentEvent;
use gpui_kit::Context;
use std::collections::HashSet;
use zest_core::models::Environment;
use zest_core::storage::{EnvironmentFile, EnvironmentRead, EnvironmentSnapshot};

impl EnvironmentState {
    pub(crate) fn apply_snapshot(&mut self, snapshot: EnvironmentSnapshot) {
        self.invalidate_cache();
        self.environments = snapshot.environments;
        self.global_environment = snapshot.globals;
        self.corrupt_secrets_warning = snapshot.corrupt_secrets_warning;
        if !self.active_environment_id.as_ref().is_some_and(|active| {
            self.environments
                .iter()
                .any(|env| &env.name == active || &env.id == active)
        }) {
            self.active_environment_id = self.environments.first().map(|env| env.id.clone());
        }
    }

    #[cfg(test)]
    pub(super) fn reload_environments(&mut self) {
        self.apply_snapshot(EnvironmentSnapshot::load(
            self.store.as_ref(),
            &self.workspace_name,
        ));
    }

    /// Reconciles external disk changes reported by the file watcher while
    /// preserving any environment (or `"globals"`) that currently has unsaved
    /// in-memory edits. Returns the list of environment names (or `"globals"`)
    /// that were modified on disk while dirty in memory so the caller can
    /// surface a conflict banner.
    pub fn apply_disk_changes(
        &mut self,
        snapshot: EnvironmentSnapshot,
        changed: &HashSet<String>,
        dirty: &HashSet<String>,
        cx: &mut Context<Self>,
    ) -> Vec<String> {
        let mut conflicts: Vec<String> = changed
            .iter()
            .filter(|name| dirty.contains(*name))
            .cloned()
            .collect();

        let should_reload = |name: &str| changed.contains(name) && !dirty.contains(name);
        self.environments.retain(|env| !should_reload(&env.name));
        self.environments.extend(
            snapshot
                .environments
                .into_iter()
                .filter(|env| should_reload(&env.name)),
        );
        self.environments.sort_by_key(|env| env.name.to_lowercase());
        if should_reload("globals") {
            self.global_environment = snapshot.globals;
        }
        self.corrupt_secrets_warning = snapshot.corrupt_secrets_warning;
        if !self
            .active_environment_id
            .as_ref()
            .is_some_and(|id| self.environments.iter().any(|env| &env.id == id))
        {
            self.set_active_environment(self.environments.first().map(|env| env.id.clone()), cx);
        }

        self.invalidate_cache();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        conflicts.sort();
        conflicts
    }

    pub(crate) fn apply_environment_read(
        &mut self,
        name: &str,
        read: Option<EnvironmentRead>,
        cx: &mut Context<Self>,
    ) {
        if let Some(read) = read {
            let environment = read.to_environment(name, &self.workspace_name);
            if let Some(warning) = read.secrets_warning {
                self.corrupt_secrets_warning = Some(warning);
            }
            self.upsert_environment(name, environment);
        } else if name == "globals" {
            self.global_environment = EnvironmentFile::default().to_environment(
                "globals",
                &self.workspace_name,
                "Globals",
                &Default::default(),
            );
        } else {
            self.environments.retain(|env| env.name != name);
        }
        self.invalidate_cache();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        cx.notify();
    }

    pub(super) fn upsert_environment(&mut self, name: &str, environment: Environment) {
        if name == "globals" {
            self.global_environment = environment;
        } else if let Some(slot) = self.environments.iter_mut().find(|env| env.name == name) {
            *slot = environment;
        } else {
            self.environments.push(environment);
            self.environments.sort_by_key(|env| env.name.to_lowercase());
        }
    }

    #[cfg(test)]
    pub fn reload_environment_from_disk(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Ok(read) = self.store.read_environment(name) {
            self.apply_environment_read(name, read, cx);
        }
    }
}
