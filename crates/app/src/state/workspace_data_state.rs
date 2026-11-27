use super::WorkspaceEvent;
use crate::settings::AppSettings;
use gpui_kit::Context;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::http::ReqwestExecutor;
use zest_core::models::{Auth, Folder, KeyValuePair, RequestItem, StoredCookie, Workspace};
use zest_core::storage::models::{WorkspaceMeta, WorkspaceTree};
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

pub struct WorkspaceDataState {
    pub store: Arc<dyn WorkspaceStore>,
    pub executor: Arc<ReqwestExecutor>,
    pub workspace_path: PathBuf,
    pub workspace_meta: WorkspaceMeta,
    pub active_workspace: Option<Workspace>,
    pub folders: Vec<Folder>,
    pub requests: Vec<RequestItem>,
    pub scratch_request: RequestItem,
    pub parse_errors: HashMap<String, String>,
    pub pins: Vec<String>,
    pub cookies: Vec<StoredCookie>,
    pub recent_workspaces: Vec<String>,
    /// Monotonically increasing counter bumped whenever the sidebar tree structure,
    /// request names/methods/status codes/examples, pins, or parse errors change.
    pub tree_version: usize,
    /// Monotonically increasing counter bumped when a request URL changes (only affects
    /// the sidebar tree when a non-empty search filter query is active).
    pub url_version: usize,
    /// Background scan coalescing flags and lazily rebuilt request-id index.
    pub(super) scan: WorkspaceScanState,
}

#[derive(Default)]
pub(super) struct WorkspaceScanState {
    /// Set while the initial workspace load (background scan) is in flight.
    pub(super) load_in_flight: bool,
    /// A load was requested while one was already in flight; carries the
    /// `announce_opened` flag of the latest request for the follow-up.
    pub(super) load_pending: Option<bool>,
    /// Set while a watcher-driven tree rescan runs on the background executor.
    pub(super) rescan_in_flight: bool,
    /// A rescan was requested while one was already in flight; one follow-up
    /// scan runs after the current one completes.
    pub(super) rescan_pending: bool,
    /// Lazily rebuilt id→position index for `find_request`; valid while
    /// `tree_version` is unchanged. Interior-mutable so the read-only
    /// `find_request` can rebuild it on demand.
    pub(super) request_index: std::cell::RefCell<Option<(usize, HashMap<String, usize>)>>,
}

/// Read-only snapshot of everything the workspace store holds. Produced by
/// [`WorkspaceDataState::scan_workspace_blocking`] on a background thread and
/// applied on the UI thread via [`WorkspaceDataState::apply_workspace_scan`].
pub(crate) struct WorkspaceScan {
    pub(crate) workspace_meta: Option<WorkspaceMeta>,
    pub(crate) pins: Vec<String>,
    pub(crate) cookies: Vec<StoredCookie>,
    pub(crate) tree: Option<WorkspaceTree>,
    pub(crate) folder_name: String,
    /// Identity of the scanned store: a scan for a workspace the user has
    /// already switched away from is dropped on apply.
    pub(crate) store: Arc<dyn WorkspaceStore>,
}

fn default_workspace_dir() -> PathBuf {
    dirs::document_dir()
        .map(|d| d.join("zest"))
        .or_else(|| dirs::home_dir().map(|h| h.join("Documents").join("zest")))
        .unwrap_or_else(|| PathBuf::from("./zest"))
}

/// The auth a request would effectively use when its own auth is `Auth::Inherit`,
/// together with the scope (folder or workspace) that supplies it.
#[derive(Debug, Clone, PartialEq)]
pub struct InheritedAuthInfo {
    pub source_name: String,
    /// `None` when the auth comes from the workspace scope.
    pub source_folder_id: Option<String>,
    pub auth: Auth,
}

/// Human-readable label for an `Auth` variant, shared by banner/card renderers.
pub fn auth_kind_label(auth: &Auth) -> &'static str {
    match auth {
        Auth::None => "No Auth",
        other => other.display_label(),
    }
}

impl WorkspaceDataState {
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
        // Note: `.gitignore` upkeep shells out to `git` (up to 30s) and the
        // full workspace scan is O(tree); both run off the UI thread —
        // `AppState::new` spawns `ensure_gitignore` and `load_workspace_async`.

        Self {
            store,
            executor: Arc::new(ReqwestExecutor::new()),
            workspace_path: ws_dir,
            workspace_meta: WorkspaceMeta::default(),
            active_workspace: Some(Workspace::new(&default_name)),
            folders: Vec::new(),
            requests: Vec::new(),
            scratch_request: RequestItem::new(
                "scratch",
                "Untitled Request",
                zest_core::models::HttpMethod::Get,
                "",
            ),
            parse_errors: HashMap::new(),
            pins: Vec::new(),
            cookies: Vec::new(),
            recent_workspaces: settings.recent_workspaces,
            tree_version: 0,
            url_version: 0,
            scan: WorkspaceScanState::default(),
        }
    }

    pub fn new_for_path(path: PathBuf, store: Arc<dyn WorkspaceStore>) -> Self {
        let mut state = Self {
            store,
            executor: Arc::new(ReqwestExecutor::new()),
            workspace_path: path,
            workspace_meta: WorkspaceMeta::default(),
            active_workspace: Some(Workspace::new("Test Workspace")),
            folders: Vec::new(),
            requests: Vec::new(),
            scratch_request: RequestItem::new(
                "scratch",
                "Untitled Request",
                zest_core::models::HttpMethod::Get,
                "",
            ),
            parse_errors: HashMap::new(),
            pins: Vec::new(),
            cookies: Vec::new(),
            recent_workspaces: Vec::new(),
            tree_version: 0,
            url_version: 0,
            scan: WorkspaceScanState::default(),
        };
        state.reload_all();
        state
    }

    pub fn bump_tree_version(&mut self) {
        self.tree_version = self.tree_version.wrapping_add(1);
    }

    pub fn open_workspace(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = path.canonicalize().unwrap_or(path);
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(&path));
        if !path.join("workspace.yaml").exists() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Workspace");
            let _ = store.init_workspace(name);
        }
        // .gitignore upkeep shells out to `git` (up to 30s) — background it.
        {
            let s = store.clone();
            cx.background_executor()
                .spawn(async move {
                    let _ = s.ensure_gitignore();
                })
                .detach();
        }
        self.store = store;
        self.workspace_path = path.clone();

        let mut settings = AppSettings::load();
        settings.add_recent_workspace(&path.to_string_lossy());
        self.recent_workspaces = settings.recent_workspaces;

        // Drop the previous workspace's data immediately (the scan is async);
        // `WorkspaceOpened` fires once the new scan lands so consumers see a
        // populated workspace.
        self.workspace_meta = WorkspaceMeta::default();
        self.folders.clear();
        self.requests.clear();
        self.parse_errors.clear();
        self.active_workspace = Some(Workspace::new(
            self.workspace_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Workspace"),
        ));
        self.bump_tree_version();

        self.load_workspace_async(cx, true);
        cx.notify();
    }

    pub fn new_workspace(&mut self, path: PathBuf, name: &str, cx: &mut Context<Self>) {
        let path = path.canonicalize().unwrap_or(path);
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(&path));
        let _ = store.init_workspace(name);
        self.open_workspace(path, cx);
    }

    pub fn execute_import_current_workspace(
        &mut self,
        result: zest_core::importers::ImportResult,
        options: zest_core::importers::ImportOptions,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<zest_core::importers::ImportSummary> {
        let summary = zest_core::importers::apply_import_to_workspace(
            self.store.as_ref(),
            &result,
            &options,
        )?;
        self.reload_tree();
        cx.emit(WorkspaceEvent::RequestsReloaded { req_ids: None });
        cx.notify();
        Ok(summary)
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

    pub fn find_request(&self, req_id: &str) -> Option<&RequestItem> {
        if req_id == "scratch" {
            return Some(&self.scratch_request);
        }
        if !self.request_index_is_fresh() {
            self.rebuild_request_index();
        }
        let pos = self
            .scan
            .request_index
            .borrow()
            .as_ref()
            .and_then(|(_, map)| map.get(req_id))
            .copied()?;
        self.requests.get(pos)
    }

    pub fn find_request_mut(&mut self, req_id: &str) -> Option<&mut RequestItem> {
        if req_id == "scratch" {
            return Some(&mut self.scratch_request);
        }
        if !self.request_index_is_fresh() {
            self.rebuild_request_index();
        }
        let pos = self
            .scan
            .request_index
            .borrow()
            .as_ref()
            .and_then(|(_, map)| map.get(req_id))
            .copied()?;
        self.requests.get_mut(pos)
    }

    /// The id→position index stays valid while `tree_version` is unchanged:
    /// every mutation of the request list bumps it (the sidebar flat-cache
    /// relies on the same contract).
    fn request_index_is_fresh(&self) -> bool {
        self.scan
            .request_index
            .borrow()
            .as_ref()
            .map(|(version, _)| *version == self.tree_version)
            .unwrap_or(false)
    }

    fn rebuild_request_index(&self) {
        // First occurrence wins to match the previous linear-scan semantics.
        let mut map = HashMap::with_capacity(self.requests.len());
        for (i, r) in self.requests.iter().enumerate() {
            map.entry(r.id.clone()).or_insert(i);
        }
        *self.scan.request_index.borrow_mut() = Some((self.tree_version, map));
    }

    pub fn mutate_request<F, R>(&mut self, req_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut RequestItem) -> R,
    {
        let is_scratch = req_id == "scratch";
        let mut tree_changed = false;
        let mut url_changed = false;
        let res = if let Some(r) = self.find_request_mut(req_id) {
            let old_method = r.method;
            let old_name = r.name.clone();
            let old_status = r.last_status_code;
            let old_ex_len = r.examples.len();
            let old_url = r.url.clone();
            let out = f(r);
            if !is_scratch {
                tree_changed = r.method != old_method
                    || r.name != old_name
                    || r.last_status_code != old_status
                    || r.examples.len() != old_ex_len;
                url_changed = r.url != old_url;
            }
            Some(out)
        } else {
            None
        };
        if tree_changed {
            self.bump_tree_version();
        }
        if url_changed {
            self.url_version = self.url_version.wrapping_add(1);
        }
        res
    }

    pub fn update_active_request<F>(&mut self, req_id: &str, f: F)
    where
        F: FnOnce(&mut RequestItem),
    {
        let _ = self.mutate_request(req_id, f);
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
        let _ = self.store.save_workspace_meta(&self.workspace_meta);
        cx.emit(WorkspaceEvent::WorkspaceMetaModified);
        cx.notify();
    }

    pub fn save_folder_settings(
        &mut self,
        folder_id: &str,
        headers: Vec<KeyValuePair>,
        auth: Auth,
        cx: &mut Context<Self>,
    ) {
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == folder_id) {
            f.headers = headers.clone();
            f.auth = auth.clone();
        }
        if let Ok(mut meta) = self.store.load_folder_meta(folder_id) {
            meta.headers = headers;
            meta.auth = auth;
            let _ = self.store.save_folder_meta(folder_id, &meta);
        }
        cx.emit(WorkspaceEvent::FolderMetaModified(folder_id.to_string()));
        cx.notify();
    }
}

#[cfg(test)]
#[path = "workspace_export_tests.rs"]
mod export_tests;
