use super::EnvironmentEvent;
use gpui_kit::Context;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use zest_core::models::Environment;
use zest_core::storage::models::EnvironmentFile;
use zest_core::storage::WorkspaceStore;

pub struct EnvironmentState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_name: String,
    pub environments: Vec<Environment>,
    pub global_environment: Environment,
    pub active_environment_id: Option<String>,
    pub corrupt_secrets_warning: Option<String>,
    cached_effective_env: RwLock<Option<Arc<Environment>>>,
}

impl EnvironmentState {
    pub fn new(
        store: Arc<dyn WorkspaceStore>,
        workspace_name: String,
        initial_active_id: Option<String>,
    ) -> Self {
        let mut state = Self {
            store,
            workspace_name: workspace_name.clone(),
            environments: Vec::new(),
            global_environment: Environment::new(&workspace_name, "Globals"),
            active_environment_id: initial_active_id,
            corrupt_secrets_warning: None,
            cached_effective_env: RwLock::new(None),
        };
        state.reload_environments();
        state
    }

    pub fn set_workspace(
        &mut self,
        store: Arc<dyn WorkspaceStore>,
        workspace_name: String,
        active_env: Option<String>,
    ) {
        self.store = store;
        self.workspace_name = workspace_name.clone();
        self.active_environment_id = active_env;
        self.corrupt_secrets_warning = None;
        self.global_environment = Environment::new(&workspace_name, "Globals");
        self.invalidate_cache();
        self.reload_environments();
    }

    pub fn reload_environments(&mut self) {
        self.invalidate_cache();
        let ws_name = self.workspace_name.clone();
        let env_names = self.store.list_environments().unwrap_or_default();
        let mut envs = Vec::new();

        for name in &env_names {
            if let Ok(env_file) = self.store.load_environment(name) {
                let secrets = match self.store.load_secrets(name) {
                    Ok(s) => s,
                    Err(e) => {
                        self.corrupt_secrets_warning = Some(format!("{}", e));
                        HashMap::new()
                    }
                };
                let env = env_file.to_environment(name, &ws_name, name, &secrets);
                envs.push(env);
            }
        }

        envs.sort_by_key(|a| a.name.to_lowercase());
        self.environments = envs;

        let global_file = self.store.load_global_environment().unwrap_or_default();
        let global_secrets = match self.store.load_global_secrets() {
            Ok(s) => s,
            Err(e) => {
                self.corrupt_secrets_warning = Some(format!("{}", e));
                HashMap::new()
            }
        };
        self.global_environment =
            global_file.to_environment("globals", &ws_name, "Globals", &global_secrets);

        if let Some(act) = &self.active_environment_id {
            if !self
                .environments
                .iter()
                .any(|e| &e.name == act || &e.id == act)
            {
                self.active_environment_id = self.environments.first().map(|e| e.id.clone());
            }
        } else if self.active_environment_id.is_none() {
            self.active_environment_id = self.environments.first().map(|e| e.id.clone());
        }
    }

    /// Reconciles external disk changes reported by the file watcher while
    /// preserving any environment (or `"globals"`) that currently has unsaved
    /// in-memory edits. Returns the list of environment names (or `"globals"`)
    /// that were modified on disk while dirty in memory so the caller can
    /// surface a conflict banner.
    pub fn apply_disk_changes(
        &mut self,
        changed: &HashSet<String>,
        dirty: &HashSet<String>,
        cx: &mut Context<Self>,
    ) -> Vec<String> {
        let mut conflicts: Vec<String> = changed
            .iter()
            .filter(|name| dirty.contains(*name))
            .cloned()
            .collect();

        let kept_envs: Vec<Environment> = self
            .environments
            .iter()
            .filter(|e| dirty.contains(&e.name))
            .cloned()
            .collect();
        let kept_globals = dirty
            .contains("globals")
            .then(|| self.global_environment.clone());

        self.reload_environments();

        for env in kept_envs {
            if let Some(slot) = self.environments.iter_mut().find(|e| e.name == env.name) {
                *slot = env;
            } else {
                if !conflicts.contains(&env.name) {
                    conflicts.push(env.name.clone());
                }
                self.environments.push(env);
                self.environments.sort_by_key(|a| a.name.to_lowercase());
            }
        }

        if let Some(g) = kept_globals {
            self.global_environment = g;
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

        for v in &self.global_environment.variables {
            key_map.insert(v.key.as_str(), effective.variables.len());
            effective.variables.push(v.clone());
        }

        if let Some(active_env) = self.active_environment() {
            for v in &active_env.variables {
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
        let new_var = zest_core::models::EnvironmentVariable {
            key: "".to_string(),
            initial_value: "".to_string(),
            current_value: "".to_string(),
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
        let env_name = name.unwrap_or_else(|| format!("env-{}", self.environments.len() + 1));
        let env_file = EnvironmentFile::default();
        let _ = self.store.save_environment(&env_name, &env_file);
        self.reload_environments();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        if self.active_environment_id.is_none() {
            self.set_active_environment(Some(env_name.clone()), cx);
        } else {
            cx.notify();
        }
        env_name
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
        let (env_file, secrets) = EnvironmentFile::from_environment(&existing);
        let _ = self.store.save_environment(&new_name, &env_file);
        let _ = self.store.save_secrets(&new_name, &secrets);
        self.reload_environments();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        cx.notify();
        Some(new_name)
    }

    pub fn rename_environment(&mut self, env_id: &str, new_name: String, cx: &mut Context<Self>) {
        let trimmed = new_name.trim();
        if trimmed.is_empty() || env_id == "globals" {
            return;
        }
        // Duplicate names would collide with the sibling env file on disk;
        // the store also refuses the rename as a last line of defense.
        if trimmed != env_id
            && self
                .environments
                .iter()
                .any(|e| e.id != env_id && (e.name == trimmed || e.id == trimmed))
        {
            return;
        }
        let was_active = self.active_environment_id.as_deref() == Some(env_id);
        let _ = self.store.rename_environment(env_id, trimmed);
        if was_active {
            self.active_environment_id = Some(trimmed.to_string());
        }
        self.reload_environments();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        if was_active {
            cx.emit(EnvironmentEvent::ActiveEnvironmentChanged(
                self.active_environment_id.clone(),
            ));
        }
        cx.notify();
    }

    pub fn delete_environment(&mut self, env_id: &str, cx: &mut Context<Self>) {
        if env_id == "globals" {
            return;
        }
        let _ = self.store.delete_environment(env_id);
        self.reload_environments();
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        if self.active_environment_id.as_deref() == Some(env_id) {
            self.set_active_environment(self.environments.first().map(|e| e.id.clone()), cx);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{AppContext as _, TestAppContext};
    use tempfile::tempdir;
    use zest_core::models::EnvironmentVariable;
    use zest_core::storage::models::EnvVarFile;
    use zest_core::storage::FsWorkspaceStore;

    #[test]
    fn test_reload_environments_sorted_alphabetically() {
        let temp = tempdir().expect("create temp dir");
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(temp.path()));
        let _ = store.init_workspace("TestWS");

        let _ = store.save_environment("production", &EnvironmentFile::default());
        let _ = store.save_environment("alpha", &EnvironmentFile::default());
        let _ = store.save_environment("Development", &EnvironmentFile::default());
        let _ = store.save_environment("beta", &EnvironmentFile::default());

        let state = EnvironmentState::new(store, "TestWS".to_string(), None);
        let names: Vec<&str> = state.environments.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["alpha", "beta", "Development", "local", "production"]
        );
    }

    #[gpui_kit::test]
    fn test_apply_disk_changes_preserves_dirty_env_and_globals(cx: &mut TestAppContext) {
        let temp = tempdir().expect("create temp dir");
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(temp.path()));
        store.init_workspace("TestWS").unwrap();

        let mut disk_dev = EnvironmentFile::default();
        disk_dev.variables.push(EnvVarFile {
            name: "HOST".to_string(),
            value: Some("disk-dev".to_string()),
            secret: false,
            enabled: true,
        });
        store.save_environment("dev", &disk_dev).unwrap();

        let mut disk_staging = EnvironmentFile::default();
        disk_staging.variables.push(EnvVarFile {
            name: "HOST".to_string(),
            value: Some("disk-staging-v1".to_string()),
            secret: false,
            enabled: true,
        });
        store.save_environment("staging", &disk_staging).unwrap();

        let env_entity =
            cx.new(|_| EnvironmentState::new(store.clone(), "TestWS".to_string(), None));

        // Simulate in-progress user typing in `dev` and `globals`
        env_entity.update(cx, |env, cx| {
            let dev_idx = env
                .environments
                .iter()
                .position(|e| e.name == "dev")
                .unwrap();
            env.environments[dev_idx].variables[0].current_value = "user-typing-abxy".to_string();
            env.global_environment.variables.push(EnvironmentVariable {
                key: "G_KEY".to_string(),
                initial_value: "g_typing".to_string(),
                current_value: "g_typing".to_string(),
                secret: false,
            });

            // Meanwhile disk changes for both `dev` (dirty) and `staging` (clean)
            let mut ext_staging = EnvironmentFile::default();
            ext_staging.variables.push(EnvVarFile {
                name: "HOST".to_string(),
                value: Some("disk-staging-v2".to_string()),
                secret: false,
                enabled: true,
            });
            store.save_environment("staging", &ext_staging).unwrap();

            let changed: HashSet<String> = ["dev".to_string(), "staging".to_string()]
                .into_iter()
                .collect();
            let dirty: HashSet<String> = ["dev".to_string(), "globals".to_string()]
                .into_iter()
                .collect();

            let conflicts = env.apply_disk_changes(&changed, &dirty, cx);
            assert_eq!(conflicts, vec!["dev".to_string()]);

            // Dirty `dev` and `globals` must retain user's in-memory text
            let dev = env.environments.iter().find(|e| e.name == "dev").unwrap();
            assert_eq!(dev.variables[0].current_value, "user-typing-abxy");
            assert_eq!(
                env.global_environment.variables[0].current_value,
                "g_typing"
            );

            // Clean `staging` must update to the new disk value
            let staging = env
                .environments
                .iter()
                .find(|e| e.name == "staging")
                .unwrap();
            assert_eq!(staging.variables[0].initial_value, "disk-staging-v2");

            // Reloading `dev` explicitly from disk discards the dirty memory copy
            env.reload_environment_from_disk("dev", cx);
            let dev_reloaded = env.environments.iter().find(|e| e.name == "dev").unwrap();
            assert_eq!(dev_reloaded.variables[0].initial_value, "disk-dev");
            assert_eq!(dev_reloaded.variables[0].current_value, "");
        });
    }
}
