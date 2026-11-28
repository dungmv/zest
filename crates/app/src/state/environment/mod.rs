//! Named environments, globals, and the effective variable cache.

mod crud;
mod snapshot;
#[cfg(test)]
mod tests;
mod variables;

pub use zest_core::storage::EnvironmentSnapshot;

use super::EnvironmentEvent;
use gpui_kit::Context;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use zest_core::models::Environment;
use zest_core::storage::WorkspaceStore;

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
        zest_core::env::merge_effective_environment(
            &self.workspace_name,
            self.active_environment(),
            &self.global_environment,
        )
    }
}

#[cfg(test)]
mod effective_tests;
