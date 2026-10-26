use crate::models::HttpMethod;
use crate::storage::models::{
    EnvironmentFile, FolderFile, LocFile, LocalState, RequestFile, WorkspaceMeta, WorkspaceTree,
};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Trait defining all storage operations for a workspace.
/// The UI depends ONLY on this trait.
pub trait WorkspaceStore: Send + Sync {
    /// Absolute path to workspace root
    fn workspace_root(&self) -> &Path;

    /// Initialize a new workspace on disk
    fn init_workspace(&self, name: &str) -> Result<()>;

    /// Check and repair .gitignore, returns true if sensitive files are tracked by Git
    fn ensure_gitignore(&self) -> Result<bool>;

    /// Workspace metadata
    fn load_workspace_meta(&self) -> Result<WorkspaceMeta>;
    fn save_workspace_meta(&self, meta: &WorkspaceMeta) -> Result<()>;

    /// Scan entire collection hierarchy
    fn scan_tree(&self) -> Result<WorkspaceTree>;

    /// Folder metadata
    fn load_folder_meta(&self, folder_rel: &str) -> Result<FolderFile>;
    fn save_folder_meta(&self, folder_rel: &str, meta: &FolderFile) -> Result<()>;

    /// Request CRUD
    fn read_request(&self, rel_path: &str) -> Result<RequestFile>;
    fn write_request(&self, rel_path: &str, req: &RequestFile) -> Result<()>;
    fn create_request(
        &self,
        folder_rel: Option<&str>,
        name: &str,
        method: HttpMethod,
        url: &str,
    ) -> Result<(String, String)>; // (slug, rel_path)
    fn delete_request(&self, rel_path: &str) -> Result<()>;
    fn rename_request(&self, rel_path: &str, new_name: &str) -> Result<String>; // new rel_path
    fn move_request(&self, rel_path: &str, target_folder_rel: Option<&str>) -> Result<String>;

    /// Group / Folder CRUD
    fn create_group(&self, parent_folder_rel: Option<&str>, name: &str)
        -> Result<(String, String)>; // (dir_name, rel_path)
    fn delete_group(&self, folder_rel: &str) -> Result<()>;
    fn rename_group(&self, folder_rel: &str, new_name: &str) -> Result<String>;
    fn move_group(&self, folder_rel: &str, target_parent_rel: Option<&str>) -> Result<String>;
    fn reorder_folder(&self, folder_rel: Option<&str>, order: &[String]) -> Result<()>;

    /// Environments & Secrets
    fn list_environments(&self) -> Result<Vec<String>>;
    fn load_environment(&self, name: &str) -> Result<EnvironmentFile>;
    fn save_environment(&self, name: &str, env: &EnvironmentFile) -> Result<()>;
    fn delete_environment(&self, name: &str) -> Result<()>;
    fn rename_environment(&self, old_name: &str, new_name: &str) -> Result<()>;
    fn load_secrets(&self, env_name: &str) -> Result<HashMap<String, String>>;
    fn save_secrets(&self, env_name: &str, secrets: &HashMap<String, String>) -> Result<()>;

    /// Globals & Globals Secrets
    fn load_global_environment(&self) -> Result<EnvironmentFile> {
        match self.load_environment("globals") {
            Ok(env) => Ok(env),
            Err(_) => Ok(EnvironmentFile::default()),
        }
    }
    fn save_global_environment(&self, env: &EnvironmentFile) -> Result<()> {
        self.save_environment("globals", env)
    }
    fn load_global_secrets(&self) -> Result<HashMap<String, String>> {
        self.load_secrets("globals")
    }
    fn save_global_secrets(&self, secrets: &HashMap<String, String>) -> Result<()> {
        self.save_secrets("globals", secrets)
    }

    /// Response Cache (.loc)
    fn load_response_cache(&self, rel_path: &str) -> Result<Option<LocFile>>;
    fn save_response_cache(&self, rel_path: &str, cache: &LocFile) -> Result<()>;

    /// .local files
    fn load_pins(&self) -> Result<Vec<String>>;
    fn save_pins(&self, pins: &[String]) -> Result<()>;
    fn load_state(&self) -> Result<LocalState>;
    fn save_state(&self, state: &LocalState) -> Result<()>;
    fn load_cookies(&self) -> Result<serde_json::Value>;
    fn save_cookies(&self, cookies: &serde_json::Value) -> Result<()>;

    /// Thread-safe set of paths currently suppressed from watcher notifications
    fn suppressed_paths(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<std::collections::HashSet<std::path::PathBuf>>>;
}
