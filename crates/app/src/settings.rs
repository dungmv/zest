use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_timeout_secs() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub recent_workspaces: Vec<String>,
    #[serde(default)]
    pub current_workspace: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default = "default_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_true")]
    pub follow_redirects: bool,
    #[serde(default = "default_true")]
    pub validate_tls: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            recent_workspaces: Vec::new(),
            current_workspace: None,
            theme: None,
            window_width: Some(1200.0),
            window_height: Some(800.0),
            request_timeout_secs: default_timeout_secs(),
            follow_redirects: true,
            validate_tls: true,
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_CONFIG_PATH: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_test_config_path(path: Option<PathBuf>) {
    TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = path);
}

impl AppSettings {
    pub fn config_file_path() -> PathBuf {
        if let Ok(dir) = std::env::var("ZEST_CONFIG_DIR") {
            return PathBuf::from(dir).join("settings.json");
        }
        #[cfg(test)]
        {
            if let Some(path) = TEST_CONFIG_PATH.with(|p| p.borrow().clone()) {
                return path;
            }
            std::env::temp_dir().join(format!("zest_test_settings_{}.json", std::process::id()))
        }
        #[cfg(not(test))]
        {
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("zest").join("settings.json")
            } else {
                PathBuf::from("./zest-settings.json")
            }
        }
    }

    pub fn normalize_path(path: &str) -> String {
        let p = std::path::Path::new(path);
        if let Ok(canon) = p.canonicalize() {
            canon.to_string_lossy().to_string()
        } else {
            let mut s = path.trim();
            while s.len() > 1 && (s.ends_with('/') || s.ends_with('\\')) {
                s = &s[..s.len() - 1];
            }
            s.to_string()
        }
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str::<Self>(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn add_recent_workspace(&mut self, path: &str) {
        let normalized = Self::normalize_path(path);
        self.recent_workspaces
            .retain(|p| Self::normalize_path(p) != normalized);
        self.recent_workspaces.insert(0, normalized.clone());
        if self.recent_workspaces.len() > 10 {
            self.recent_workspaces.truncate(10);
        }
        self.current_workspace = Some(normalized);
        let _ = self.save();
    }
}
