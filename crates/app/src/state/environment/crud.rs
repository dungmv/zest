use super::EnvironmentState;
use crate::services::workspace_io;
use crate::state::EnvironmentEvent;
use gpui_kit::Context;
use std::sync::Arc;
use zest_core::models::Environment;

impl EnvironmentState {
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
        self.create_environment_on_disk(env_name.clone(), None, true, cx);
        env_name
    }

    fn create_environment_on_disk(
        &mut self,
        name: String,
        copy: Option<Environment>,
        select_if_empty: bool,
        cx: &mut Context<Self>,
    ) {
        let reserved = name.to_ascii_lowercase();
        self.reserved_names.insert(reserved.clone());
        let store = self.store.clone();
        let workspace = self.workspace_name.clone();
        cx.spawn(async move |this, cx| {
            let disk = store.clone();
            let target = name.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    workspace_io::create_environment(disk.as_ref(), &target, &workspace, copy)
                })
                .await;
            let _ = this.update(cx, |env, cx| {
                if !Arc::ptr_eq(&store, &env.store) {
                    return;
                }
                env.reserved_names.remove(&reserved);
                match result {
                    Ok(environment) => {
                        env.upsert_environment(&name, environment);
                        env.invalidate_cache();
                        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
                        if select_if_empty && env.active_environment_id.is_none() {
                            env.set_active_environment(Some(name), cx);
                        }
                        cx.notify();
                    }
                    Err(error) => {
                        crate::notify::error("Could Not Create Environment", error.to_string(), cx)
                    }
                }
            });
        })
        .detach();
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
        self.create_environment_on_disk(new_name.clone(), Some(existing), false, cx);
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

    pub(in crate::state) fn finish_environment_operation(
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
