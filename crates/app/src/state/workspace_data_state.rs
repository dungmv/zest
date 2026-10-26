use crate::settings::AppSettings;
use gpui::Context;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::http::{resolve_inherited_headers, InheritedHeader, ReqwestExecutor};
use zest_core::models::{Auth, Folder, HttpRequestItem, KeyValuePair, StoredCookie, Workspace};
use zest_core::storage::models::{FolderNode, WorkspaceMeta};
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

pub struct WorkspaceDataState {
    pub store: Arc<dyn WorkspaceStore>,
    pub executor: Arc<ReqwestExecutor>,
    pub workspace_path: PathBuf,
    pub workspace_meta: WorkspaceMeta,
    pub active_workspace: Option<Workspace>,
    pub folders: Vec<Folder>,
    pub requests: Vec<HttpRequestItem>,
    pub scratch_request: HttpRequestItem,
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
                path
            } else {
                default_workspace_dir()
            }
        } else {
            default_workspace_dir()
        };

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
        let _ = store.ensure_gitignore();

        let mut state = Self {
            store,
            executor: Arc::new(ReqwestExecutor::new()),
            workspace_path: ws_dir,
            workspace_meta: WorkspaceMeta::default(),
            active_workspace: Some(Workspace::new(&default_name)),
            folders: Vec::new(),
            requests: Vec::new(),
            scratch_request: HttpRequestItem::new(
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
        };

        state.reload_all();
        state
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
            scratch_request: HttpRequestItem::new(
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
        };
        state.reload_all();
        state
    }

    pub fn bump_tree_version(&mut self) {
        self.tree_version = self.tree_version.wrapping_add(1);
    }

    pub fn open_workspace(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(&path));
        if !path.join("workspace.yaml").exists() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Workspace");
            let _ = store.init_workspace(name);
        }
        let _ = store.ensure_gitignore();
        self.store = store;
        self.workspace_path = path.clone();

        let mut settings = AppSettings::load();
        settings.add_recent_workspace(&path.to_string_lossy());
        self.recent_workspaces = settings.recent_workspaces;

        self.reload_all();
        cx.notify();
    }

    pub fn new_workspace(&mut self, path: PathBuf, name: &str, cx: &mut Context<Self>) {
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

    pub fn reload_tree(&mut self) {
        if let Ok(tree) = self.store.scan_tree() {
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
                all_requests: &mut Vec<HttpRequestItem>,
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
                    let mut item = req_node.file.into_item(
                        &req_node.rel_path,
                        ws_name,
                        Some(folder_rel.clone()),
                    );
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

            self.requests = all_requests;
            self.folders = all_folders;
            self.parse_errors = errors;
            self.bump_tree_version();
        }
    }

    pub fn find_request(&self, req_id: &str) -> Option<&HttpRequestItem> {
        if req_id == "scratch" {
            Some(&self.scratch_request)
        } else {
            self.requests.iter().find(|r| r.id == req_id)
        }
    }

    pub fn find_request_mut(&mut self, req_id: &str) -> Option<&mut HttpRequestItem> {
        if req_id == "scratch" {
            Some(&mut self.scratch_request)
        } else {
            self.requests.iter_mut().find(|r| r.id == req_id)
        }
    }

    pub fn mutate_request<F, R>(&mut self, req_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut HttpRequestItem) -> R,
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
        F: FnOnce(&mut HttpRequestItem),
    {
        let _ = self.mutate_request(req_id, f);
    }

    pub fn save_workspace_scope_settings(
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
        cx.notify();
    }

    pub fn save_folder_scope_settings(
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

    pub fn resolve_request_hierarchy(&self, req: &HttpRequestItem) -> (Vec<KeyValuePair>, Auth) {
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

        zest_core::http::resolve_hierarchical_headers_and_auth(
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
    pub fn inherited_headers_for_request(&self, req: &HttpRequestItem) -> Vec<InheritedHeader> {
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
