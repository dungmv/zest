//! Collection tree state (`CollectionState`): owns folders, requests, pins,
//! scratch request, parse errors, and background tree scanning.

mod folder_crud;
mod inheritance;
mod request_crud;
mod scanner;

pub use request_crud::edit_active_request;

use super::CollectionEvent;
use gpui_kit::Context;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::models::{Auth, Folder, KeyValuePair, RequestItem};
use zest_core::storage::WorkspaceStore;

/// Lightweight mirror of workspace-level inheritance scope (`name`, `headers`,
/// `auth`) kept in sync via `WorkspaceEvent` so collection-only views such as
/// `SidebarTree`, `RequestComposer`, and `AuthTab` never need `WorkspaceState`.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceScopeSnapshot {
    pub name: String,
    pub headers: Vec<KeyValuePair>,
    pub auth: Auth,
}

#[derive(Default)]
pub(super) struct CollectionScanState {
    pub(super) load_in_flight: bool,
    pub(super) load_pending: Option<bool>,
    pub(super) rescan_in_flight: bool,
    pub(super) rescan_pending: bool,
    pub(super) edits_during_rescan: std::collections::HashSet<String>,
    pub(super) request_index: std::cell::RefCell<Option<(usize, HashMap<String, usize>)>>,
}

pub struct CollectionState {
    pub store: Arc<dyn WorkspaceStore>,
    pub workspace_path: PathBuf,
    pub folders: Vec<Folder>,
    pub requests: Vec<RequestItem>,
    pub scratch_request: RequestItem,
    pub parse_errors: HashMap<String, String>,
    pub pins: Vec<String>,
    /// Monotonically increasing counter bumped whenever the sidebar tree structure,
    /// request names/methods/status codes/examples, pins, or parse errors change.
    pub tree_version: usize,
    /// Monotonically increasing counter bumped when a request URL changes (only affects
    /// the sidebar tree when a non-empty search filter query is active).
    pub url_version: usize,
    pub workspace_scope: WorkspaceScopeSnapshot,
    pub(super) scan: CollectionScanState,
}

impl CollectionState {
    pub fn new(
        store: Arc<dyn WorkspaceStore>,
        workspace_path: PathBuf,
        workspace_name: String,
    ) -> Self {
        Self {
            store,
            workspace_path,
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
            tree_version: 0,
            url_version: 0,
            workspace_scope: WorkspaceScopeSnapshot {
                name: workspace_name,
                headers: Vec::new(),
                auth: Auth::None,
            },
            scan: CollectionScanState::default(),
        }
    }

    pub fn new_for_path(path: PathBuf, store: Arc<dyn WorkspaceStore>) -> Self {
        let mut state = Self::new(store, path, "Test Workspace".to_string());
        state.reload_all_sync();
        state
    }

    pub fn bump_tree_version(&mut self) {
        self.tree_version = self.tree_version.wrapping_add(1);
    }

    pub fn set_workspace_scope(
        &mut self,
        name: String,
        headers: Vec<KeyValuePair>,
        auth: Auth,
        cx: &mut Context<Self>,
    ) {
        self.workspace_scope = WorkspaceScopeSnapshot {
            name,
            headers,
            auth,
        };
        cx.notify();
    }

    /// Switches to a new workspace store, clearing the previous tree immediately
    /// and kicking off an async background scan.
    pub fn switch_workspace(
        &mut self,
        store: Arc<dyn WorkspaceStore>,
        path: PathBuf,
        folder_name: String,
        cx: &mut Context<Self>,
    ) {
        self.scan = CollectionScanState::default();
        self.store = store;
        self.workspace_path = path;
        self.folders.clear();
        self.requests.clear();
        self.parse_errors.clear();
        self.pins.clear();
        self.scratch_request = RequestItem::new(
            "scratch",
            "Untitled Request",
            zest_core::models::HttpMethod::Get,
            "",
        );
        self.workspace_scope = WorkspaceScopeSnapshot {
            name: folder_name,
            headers: Vec::new(),
            auth: Auth::None,
        };
        self.bump_tree_version();
        self.load_workspace_async(cx, true);
        cx.notify();
    }

    pub(crate) fn finish_import(&mut self, environments: Vec<String>, cx: &mut Context<Self>) {
        cx.emit(CollectionEvent::ImportApplied(environments));
    }

    pub fn is_loading_workspace(&self) -> bool {
        self.scan.load_in_flight
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
        if self.scan.rescan_in_flight {
            self.scan.edits_during_rescan.insert(req_id.to_string());
        }
        self.requests.get_mut(pos)
    }

    fn request_index_is_fresh(&self) -> bool {
        self.scan
            .request_index
            .borrow()
            .as_ref()
            .map(|(version, _)| *version == self.tree_version)
            .unwrap_or(false)
    }

    fn rebuild_request_index(&self) {
        let mut map = HashMap::with_capacity(self.requests.len());
        for (idx, req) in self.requests.iter().enumerate() {
            map.insert(req.id.clone(), idx);
        }
        *self.scan.request_index.borrow_mut() = Some((self.tree_version, map));
    }

    /// Mutates a request by id and bumps `tree_version` / `url_version` if any
    /// sidebar-visible field changed.
    pub fn mutate_request<F, R>(&mut self, req_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut RequestItem) -> R,
    {
        let req = self.find_request_mut(req_id)?;
        let prev_name = req.name.clone();
        let prev_method = req.method;
        let prev_protocol = req.protocol;
        let prev_status = req.last_status_code;
        let prev_url = req.url.clone();
        let prev_examples_len = req.examples.len();

        let res = f(req);

        let tree_changed = req.name != prev_name
            || req.method != prev_method
            || req.protocol != prev_protocol
            || req.last_status_code != prev_status
            || req.examples.len() != prev_examples_len;
        let url_changed = req.url != prev_url;

        if tree_changed {
            self.bump_tree_version();
        }
        if url_changed {
            self.url_version = self.url_version.wrapping_add(1);
        }
        Some(res)
    }

    pub fn update_active_request<F>(&mut self, req_id: &str, f: F)
    where
        F: FnOnce(&mut RequestItem),
    {
        self.mutate_request(req_id, f);
    }

    pub fn save_folder_settings(
        &mut self,
        folder_id: &str,
        headers: Vec<KeyValuePair>,
        auth: Auth,
        cx: &mut Context<Self>,
    ) {
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == folder_id) {
            f.headers = headers;
            f.auth = auth;
        }
        cx.emit(CollectionEvent::FolderMetaModified(folder_id.to_string()));
        cx.notify();
    }
}
