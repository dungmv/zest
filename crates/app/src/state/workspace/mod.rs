//! Workspace-level metadata, lifecycle (open/new/rename), recent workspaces,
//! and workspace-scoped HTTP executor / cookie jar.

mod cookies;

use super::WorkspaceEvent;
use crate::services::workspace_io;
use crate::settings::AppSettings;
use gpui_kit::{Context, Task};
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::http::ReqwestExecutor;
use zest_core::models::{Auth, KeyValuePair, StoredCookie};
use zest_core::storage::models::WorkspaceMeta;
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

pub struct WorkspaceState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_path: PathBuf,
    workspace_meta: WorkspaceMeta,
    open_task: Option<Task<()>>,
    pub recent_workspaces: Vec<String>,
    pub executor: Arc<ReqwestExecutor>,
    pub cookies: Vec<StoredCookie>,
}

impl WorkspaceState {
    pub fn new(settings: AppSettings) -> Self {
        let ws_dir = workspace_io::startup_workspace(&settings);
        let store = Arc::new(FsWorkspaceStore::new(&ws_dir));

        Self {
            store,
            workspace_path: ws_dir,
            workspace_meta: WorkspaceMeta {
                name: String::new(),
                ..Default::default()
            },
            open_task: None,
            recent_workspaces: settings.recent_workspaces,
            executor: Arc::new(ReqwestExecutor::new()),
            cookies: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn new_for_path(path: PathBuf, store: Arc<dyn WorkspaceStore>) -> Self {
        let mut state = Self {
            store,
            workspace_path: path,
            workspace_meta: WorkspaceMeta::default(),
            open_task: None,
            recent_workspaces: Vec::new(),
            executor: Arc::new(ReqwestExecutor::new()),
            cookies: Vec::new(),
        };
        state.workspace_meta = state.store.load_workspace_meta().unwrap_or_default();
        state.cookies = workspace_io::read_cookies(state.store.as_ref());
        state.executor.restore_stored_cookies(&state.cookies);
        state
    }

    pub fn workspace_name(&self) -> &str {
        let name = self.workspace_meta.name.as_str();
        if name.trim().is_empty() {
            workspace_io::workspace_folder_name(&self.workspace_path)
        } else {
            name
        }
    }

    pub(crate) fn workspace_meta(&self) -> &WorkspaceMeta {
        &self.workspace_meta
    }

    /// Hydrates workspace metadata and cookies from a completed background scan.
    /// Scans for a workspace the user has already switched away from are dropped.
    pub fn apply_scan_result(
        &mut self,
        workspace_meta: Option<WorkspaceMeta>,
        cookies: Vec<StoredCookie>,
        store: &Arc<dyn WorkspaceStore>,
        cx: &mut Context<Self>,
    ) {
        if !Arc::ptr_eq(store, &self.store) {
            return;
        }
        if let Some(meta) = workspace_meta {
            self.workspace_meta = meta;
        }
        self.cookies = cookies;
        self.executor.restore_stored_cookies(&self.cookies);
        cx.notify();
    }

    pub(crate) fn apply_workspace_meta(&mut self, meta: WorkspaceMeta, cx: &mut Context<Self>) {
        self.workspace_meta = meta;
        cx.notify();
    }

    pub fn open_workspace(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.prepare_workspace(path, None, cx);
    }

    pub fn new_workspace(&mut self, path: PathBuf, name: &str, cx: &mut Context<Self>) {
        self.prepare_workspace(path, Some(name.to_string()), cx);
    }

    fn prepare_workspace(&mut self, path: PathBuf, name: Option<String>, cx: &mut Context<Self>) {
        self.open_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { workspace_io::prepare_workspace(path, name) })
                .await;
            let _ = this.update(cx, |state, cx| match result {
                Ok((path, store)) => state.request_workspace_switch(path, store, cx),
                Err(error) => {
                    crate::notify::error("Could Not Open Workspace", error.to_string(), cx)
                }
            });
        }));
    }

    pub(crate) fn open_cloned_workspace(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.open_task = None;
        let store = Arc::new(FsWorkspaceStore::new(&path));
        self.request_workspace_switch(path, store, cx);
    }

    fn request_workspace_switch(
        &self,
        path: PathBuf,
        store: Arc<dyn WorkspaceStore>,
        cx: &mut Context<Self>,
    ) {
        let folder_name = workspace_io::workspace_folder_name(&path).to_string();
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
        cx: &mut Context<Self>,
    ) {
        self.store = store.clone();
        self.workspace_path = path.clone();
        self.workspace_meta = WorkspaceMeta {
            name: String::new(),
            ..Default::default()
        };
        self.executor = Arc::new(ReqwestExecutor::new());
        self.cookies.clear();

        self.persist_recent_workspace(cx);
        cx.background_executor()
            .spawn(async move {
                let _ = store.ensure_gitignore();
            })
            .detach();
        cx.notify();
    }

    pub fn save_workspace_settings(
        &mut self,
        headers: Vec<KeyValuePair>,
        auth: Auth,
        cx: &mut Context<Self>,
    ) {
        self.workspace_meta.headers = headers;
        self.workspace_meta.auth = auth;
        cx.emit(WorkspaceEvent::WorkspaceMetaModified);
        cx.notify();
    }

    pub fn rename_workspace_internal(&mut self, new_name: &str) -> bool {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return false;
        }
        self.workspace_meta.name = trimmed.to_string();
        true
    }

    pub fn rename_workspace(&mut self, new_name: String, cx: &mut Context<Self>) {
        if self.rename_workspace_internal(&new_name) {
            cx.emit(WorkspaceEvent::WorkspaceMetaModified);
            cx.notify();
        }
    }

    pub fn open_workspace_folder(&self, cx: &mut Context<Self>) {
        let path = self.workspace_path.clone();
        cx.background_executor()
            .spawn(async move { workspace_io::open_workspace_folder(path) })
            .detach();
    }

    pub(crate) fn persist_recent_workspace(&mut self, cx: &mut Context<Self>) {
        let path = self.workspace_path.to_string_lossy().into_owned();
        if let Some(index) = self
            .recent_workspaces
            .iter()
            .position(|recent| recent == &path)
        {
            self.recent_workspaces.remove(index);
        }
        self.recent_workspaces.insert(0, path.clone());
        self.recent_workspaces.truncate(10);
        crate::settings::save_settings_change(cx, move |settings| {
            settings.record_recent_workspace(&path)
        });
    }

    pub fn clean_history(&mut self, cx: &mut Context<Self>) {
        self.recent_workspaces.clear();
        crate::settings::save_settings_change(cx, |settings| settings.recent_workspaces.clear());
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
