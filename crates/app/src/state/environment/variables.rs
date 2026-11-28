use super::EnvironmentState;
use crate::state::EnvironmentEvent;
use gpui_kit::Context;
use zest_core::models::Environment;

impl EnvironmentState {
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
}
