//! Workspace-level metadata, lifecycle (open/new/rename), recent workspaces,
//! and workspace-scoped HTTP executor / cookie jar.

use super::WorkspaceEvent;
use crate::settings::AppSettings;
use gpui_kit::Context;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zest_core::http::ReqwestExecutor;
use zest_core::models::{Auth, KeyValuePair, StoredCookie, Workspace};
use zest_core::storage::models::WorkspaceMeta;
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

fn default_workspace_dir() -> PathBuf {
    dirs::document_dir()
        .map(|d| d.join("zest"))
        .or_else(|| dirs::home_dir().map(|h| h.join("Documents").join("zest")))
        .unwrap_or_else(|| PathBuf::from("./zest"))
}

fn workspace_folder_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Workspace")
        .to_owned()
}

pub struct WorkspaceState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_path: PathBuf,
    pub workspace_meta: WorkspaceMeta,
    pub active_workspace: Option<Workspace>,
    pub recent_workspaces: Vec<String>,
    pub executor: Arc<ReqwestExecutor>,
    pub cookies: Vec<StoredCookie>,
}

impl WorkspaceState {
    pub fn new() -> Self {
        let mut settings = AppSettings::load();
        let ws_dir = if let Some(curr) = &settings.current_workspace {
            let path = PathBuf::from(curr);
            if path.exists() {
                path.canonicalize().unwrap_or(path)
            } else {
                default_workspace_dir()
            }
        } else {
            default_workspace_dir()
        };
        let ws_dir = ws_dir.canonicalize().unwrap_or(ws_dir);

        let default_name = ws_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string();

        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(&ws_dir));
        if !ws_dir.join("workspace.yaml").exists() {
            let _ = store.init_workspace(&default_name);
        }

        settings.add_recent_workspace(&ws_dir.to_string_lossy());

        Self {
            store,
            workspace_path: ws_dir,
            workspace_meta: WorkspaceMeta::default(),
            active_workspace: Some(Workspace::new(&default_name)),
            recent_workspaces: settings.recent_workspaces,
            executor: Arc::new(ReqwestExecutor::new()),
            cookies: Vec::new(),
        }
    }

    pub fn new_for_path(path: PathBuf, store: Arc<dyn WorkspaceStore>) -> Self {
        let mut state = Self {
            store,
            workspace_path: path,
            workspace_meta: WorkspaceMeta::default(),
            active_workspace: Some(Workspace::new("Test Workspace")),
            recent_workspaces: Vec::new(),
            executor: Arc::new(ReqwestExecutor::new()),
            cookies: Vec::new(),
        };
        state.reload_meta_and_cookies();
        state
    }

    pub fn workspace_name(&self) -> &str {
        self.active_workspace
            .as_ref()
            .map(|w| w.name.as_str())
            .unwrap_or("Workspace")
    }

    pub fn workspace_description(&self) -> Option<&str> {
        self.active_workspace
            .as_ref()
            .map(|w| w.description.as_str())
            .filter(|d| !d.trim().is_empty())
    }

    pub fn reload_meta_and_cookies(&mut self) {
        if let Ok(meta) = self.store.load_workspace_meta() {
            self.workspace_meta = meta;
        }
        let folder_name = self
            .workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string();
        self.rebuild_active_workspace(folder_name);

        self.cookies = self
            .store
            .load_cookies()
            .ok()
            .and_then(|val| serde_json::from_value::<Vec<StoredCookie>>(val).ok())
            .unwrap_or_default();
        self.executor.restore_stored_cookies(&self.cookies);
    }

    /// Hydrates workspace metadata and cookies from a completed background scan.
    /// Scans for a workspace the user has already switched away from are dropped.
    pub fn apply_scan_result(
        &mut self,
        workspace_meta: Option<WorkspaceMeta>,
        cookies: Vec<StoredCookie>,
        folder_name: String,
        store: &Arc<dyn WorkspaceStore>,
        cx: &mut Context<Self>,
    ) {
        if !Arc::ptr_eq(store, &self.store) {
            return;
        }
        if let Some(meta) = workspace_meta {
            self.workspace_meta = meta;
        }
        self.rebuild_active_workspace(folder_name);
        self.cookies = cookies;
        self.executor.restore_stored_cookies(&self.cookies);
        cx.notify();
    }

    fn rebuild_active_workspace(&mut self, folder_name: String) {
        let initial_name = if self.workspace_meta.name.trim().is_empty() {
            folder_name
        } else {
            self.workspace_meta.name.clone()
        };
        self.active_workspace = Some(Workspace {
            id: self.workspace_path.to_string_lossy().to_string(),
            name: initial_name,
            description: String::new(),
            active_environment_id: None,
            headers: self.workspace_meta.headers.clone(),
            auth: self.workspace_meta.auth.clone(),
        });
    }

    pub fn open_workspace(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = path.canonicalize().unwrap_or(path);
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(&path));
        if !path.join("workspace.yaml").exists() {
            let _ = store.init_workspace(&workspace_folder_name(&path));
        }
        self.request_workspace_switch(path, store, cx);
    }

    pub(crate) fn open_cloned_workspace(&self, path: PathBuf, cx: &mut Context<Self>) {
        let store = Arc::new(FsWorkspaceStore::new(&path));
        self.request_workspace_switch(path, store, cx);
    }

    fn request_workspace_switch(
        &self,
        path: PathBuf,
        store: Arc<dyn WorkspaceStore>,
        cx: &mut Context<Self>,
    ) {
        let folder_name = workspace_folder_name(&path);
        cx.emit(WorkspaceEvent::WorkspaceSwitchRequested {
            path,
            store,
            folder_name,
        });
    }

    pub(crate) fn activate_workspace(
        &mut self,
        path: PathBuf,
        store: Arc<dyn WorkspaceStore>,
        folder_name: &str,
        cx: &mut Context<Self>,
    ) {
        self.store = store.clone();
        self.workspace_path = path.clone();
        self.workspace_meta = WorkspaceMeta::default();
        self.active_workspace = Some(Workspace::new(folder_name));
        self.executor = Arc::new(ReqwestExecutor::new());
        self.cookies.clear();

        let mut settings = AppSettings::load();
        settings.add_recent_workspace(&path.to_string_lossy());
        self.recent_workspaces = settings.recent_workspaces;

        cx.background_executor()
            .spawn(async move {
                let _ = store.ensure_gitignore();
            })
            .detach();
        cx.notify();
    }

    pub fn new_workspace(&mut self, path: PathBuf, name: &str, cx: &mut Context<Self>) {
        let path = path.canonicalize().unwrap_or(path);
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(&path));
        let _ = store.init_workspace(name);
        self.open_workspace(path, cx);
    }

    pub fn execute_import_new_workspace(
        &mut self,
        target_dir: PathBuf,
        workspace_name: &str,
        result: zest_core::importers::ImportResult,
        options: zest_core::importers::ImportOptions,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<zest_core::importers::ImportSummary> {
        let new_store = Arc::new(FsWorkspaceStore::new(&target_dir));
        new_store.init_workspace(workspace_name)?;
        let summary =
            zest_core::importers::apply_import_to_workspace(new_store.as_ref(), &result, &options)?;
        self.open_workspace(target_dir, cx);
        Ok(summary)
    }

    pub fn save_workspace_settings(
        &mut self,
        headers: Vec<KeyValuePair>,
        auth: Auth,
        cx: &mut Context<Self>,
    ) {
        self.workspace_meta.headers = headers.clone();
        self.workspace_meta.auth = auth.clone();
        if let Some(ws) = &mut self.active_workspace {
            ws.headers = headers;
            ws.auth = auth;
        }
        cx.emit(WorkspaceEvent::WorkspaceMetaModified);
        cx.notify();
    }

    pub fn rename_workspace_internal(&mut self, new_name: &str) -> bool {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return false;
        }
        self.workspace_meta.name = trimmed.to_string();
        let _ = self.store.save_workspace_meta(&self.workspace_meta);
        if let Some(ws) = &mut self.active_workspace {
            ws.name = trimmed.to_string();
        }
        true
    }

    pub fn rename_workspace(&mut self, new_name: String, cx: &mut Context<Self>) {
        if self.rename_workspace_internal(&new_name) {
            cx.emit(WorkspaceEvent::WorkspaceMetaModified);
            cx.notify();
        }
    }

    pub fn open_workspace_folder(&self) {
        let path = &self.workspace_path;
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("explorer").arg(path).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(path).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }

    pub fn clean_history_internal(&mut self) {
        self.recent_workspaces.clear();
        let mut settings = AppSettings::load();
        settings.recent_workspaces.clear();
        let _ = settings.save();
    }

    pub fn clean_history(&mut self, cx: &mut Context<Self>) {
        self.clean_history_internal();
        cx.notify();
    }

    /// Refreshes the in-memory cookie snapshot from the executor without
    /// touching disk. Returns the snapshot so the caller can persist it on
    /// the background executor (disk writes must not run on the UI thread).
    pub fn sync_cookies_in_memory(&mut self) -> Vec<StoredCookie> {
        self.cookies = self.executor.get_stored_cookies();
        self.cookies.clone()
    }

    pub fn sync_cookies_from_executor(&mut self) {
        let cookies = self.sync_cookies_in_memory();
        if let Ok(val) = serde_json::to_value(&cookies) {
            let _ = self.store.save_cookies(&val);
        }
    }

    pub fn remove_cookie_internal(&mut self, domain: &str, path: &str, name: &str) {
        self.executor.remove_cookie(domain, path, name);
        self.sync_cookies_from_executor();
    }

    pub fn remove_cookie(&mut self, domain: &str, path: &str, name: &str, cx: &mut Context<Self>) {
        self.remove_cookie_internal(domain, path, name);
        cx.notify();
    }

    pub fn clear_cookies_internal(&mut self) {
        self.executor.clear_cookies();
        self.cookies.clear();
        let _ = self.store.save_cookies(&serde_json::json!([]));
    }

    pub fn clear_cookies(&mut self, cx: &mut Context<Self>) {
        self.clear_cookies_internal();
        cx.notify();
    }
}
