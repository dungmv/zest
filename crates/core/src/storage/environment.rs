use super::{EnvironmentFile, WorkspaceStore};
use crate::models::Environment;
use std::collections::HashMap;

/// A public environment file and the local values read alongside it.
pub struct EnvironmentRead {
    pub file: EnvironmentFile,
    pub secrets: HashMap<String, String>,
    pub secrets_warning: Option<String>,
}

impl EnvironmentRead {
    pub(super) fn new(
        file: EnvironmentFile,
        secrets: anyhow::Result<HashMap<String, String>>,
    ) -> Self {
        let (secrets, secrets_warning) = match secrets {
            Ok(values) => (values, None),
            Err(error) => (HashMap::new(), Some(error.to_string())),
        };
        Self {
            file,
            secrets,
            secrets_warning,
        }
    }

    pub(super) fn into_file(mut self) -> EnvironmentFile {
        for variable in &mut self.file.variables {
            if variable.secret {
                if let Some(value) = self.secrets.get(&variable.name) {
                    variable.value = Some(value.clone());
                }
            }
        }
        self.file
    }

    pub fn to_environment(&self, name: &str, workspace_name: &str) -> Environment {
        let label = if name == "globals" { "Globals" } else { name };
        self.file
            .to_environment(name, workspace_name, label, &self.secrets)
    }
}

#[derive(Clone)]
pub struct EnvironmentSnapshot {
    pub environments: Vec<Environment>,
    pub globals: Environment,
    pub corrupt_secrets_warning: Option<String>,
}

impl EnvironmentSnapshot {
    pub fn load(store: &dyn WorkspaceStore, workspace_name: &str) -> Self {
        let mut corrupt_secrets_warning = None;
        let mut load = |name: &str| {
            let read = store.read_environment(name).ok().flatten()?;
            if let Some(warning) = &read.secrets_warning {
                corrupt_secrets_warning = Some(warning.clone());
            }
            Some(read.to_environment(name, workspace_name))
        };
        let mut environments: Vec<_> = store
            .list_environments()
            .unwrap_or_default()
            .iter()
            .filter_map(|name| load(name))
            .collect();
        environments.sort_by_key(|environment| environment.name.to_lowercase());
        let globals = load("globals").unwrap_or_else(|| {
            EnvironmentFile::default().to_environment(
                "globals",
                workspace_name,
                "Globals",
                &HashMap::new(),
            )
        });
        Self {
            environments,
            globals,
            corrupt_secrets_warning,
        }
    }
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
