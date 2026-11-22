use super::WorkspaceEvent;
use crate::settings::AppSettings;
use gpui_kit::Context;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::http::ReqwestExecutor;
use zest_core::models::{
    Auth, Folder, KeyValuePair, Protocol, RequestItem, StoredCookie, Workspace,
};
use zest_core::storage::models::{FolderNode, WorkspaceMeta, WorkspaceTree};
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};
use zest_core::transport::{resolve_inherited_headers, InheritedHeader};

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
    /// Set while the initial workspace load (background scan) is in flight.
    load_in_flight: bool,
    /// A load was requested while one was already in flight; carries the
    /// `announce_opened` flag of the latest request for the follow-up.
    load_pending: Option<bool>,
    /// Set while a watcher-driven tree rescan runs on the background executor.
    rescan_in_flight: bool,
    /// A rescan was requested while one was already in flight; one follow-up
    /// scan runs after the current one completes.
    rescan_pending: bool,
    /// Lazily rebuilt id→position index for `find_request`; valid while
    /// `tree_version` is unchanged. Interior-mutable so the read-only
    /// `find_request` can rebuild it on demand.
    request_index: std::cell::RefCell<Option<(usize, HashMap<String, usize>)>>,
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
        Auth::Bearer { .. } => "Bearer Token",
        Auth::Basic { .. } => "Basic Auth",
        Auth::ApiKey { .. } => "API Key",
        Auth::Inherit => "Inherit",
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
            load_in_flight: false,
            load_pending: None,
            rescan_in_flight: false,
            rescan_pending: false,
            request_index: std::cell::RefCell::new(None),
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
            load_in_flight: false,
            load_pending: None,
            rescan_in_flight: false,
            rescan_pending: false,
            request_index: std::cell::RefCell::new(None),
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

    /// Cheap counts for the export modal: (exportable requests, folders,
    /// requests skipped because their protocol is not OpenAPI-representable).
    pub fn openapi_export_stats(&self) -> (usize, usize, usize) {
        self.openapi_export_stats_for(None)
    }

    /// Folder-scoped variant of [`Self::openapi_export_stats`]: counts only
    /// requests inside `folder_id`'s subtree. Folders are counted from the
    /// subtree too.
    pub fn openapi_export_stats_for(&self, folder_id: Option<&str>) -> (usize, usize, usize) {
        let scope_ids = folder_id.map(|id| self.folder_subtree_ids(id));
        let scoped_requests = self.requests_in_scope(scope_ids.as_deref());
        let exportable = scoped_requests
            .iter()
            .filter(|r| r.protocol.uses_http_transport())
            .count();
        // Exclude the export root itself from the folder count.
        let folders = scope_ids
            .as_ref()
            .map_or(self.folders.len(), |ids| ids.len() - 1);
        (exportable, folders, scoped_requests.len() - exportable)
    }

    /// Builds an OpenAPI 3.0 export of the whole workspace as the input for
    /// `zest_core::exporters::requests_to_openapi`. Headers/auth are resolved
    /// to their effective values (workspace + folder chain inheritance) via
    /// `resolve_request_hierarchy`; WebSocket/gRPC requests are skipped by the
    /// exporter and reported as warnings. GraphQL requests are flattened to
    /// their HTTP transport shape first (same conversion as the send pipeline).
    pub fn build_openapi_export(&self) -> zest_core::exporters::OpenApiExport {
        self.build_openapi_export_for(None)
    }

    /// Folder-scoped variant of [`Self::build_openapi_export`]: only requests
    /// inside `folder_id`'s subtree are exported, tags become the folder chain
    /// relative to the export root, and the folder name is the document title.
    pub fn build_openapi_export_for(
        &self,
        folder_id: Option<&str>,
    ) -> zest_core::exporters::OpenApiExport {
        let scope_ids = folder_id.map(|id| self.folder_subtree_ids(id));
        let scoped_requests = self.requests_in_scope(scope_ids.as_deref());
        // Length of the export root's own ancestor chain; request chains share
        // this prefix, so skipping it keeps tags relative to the export root.
        let root_chain_len = folder_id.map(|id| self.get_folder_hierarchy(Some(id)).len());

        let exported: Vec<zest_core::importers::ImportedRequest> = scoped_requests
            .into_iter()
            .map(|req| {
                let effective = if req.protocol == Protocol::GraphQL {
                    req.to_graphql_http_item().unwrap_or_else(|_| req.clone())
                } else {
                    req.clone()
                };
                let (headers, auth) = self.resolve_request_hierarchy(&effective);
                let folder_path: Vec<String> = self
                    .get_folder_hierarchy(req.folder_id.as_deref())
                    .into_iter()
                    .skip(root_chain_len.unwrap_or(0))
                    .map(|f| f.name.clone())
                    .collect();
                zest_core::importers::ImportedRequest {
                    name: effective.name.clone(),
                    method: effective.method,
                    url: effective.url.clone(),
                    // Keep the original protocol so the exporter can warn on
                    // non-HTTP-transport requests.
                    protocol: req.protocol,
                    params: effective.params.clone(),
                    headers,
                    auth,
                    body: effective.body.clone(),
                    script: None,
                    examples: effective.examples.clone(),
                    folder_path,
                    // RequestItem carries no description field.
                    description: None,
                }
            })
            .collect();

        let (title, description) = match folder_id {
            Some(id) => (
                self.folders
                    .iter()
                    .find(|f| f.id == id)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "Folder".to_string()),
                None,
            ),
            None => match &self.active_workspace {
                Some(w) => (
                    w.name.clone(),
                    Some(w.description.clone()).filter(|d| !d.trim().is_empty()),
                ),
                None => ("Workspace".to_string(), None),
            },
        };
        zest_core::exporters::requests_to_openapi(&title, description.as_deref(), &exported)
    }

    /// Requests whose folder is in the given id set (workspace-scope when
    /// `scope_ids` is `None`).
    fn requests_in_scope(&self, scope_ids: Option<&[String]>) -> Vec<&RequestItem> {
        match scope_ids {
            None => self.requests.iter().collect(),
            Some(ids) => self
                .requests
                .iter()
                .filter(|r| {
                    r.folder_id
                        .as_deref()
                        .is_some_and(|fid| ids.iter().any(|id| id == fid))
                })
                .collect(),
        }
    }

    /// Ids of `folder_id` and all its descendant folders (cycle-safe walk over
    /// the flat folder list).
    fn folder_subtree_ids(&self, folder_id: &str) -> Vec<String> {
        let mut ids = vec![folder_id.to_string()];
        let mut i = 0;
        while i < ids.len() {
            let current = ids[i].clone();
            i += 1;
            let child_ids: Vec<String> = self
                .folders
                .iter()
                .filter(|f| f.parent_id.as_deref() == Some(current.as_str()))
                .map(|f| f.id.clone())
                .collect();
            for child in child_ids {
                if !ids.contains(&child) {
                    ids.push(child);
                }
            }
        }
        ids
    }

    pub fn reload_all(&mut self) {
        if let Ok(meta) = self.store.load_workspace_meta() {
            self.workspace_meta = meta;
        }
        let folder_name = self
            .workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace");
        let initial_name = if self.workspace_meta.name.trim().is_empty() {
            folder_name.to_string()
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

        self.pins = self.store.load_pins().unwrap_or_default();
        self.cookies = self
            .store
            .load_cookies()
            .ok()
            .and_then(|val| serde_json::from_value::<Vec<StoredCookie>>(val).ok())
            .unwrap_or_default();
        self.executor.restore_stored_cookies(&self.cookies);
        self.reload_tree();
    }

    /// Runs the full read-side scan of the workspace store (meta, pins,
    /// cookies, collection tree). Blocking fs + YAML work — must run on a
    /// background executor, never on the UI thread.
    pub(crate) fn scan_workspace_blocking(
        store: Arc<dyn WorkspaceStore>,
        folder_name: String,
    ) -> WorkspaceScan {
        WorkspaceScan {
            workspace_meta: store.load_workspace_meta().ok(),
            pins: store.load_pins().unwrap_or_default(),
            cookies: store
                .load_cookies()
                .ok()
                .and_then(|val| serde_json::from_value::<Vec<StoredCookie>>(val).ok())
                .unwrap_or_default(),
            tree: store.scan_tree().ok(),
            folder_name,
            store: store.clone(),
        }
    }

    /// Applies a background scan to in-memory state (meta, workspace shell,
    /// pins, cookies, collection tree). Scans for a workspace the user has
    /// already switched away from are dropped.
    pub(crate) fn apply_workspace_scan(&mut self, scan: WorkspaceScan) {
        if !Arc::ptr_eq(&scan.store, &self.store) {
            return;
        }
        if let Some(meta) = scan.workspace_meta {
            self.workspace_meta = meta;
        }
        let initial_name = if self.workspace_meta.name.trim().is_empty() {
            scan.folder_name
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

        self.pins = scan.pins;
        self.cookies = scan.cookies;
        self.executor.restore_stored_cookies(&self.cookies);
        if let Some(tree) = scan.tree {
            self.apply_tree(tree, HashMap::new());
        }
    }

    /// Loads the workspace off the UI thread: the blocking store scan runs on
    /// the background executor and the result is applied on the UI thread,
    /// followed by a `RequestsReloaded` announcement. With `announce_opened`,
    /// a `WorkspaceOpened` event (consumed by the open-workspace flow) fires
    /// once the real metadata is available. Loads requested while one is in
    /// flight coalesce into a single follow-up load.
    pub fn load_workspace_async(&mut self, cx: &mut Context<Self>, announce_opened: bool) {
        if self.load_in_flight {
            self.load_pending = Some(announce_opened);
            return;
        }
        self.load_in_flight = true;
        let store = self.store.clone();
        let folder_name = self
            .workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string();
        let workspace_path = self.workspace_path.clone();
        cx.spawn(async move |this, cx| {
            let scan = cx
                .background_executor()
                .spawn(async move { Self::scan_workspace_blocking(store, folder_name) })
                .await;
            this.update(cx, |ws, cx| {
                ws.load_in_flight = false;
                ws.apply_workspace_scan(scan);
                cx.emit(WorkspaceEvent::RequestsReloaded { req_ids: None });
                if announce_opened {
                    let workspace_name = ws
                        .active_workspace
                        .as_ref()
                        .map(|w| w.name.clone())
                        .unwrap_or_else(|| "Workspace".to_string());
                    cx.emit(WorkspaceEvent::WorkspaceOpened {
                        path: workspace_path,
                        workspace_name,
                        store: ws.store.clone(),
                    });
                }
                cx.notify();
                if let Some(announce) = std::mem::take(&mut ws.load_pending) {
                    ws.load_workspace_async(cx, announce);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Watcher-driven tree rescan: identical to [`Self::reload_tree_preserving`]
    /// but the blocking `scan_tree` runs on the background executor. Rescans
    /// requested while one is in flight coalesce into one follow-up scan.
    pub fn reload_tree_preserving_async(
        &mut self,
        dirty: std::collections::HashSet<String>,
        cx: &mut Context<Self>,
    ) {
        if self.rescan_in_flight {
            self.rescan_pending = true;
            return;
        }
        self.rescan_in_flight = true;
        let kept_requests = self.capture_kept_requests(&dirty);
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let tree = cx
                .background_executor()
                .spawn(async move { store.scan_tree() })
                .await;
            this.update(cx, |ws, cx| {
                ws.rescan_in_flight = false;
                if let Ok(tree) = tree {
                    ws.apply_tree(tree, kept_requests);
                }
                let rerun = std::mem::take(&mut ws.rescan_pending);
                cx.emit(WorkspaceEvent::RequestsReloaded { req_ids: None });
                cx.notify();
                if rerun {
                    ws.reload_tree_preserving_async(std::collections::HashSet::new(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn reload_tree(&mut self) {
        self.reload_tree_preserving(&std::collections::HashSet::new());
    }

    /// Rescans the collection hierarchy on disk while preserving the in-memory
    /// `RequestItem` for any request ID in `dirty` whose file still exists
    /// on disk. Prevents `TreeChanged` events from clobbering active typing in
    /// the request composer.
    pub fn reload_tree_preserving(&mut self, dirty: &std::collections::HashSet<String>) {
        let kept_requests = self.capture_kept_requests(dirty);

        if let Ok(tree) = self.store.scan_tree() {
            self.apply_tree(tree, kept_requests);
        }
    }

    /// Snapshot of the in-memory requests whose on-disk files are dirty, to
    /// be merged back after a tree rescan.
    fn capture_kept_requests(
        &self,
        dirty: &std::collections::HashSet<String>,
    ) -> HashMap<String, RequestItem> {
        if dirty.is_empty() {
            HashMap::new()
        } else {
            self.requests
                .iter()
                .filter(|r| dirty.contains(&r.id))
                .map(|r| (r.id.clone(), r.clone()))
                .collect()
        }
    }

    /// Applies a scanned collection tree to in-memory state, merging back
    /// `kept_requests` (dirty in-memory edits) over the scanned items.
    fn apply_tree(&mut self, tree: WorkspaceTree, mut kept_requests: HashMap<String, RequestItem>) {
        let ws_name = self.workspace_meta.name.clone();
        let total_reqs = tree.count_requests();
        let total_folders = tree.count_folders();
        let mut all_requests = Vec::with_capacity(total_reqs);
        let mut all_folders = Vec::with_capacity(total_folders);
        let mut errors = HashMap::new();

        for req_node in tree.root_requests {
            let mut item = req_node.file.into_item(&req_node.rel_path, &ws_name, None);
            item.last_status_code = req_node.last_status_code;
            all_requests.push(item);
            if let Some(err) = req_node.parse_error {
                errors.insert(req_node.rel_path, err);
            }
        }

        fn process_folder(
            folder_node: FolderNode,
            parent_id: Option<String>,
            ws_name: &str,
            all_folders: &mut Vec<Folder>,
            all_requests: &mut Vec<RequestItem>,
            errors: &mut HashMap<String, String>,
        ) {
            let mut f = Folder::new(ws_name, &folder_node.file.name);
            f.id = folder_node.rel_path.clone();
            f.parent_id = parent_id;
            f.headers = folder_node.file.headers;
            f.auth = folder_node.file.auth;
            all_folders.push(f);

            let folder_rel = folder_node.rel_path;
            for req_node in folder_node.requests {
                let mut item =
                    req_node
                        .file
                        .into_item(&req_node.rel_path, ws_name, Some(folder_rel.clone()));
                item.last_status_code = req_node.last_status_code;
                all_requests.push(item);
                if let Some(err) = req_node.parse_error {
                    errors.insert(req_node.rel_path, err);
                }
            }

            for sub in folder_node.subfolders {
                process_folder(
                    sub,
                    Some(folder_rel.clone()),
                    ws_name,
                    all_folders,
                    all_requests,
                    errors,
                );
            }
        }

        for f_node in tree.folders {
            process_folder(
                f_node,
                None,
                &ws_name,
                &mut all_folders,
                &mut all_requests,
                &mut errors,
            );
        }

        if !kept_requests.is_empty() {
            for req in &mut all_requests {
                if let Some(mut kept) = kept_requests.remove(&req.id) {
                    kept.folder_id = req.folder_id.clone();
                    kept.last_status_code = kept.last_status_code.or(req.last_status_code);
                    errors.remove(&req.id);
                    *req = kept;
                }
            }
        }

        self.requests = all_requests;
        self.folders = all_folders;
        self.parse_errors = errors;
        self.bump_tree_version();
    }

    pub fn find_request(&self, req_id: &str) -> Option<&RequestItem> {
        if req_id == "scratch" {
            return Some(&self.scratch_request);
        }
        if !self.request_index_is_fresh() {
            self.rebuild_request_index();
        }
        let pos = self
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
        self.request_index
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
        *self.request_index.borrow_mut() = Some((self.tree_version, map));
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

    pub fn get_folder_hierarchy(&self, folder_id: Option<&str>) -> Vec<&Folder> {
        let mut chain = Vec::new();
        let mut cur_id = folder_id.map(|s| s.to_string());
        while let Some(id) = cur_id {
            if let Some(folder) = self.folders.iter().find(|f| f.id == id) {
                chain.push(folder);
                cur_id = folder.parent_id.clone();
            } else {
                break;
            }
        }
        chain.reverse();
        chain
    }

    pub fn resolve_request_hierarchy(&self, req: &RequestItem) -> (Vec<KeyValuePair>, Auth) {
        let ws_headers = self
            .active_workspace
            .as_ref()
            .map(|w| w.headers.as_slice())
            .unwrap_or(&[]);
        let ws_auth = self
            .active_workspace
            .as_ref()
            .map(|w| &w.auth)
            .unwrap_or(&Auth::None);

        let folder_chain_refs = self.get_folder_hierarchy(req.folder_id.as_deref());
        let folder_chain: Vec<(&[KeyValuePair], &Auth)> = folder_chain_refs
            .iter()
            .map(|f| (f.headers.as_slice(), &f.auth))
            .collect();

        zest_core::transport::resolve_hierarchical_headers_and_auth(
            ws_headers,
            ws_auth,
            &folder_chain,
            &req.headers,
            &req.auth,
        )
    }

    /// Projects the headers a request inherits from workspace/folders using the exact
    /// send-time merge semantics (nearest ancestor wins per key, case-insensitive).
    /// Sources are display labels (`folder "X"` / `workspace "Y"`) so the UI can show
    /// where each merged value comes from.
    pub fn inherited_headers_for_request(&self, req: &RequestItem) -> Vec<InheritedHeader> {
        let ws_label = format!(
            "workspace \"{}\"",
            self.active_workspace
                .as_ref()
                .map(|w| w.name.as_str())
                .unwrap_or("Workspace")
        );
        let ws_headers = self
            .active_workspace
            .as_ref()
            .map(|w| w.headers.as_slice())
            .unwrap_or(&[]);
        let folder_chain_refs = self.get_folder_hierarchy(req.folder_id.as_deref());
        let chain_owned: Vec<(String, &[KeyValuePair])> = folder_chain_refs
            .iter()
            .map(|f| (format!("folder \"{}\"", f.name), f.headers.as_slice()))
            .collect();
        let chain: Vec<(&str, &[KeyValuePair])> = chain_owned
            .iter()
            .map(|(label, headers)| (label.as_str(), *headers))
            .collect();
        resolve_inherited_headers(&ws_label, ws_headers, &chain, &req.headers)
    }

    /// Resolves the auth a request inherits when its own auth is `Auth::Inherit`:
    /// nearest non-inherit folder auth, else workspace auth. Returns `None` when no
    /// ancestor configures any auth.
    pub fn inherited_auth_source(&self, folder_id: Option<&str>) -> Option<InheritedAuthInfo> {
        let folder_chain_refs = self.get_folder_hierarchy(folder_id);
        for f in folder_chain_refs.iter().rev() {
            if !f.auth.is_inherit() {
                return Some(InheritedAuthInfo {
                    source_name: f.name.clone(),
                    source_folder_id: Some(f.id.clone()),
                    auth: f.auth.clone(),
                });
            }
        }
        let ws_auth = self
            .active_workspace
            .as_ref()
            .map(|w| &w.auth)
            .unwrap_or(&Auth::None);
        if ws_auth.is_inherit() {
            return None;
        }
        let ws_name = self
            .active_workspace
            .as_ref()
            .map(|w| w.name.as_str())
            .unwrap_or("Workspace")
            .to_string();
        Some(InheritedAuthInfo {
            source_name: ws_name,
            source_folder_id: None,
            auth: ws_auth.clone(),
        })
    }

    pub fn get_inherited_auth_description(&self, folder_id: Option<&str>) -> String {
        match self.inherited_auth_source(folder_id) {
            Some(info) if info.source_folder_id.is_some() => format!(
                "Inherited from folder \"{}\" ({})",
                info.source_name,
                auth_kind_label(&info.auth)
            ),
            Some(info) => format!(
                "Inherited from workspace \"{}\" ({})",
                info.source_name,
                auth_kind_label(&info.auth)
            ),
            None => "No auth configured in parent hierarchy".to_string(),
        }
    }
}

#[cfg(test)]
mod openapi_export_scope_tests {
    use super::*;
    use zest_core::models::HttpMethod;

    /// Builds a minimal state (no GPUI context): a `users` folder containing an
    /// `auth` subfolder, requests spread across the tree, and one WebSocket
    /// request that the exporter must skip.
    fn state_with_tree() -> WorkspaceDataState {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = WorkspaceDataState {
            store: Arc::new(FsWorkspaceStore::new(tmp.path())),
            executor: Arc::new(ReqwestExecutor::new()),
            workspace_path: tmp.path().to_path_buf(),
            workspace_meta: WorkspaceMeta::default(),
            active_workspace: None,
            folders: vec![
                Folder {
                    id: "f-users".into(),
                    workspace_id: "ws".into(),
                    name: "users".into(),
                    parent_id: None,
                    sort_order: 0,
                    headers: Vec::new(),
                    auth: Auth::Inherit,
                },
                Folder {
                    id: "f-auth".into(),
                    workspace_id: "ws".into(),
                    name: "auth".into(),
                    parent_id: Some("f-users".into()),
                    sort_order: 0,
                    headers: Vec::new(),
                    auth: Auth::Inherit,
                },
            ],
            requests: vec![
                {
                    let mut r = RequestItem::new("ws", "List users", HttpMethod::Get, "/users");
                    r.folder_id = Some("f-users".into());
                    r
                },
                {
                    let mut r = RequestItem::new("ws", "Login", HttpMethod::Post, "/auth/login");
                    r.folder_id = Some("f-auth".into());
                    r
                },
                RequestItem::new("ws", "Root ping", HttpMethod::Get, "/ping"),
                {
                    let mut r = RequestItem::new_with_protocol(
                        "ws",
                        "Live feed",
                        HttpMethod::Get,
                        "/feed",
                        Protocol::WebSocket,
                    );
                    r.folder_id = Some("f-auth".into());
                    r
                },
            ],
            scratch_request: RequestItem::new("ws", "scratch", HttpMethod::Get, "/"),
            parse_errors: HashMap::new(),
            pins: Vec::new(),
            cookies: Vec::new(),
            recent_workspaces: Vec::new(),
            tree_version: 0,
            url_version: 0,
            load_in_flight: false,
            load_pending: None,
            rescan_in_flight: false,
            rescan_pending: false,
            request_index: std::cell::RefCell::new(None),
        };
        // Keep the tempdir alive for the whole test run by leaking it: the
        // store never touches disk during export (pure in-memory projection).
        std::mem::forget(tmp);
        state
    }

    #[test]
    fn folder_export_includes_subtree_with_relative_tags() {
        let state = state_with_tree();
        let export = state.build_openapi_export_for(Some("f-users"));
        let paths = export.document["paths"].as_object().expect("paths");
        // Subtree requests only: the workspace-root request is excluded.
        assert_eq!(paths.len(), 2);
        assert!(paths.contains_key("/users"));
        assert!(paths.contains_key("/auth/login"));
        // Tags are relative to the export root: direct child has none, deeper
        // requests carry the subfolder chain only.
        assert!(export.document["paths"]["/users"]["get"]["tags"].is_null());
        assert_eq!(
            export.document["paths"]["/auth/login"]["post"]["tags"],
            serde_json::json!(["auth"])
        );
        assert_eq!(export.document["info"]["title"], "users");
        // The WebSocket request is skipped with a warning, not exported.
        assert!(export.warnings.iter().any(|w| w.contains("Live feed")));
    }

    #[test]
    fn folder_export_stats_count_subtree_only() {
        let state = state_with_tree();
        // (exportable, folders, skipped) inside the `users` subtree.
        assert_eq!(state.openapi_export_stats_for(Some("f-users")), (2, 1, 1));
        // Whole-workspace stats are unchanged by folder scoping.
        assert_eq!(state.openapi_export_stats(), (3, 2, 1));
    }
}
