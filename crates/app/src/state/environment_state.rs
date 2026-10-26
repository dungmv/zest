use super::EnvironmentEvent;
use gpui::{Context, Task};
use std::collections::HashMap;
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
    pub env_save_task: Option<Task<()>>,
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
            env_save_task: None,
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

        self.environments = envs;

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

    pub fn save_global_environment(&mut self, env: Environment, cx: &mut Context<Self>) {
        self.global_environment = env;
        self.invalidate_cache();
        cx.notify();
    }

    pub fn create_environment(&mut self, name: Option<String>, cx: &mut Context<Self>) -> String {
        let env_name = name.unwrap_or_else(|| format!("env-{}", self.environments.len() + 1));
        let env_file = EnvironmentFile::default();
        let _ = self.store.save_environment(&env_name, &env_file);
        self.reload_environments();
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
        if self.active_environment_id.as_deref() == Some(env_id) {
            self.set_active_environment(self.environments.first().map(|e| e.id.clone()), cx);
        }
        cx.notify();
    }

    pub fn save_environments(&mut self, envs: Vec<Environment>, cx: &mut Context<Self>) {
        self.env_save_task = None;
        for env in &envs {
            let (file, secrets) = EnvironmentFile::from_environment(env);
            let _ = self.store.save_environment(&env.name, &file);
            let _ = self.store.save_secrets(&env.name, &secrets);
        }
        self.reload_environments();
        cx.notify();
    }

    pub fn save_environments_debounced(&mut self, envs: Vec<Environment>, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let envs_clone = envs.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            cx.background_executor()
                .spawn(async move {
                    for env in &envs_clone {
                        let (file, secrets) = EnvironmentFile::from_environment(env);
                        let _ = store.save_environment(&env.name, &file);
                        let _ = store.save_secrets(&env.name, &secrets);
                    }
                })
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |ws, cx| {
                    ws.reload_environments();
                    cx.notify();
                });
            }
        });
        self.env_save_task = Some(task);
    }

    pub fn flush_environments(&mut self) {
        if self.env_save_task.take().is_some() {
            for env in &self.environments {
                let (file, secrets) = EnvironmentFile::from_environment(env);
                let _ = self.store.save_environment(&env.name, &file);
                let _ = self.store.save_secrets(&env.name, &secrets);
            }
            self.reload_environments();
        }
    }
}
