use super::EnvironmentEvent;
use gpui_kit::Context;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use zest_core::models::Environment;
use zest_core::storage::models::EnvironmentFile;
use zest_core::storage::WorkspaceStore;

#[derive(Clone)]
pub struct EnvironmentSnapshot {
    environments: Vec<Environment>,
    globals: Environment,
    corrupt_secrets_warning: Option<String>,
}

impl EnvironmentSnapshot {
    pub fn load(store: &dyn WorkspaceStore, workspace_name: &str) -> Self {
        let mut corrupt_secrets_warning = None;
        let mut load_secrets = |name: &str| {
            store.load_secrets(name).unwrap_or_else(|error| {
                corrupt_secrets_warning = Some(error.to_string());
                HashMap::new()
            })
        };
        let mut environments = Vec::new();
        for name in store.list_environments().unwrap_or_default() {
            if let Ok(file) = store.load_environment(&name) {
                let secrets = load_secrets(&name);
                environments.push(file.to_environment(&name, workspace_name, &name, &secrets));
            }
        }
        environments.sort_by_key(|environment| environment.name.to_lowercase());
        let globals = store
            .load_global_environment()
            .unwrap_or_default()
            .to_environment(
                "globals",
                workspace_name,
                "Globals",
                &load_secrets("globals"),
            );
        Self {
            environments,
            globals,
            corrupt_secrets_warning,
        }
    }
}

pub struct EnvironmentState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_name: String,
    pub environments: Vec<Environment>,
    pub global_environment: Environment,
    pub active_environment_id: Option<String>,
    pub corrupt_secrets_warning: Option<String>,
    pub(super) reserved_names: HashSet<String>,
    cached_effective_env: RwLock<Option<Arc<Environment>>>,
}

impl EnvironmentState {
    #[cfg(test)]
    pub fn new(
        store: Arc<dyn WorkspaceStore>,
        workspace_name: String,
        initial_active_id: Option<String>,
    ) -> Self {
        let mut state = Self::empty(store, workspace_name);
        state.active_environment_id = initial_active_id;
        state.reload_environments();
        state
    }

    pub(crate) fn empty(store: Arc<dyn WorkspaceStore>, workspace_name: String) -> Self {
        Self {
            store,
            workspace_name: workspace_name.clone(),
            environments: Vec::new(),
            global_environment: Environment::new(&workspace_name, "Globals"),
            active_environment_id: None,
            corrupt_secrets_warning: None,
            reserved_names: HashSet::new(),
            cached_effective_env: RwLock::new(None),
        }
    }

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
    fn reload_environments(&mut self) {
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

    /// Reloads a single environment (or `"globals"`) directly from disk,
    /// discarding any in-memory edits for that target.
    pub fn reload_environment_from_disk(&mut self, name: &str, cx: &mut Context<Self>) {
        let ws_name = self.workspace_name.clone();
        if name == "globals" {
            let global_file = self.store.load_global_environment().unwrap_or_default();
            let global_secrets = self.store.load_global_secrets().unwrap_or_default();
            self.global_environment =
                global_file.to_environment("globals", &ws_name, "Globals", &global_secrets);
        } else {
            let exists_on_disk = self
                .store
                .list_environments()
                .unwrap_or_default()
                .iter()
                .any(|n| n == name);
            if exists_on_disk {
                if let Ok(env_file) = self.store.load_environment(name) {
                    let secrets = self.store.load_secrets(name).unwrap_or_default();
                    let env = env_file.to_environment(name, &ws_name, name, &secrets);
                    if let Some(slot) = self.environments.iter_mut().find(|e| e.name == name) {
                        *slot = env;
                    } else {
                        self.environments.push(env);
                        self.environments.sort_by_key(|a| a.name.to_lowercase());
                    }
                }
            } else {
                self.environments.retain(|e| e.name != name);
            }
        }
        self.invalidate_cache();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        cx.notify();
    }

    pub fn active_environment(&self) -> Option<&Environment> {
        if let Some(env_id) = &self.active_environment_id {
            self.environments
                .iter()
                .find(|e| &e.id == env_id || &e.name == env_id)
        } else {
            None
        }
    }

    pub fn active_environment_mut(&mut self) -> Option<&mut Environment> {
        self.invalidate_cache();
        if let Some(env_id) = &self.active_environment_id {
            self.environments
                .iter_mut()
                .find(|e| &e.id == env_id || &e.name == env_id)
        } else {
            None
        }
    }

    pub fn set_active_environment(&mut self, env_id: Option<String>, cx: &mut Context<Self>) {
        if self.active_environment_id == env_id {
            return;
        }
        self.active_environment_id = env_id.clone();
        self.invalidate_cache();
        cx.emit(EnvironmentEvent::ActiveEnvironmentChanged(env_id));
        cx.notify();
    }

    pub fn invalidate_cache(&self) {
        if let Ok(mut lock) = self.cached_effective_env.write() {
            *lock = None;
        }
    }

    pub fn effective_environment(&self) -> Arc<Environment> {
        if let Ok(lock) = self.cached_effective_env.read() {
            if let Some(cached) = &*lock {
                return cached.clone();
            }
        }

        let effective = Arc::new(self.compute_effective_environment());
        if let Ok(mut lock) = self.cached_effective_env.write() {
            *lock = Some(effective.clone());
        }
        effective
    }

    fn compute_effective_environment(&self) -> Environment {
        let ws_name = self.workspace_name.as_str();
        let mut effective = Environment::new(ws_name, "Effective");
        let mut key_map = HashMap::<&str, usize>::new();

        // Duplicate keys resolve first-wins within a layer (matching the
        // variable tooltip, autocomplete and interpolator), while the active
        // environment still overrides globals.
        for v in &self.global_environment.variables {
            if key_map.contains_key(v.key.as_str()) {
                continue;
            }
            key_map.insert(v.key.as_str(), effective.variables.len());
            effective.variables.push(v.clone());
        }

        if let Some(active_env) = self.active_environment() {
            let mut seen_active = HashSet::new();
            for v in &active_env.variables {
                if !seen_active.insert(v.key.as_str()) {
                    continue;
                }
                if let Some(&idx) = key_map.get(v.key.as_str()) {
                    effective.variables[idx] = v.clone();
                } else {
                    key_map.insert(v.key.as_str(), effective.variables.len());
                    effective.variables.push(v.clone());
                }
            }
        }
        effective
    }

    pub fn save_global_environment_silently(&mut self, env: Environment) {
        self.global_environment = env;
        self.invalidate_cache();
    }

    pub fn update_variable_key(
        &mut self,
        env_id: &str,
        idx: usize,
        new_key: String,
        cx: &mut Context<Self>,
    ) {
        if env_id == "globals" {
            if let Some(vr) = self.global_environment.variables.get_mut(idx) {
                vr.key = new_key;
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::GlobalsModified);
                cx.notify();
            }
        } else if let Some(e) = self
            .environments
            .iter_mut()
            .find(|x| x.id == env_id || x.name == env_id)
        {
            if let Some(vr) = e.variables.get_mut(idx) {
                vr.key = new_key;
                let env_name = e.name.clone();
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::EnvironmentModified(env_name));
                cx.notify();
            }
        }
    }

    pub fn update_variable_initial_value(
        &mut self,
        env_id: &str,
        idx: usize,
        new_val: String,
        cx: &mut Context<Self>,
    ) {
        if env_id == "globals" {
            if let Some(vr) = self.global_environment.variables.get_mut(idx) {
                vr.initial_value = new_val;
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::GlobalsModified);
                cx.notify();
            }
        } else if let Some(e) = self
            .environments
            .iter_mut()
            .find(|x| x.id == env_id || x.name == env_id)
        {
            if let Some(vr) = e.variables.get_mut(idx) {
                vr.initial_value = new_val;
                let env_name = e.name.clone();
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::EnvironmentModified(env_name));
                cx.notify();
            }
        }
    }

    pub fn update_variable_current_value(
        &mut self,
        env_id: &str,
        idx: usize,
        new_val: String,
        cx: &mut Context<Self>,
    ) {
        if env_id == "globals" {
            if let Some(vr) = self.global_environment.variables.get_mut(idx) {
                vr.current_value = new_val;
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::GlobalsModified);
                cx.notify();
            }
        } else if let Some(e) = self
            .environments
            .iter_mut()
            .find(|x| x.id == env_id || x.name == env_id)
        {
            if let Some(vr) = e.variables.get_mut(idx) {
                vr.current_value = new_val;
                let env_name = e.name.clone();
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::EnvironmentModified(env_name));
                cx.notify();
            }
        }
    }

    pub fn toggle_variable_secret(&mut self, env_id: &str, idx: usize, cx: &mut Context<Self>) {
        if env_id == "globals" {
            if let Some(vr) = self.global_environment.variables.get_mut(idx) {
                vr.secret = !vr.secret;
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::GlobalsModified);
                cx.notify();
            }
        } else if let Some(e) = self
            .environments
            .iter_mut()
            .find(|x| x.id == env_id || x.name == env_id)
        {
            if let Some(vr) = e.variables.get_mut(idx) {
                vr.secret = !vr.secret;
                let env_name = e.name.clone();
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::EnvironmentModified(env_name));
                cx.notify();
            }
        }
    }

    pub fn add_variable(&mut self, env_id: &str, cx: &mut Context<Self>) {
        self.add_variable_with_key(env_id, String::new(), cx);
    }

    pub fn add_variable_with_key(&mut self, env_id: &str, key: String, cx: &mut Context<Self>) {
        let new_var = zest_core::models::EnvironmentVariable {
            key,
            initial_value: String::new(),
            current_value: String::new(),
            secret: false,
        };
        if env_id == "globals" {
            self.global_environment.variables.push(new_var);
            self.invalidate_cache();
            cx.emit(EnvironmentEvent::GlobalsModified);
            cx.notify();
        } else if let Some(e) = self
            .environments
            .iter_mut()
            .find(|x| x.id == env_id || x.name == env_id)
        {
            e.variables.push(new_var);
            let env_name = e.name.clone();
            self.invalidate_cache();
            cx.emit(EnvironmentEvent::EnvironmentModified(env_name));
            cx.notify();
        }
    }

    pub fn resolve_variable_env_id(&self, var_name: &str) -> String {
        if let Some(active) = self.active_environment() {
            if active.variables.iter().any(|v| v.key == var_name) {
                return active.id.clone();
            }
        }
        if self
            .global_environment
            .variables
            .iter()
            .any(|v| v.key == var_name)
        {
            return "globals".to_string();
        }
        for env in &self.environments {
            if env.variables.iter().any(|v| v.key == var_name) {
                return env.id.clone();
            }
        }
        self.active_environment_id
            .clone()
            .unwrap_or_else(|| "globals".to_string())
    }

    pub fn has_variable(&self, env_id: &str, var_name: &str) -> bool {
        if env_id == "globals" {
            self.global_environment
                .variables
                .iter()
                .any(|v| v.key == var_name)
        } else if let Some(e) = self
            .environments
            .iter()
            .find(|x| x.id == env_id || x.name == env_id)
        {
            e.variables.iter().any(|v| v.key == var_name)
        } else {
            false
        }
    }

    pub fn delete_variable(&mut self, env_id: &str, idx: usize, cx: &mut Context<Self>) {
        if env_id == "globals" {
            if idx < self.global_environment.variables.len() {
                self.global_environment.variables.remove(idx);
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::GlobalsModified);
                cx.notify();
            }
        } else if let Some(e) = self
            .environments
            .iter_mut()
            .find(|x| x.id == env_id || x.name == env_id)
        {
            if idx < e.variables.len() {
                e.variables.remove(idx);
                let env_name = e.name.clone();
                self.invalidate_cache();
                cx.emit(EnvironmentEvent::EnvironmentModified(env_name));
                cx.notify();
            }
        }
    }

    pub fn create_environment(&mut self, name: Option<String>, cx: &mut Context<Self>) -> String {
        let env_name = name.unwrap_or_else(|| {
            (self.environments.len() + 1..)
                .map(|index| format!("env-{index}"))
                .find(|name| !self.environment_name_in_use(name))
                .expect("there is an unused default environment name")
        });
        if self.environment_name_in_use(&env_name) {
            return env_name;
        }
        let env_file = EnvironmentFile::default();
        if let Err(error) = self.store.save_environment(&env_name, &env_file) {
            crate::notify::error("Could Not Create Environment", error.to_string(), cx);
            return env_name;
        }
        self.reload_environment_from_disk(&env_name, cx);
        if self.active_environment_id.is_none() {
            self.set_active_environment(Some(env_name.clone()), cx);
        }
        env_name
    }

    fn environment_name_in_use(&self, name: &str) -> bool {
        self.reserved_names.contains(&name.to_ascii_lowercase())
            || self
                .environments
                .iter()
                .any(|env| env.name.eq_ignore_ascii_case(name))
    }

    pub fn duplicate_environment(
        &mut self,
        env_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let existing = self
            .environments
            .iter()
            .find(|e| e.id == env_id || e.name == env_id)?
            .clone();
        let new_name = format!("{}-copy", existing.name);
        if self.environment_name_in_use(&new_name) {
            crate::notify::error(
                "Could Not Duplicate Environment",
                "The copy already exists",
                cx,
            );
            return None;
        }
        let (env_file, secrets) = EnvironmentFile::from_environment(&existing);
        if let Err(error) = self
            .store
            .save_environment(&new_name, &env_file)
            .and_then(|()| self.store.save_secrets(&new_name, &secrets))
        {
            crate::notify::error("Could Not Duplicate Environment", error.to_string(), cx);
            return None;
        }
        self.reload_environment_from_disk(&new_name, cx);
        Some(new_name)
    }

    pub fn rename_environment(&mut self, env_id: &str, new_name: String, cx: &mut Context<Self>) {
        let trimmed = new_name.trim();
        if env_id == "globals" || trimmed == env_id {
            return;
        }
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("globals")
            || trimmed.starts_with('.')
            || trimmed.contains(['/', '\\', ':'])
        {
            crate::notify::error(
                "Could Not Rename Environment",
                "Invalid environment name",
                cx,
            );
            return;
        }
        cx.emit(EnvironmentEvent::RenameRequested {
            id: env_id.to_string(),
            name: trimmed.to_string(),
        });
    }

    pub fn delete_environment(&mut self, env_id: &str, cx: &mut Context<Self>) {
        if env_id != "globals" {
            cx.emit(EnvironmentEvent::DeleteRequested(env_id.to_string()));
        }
    }

    pub(super) fn finish_environment_operation(
        &mut self,
        id: &str,
        new_name: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Option<Environment> {
        let index = self.environments.iter().position(|env| env.id == id)?;
        let renamed = if let Some(name) = new_name {
            let env = &mut self.environments[index];
            env.id = name.to_string();
            env.name = name.to_string();
            let updated = env.clone();
            self.environments.sort_by_key(|env| env.name.to_lowercase());
            cx.emit(EnvironmentEvent::EnvironmentRenamed {
                old_id: id.to_string(),
                new_id: name.to_string(),
            });
            Some(updated)
        } else {
            self.environments.remove(index);
            None
        };
        self.invalidate_cache();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        if self.active_environment_id.as_deref() == Some(id) {
            let next = new_name
                .map(str::to_string)
                .or_else(|| self.environments.first().map(|env| env.id.clone()));
            self.set_active_environment(next, cx);
        }
        cx.notify();
        renamed
    }
}

#[cfg(test)]
#[path = "environment_state_tests.rs"]
mod tests;
