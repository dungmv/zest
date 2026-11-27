//! Async background scanning and watcher-driven tree reconciliation for [`CollectionState`].

use super::{CollectionState, WorkspaceScopeSnapshot};
use crate::state::{CollectionEvent, EnvironmentSnapshot, PersistenceState};
use gpui_kit::{Context, WeakEntity};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use zest_core::models::{Folder, RequestItem, StoredCookie};
use zest_core::storage::models::{FolderNode, LocalState, WorkspaceMeta, WorkspaceTree};
use zest_core::storage::WorkspaceStore;

/// Read-only snapshot of everything the workspace store holds. Produced by
/// [`CollectionState::scan_workspace_blocking`] on a background thread and
/// applied on the UI thread.
pub(crate) struct WorkspaceScan {
    pub(crate) workspace_meta: Option<WorkspaceMeta>,
    local_state: LocalState,
    environment: EnvironmentSnapshot,
    pub(crate) pins: Vec<String>,
    pub(crate) cookies: Vec<StoredCookie>,
    pub(crate) tree: Option<WorkspaceTree>,
    pub(crate) folder_name: String,
    pub(crate) store: Arc<dyn WorkspaceStore>,
}

impl CollectionState {
    /// Synchronous reload used by unit tests and `new_for_path`.
    pub fn reload_all_sync(&mut self) {
        if let Ok(meta) = self.store.load_workspace_meta() {
            let folder_name = self
                .workspace_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Workspace");
            let name = if meta.name.trim().is_empty() {
                folder_name.to_string()
            } else {
                meta.name.clone()
            };
            self.workspace_scope = WorkspaceScopeSnapshot {
                name,
                headers: meta.headers,
                auth: meta.auth,
            };
        }
        self.pins = self.store.load_pins().unwrap_or_default();
        self.reload_tree();
    }

    pub(crate) fn scan_workspace_blocking(
        store: Arc<dyn WorkspaceStore>,
        folder_name: String,
    ) -> WorkspaceScan {
        let workspace_meta = store.load_workspace_meta().ok();
        let workspace_name = workspace_meta
            .as_ref()
            .map(|meta| meta.name.as_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&folder_name);
        let environment = EnvironmentSnapshot::load(store.as_ref(), workspace_name);
        WorkspaceScan {
            workspace_meta,
            local_state: store.load_state().unwrap_or_default(),
            environment,
            pins: store.load_pins().unwrap_or_default(),
            cookies: store
                .load_cookies()
                .ok()
                .and_then(|val| serde_json::from_value::<Vec<StoredCookie>>(val).ok())
                .unwrap_or_default(),
            tree: store.scan_tree().ok(),
            folder_name,
            store,
        }
    }

    /// Applies collection-side fields of a background scan (`workspace_scope`,
    /// `pins`, `tree`). Returns `false` if the scan belongs to an obsolete store.
    pub(crate) fn apply_collection_scan(&mut self, scan: &mut WorkspaceScan) -> bool {
        if !Arc::ptr_eq(&scan.store, &self.store) {
            return false;
        }
        if let Some(meta) = &scan.workspace_meta {
            let name = if meta.name.trim().is_empty() {
                scan.folder_name.clone()
            } else {
                meta.name.clone()
            };
            self.workspace_scope = WorkspaceScopeSnapshot {
                name,
                headers: meta.headers.clone(),
                auth: meta.auth.clone(),
            };
        }
        self.pins = std::mem::take(&mut scan.pins);
        if let Some(tree) = scan.tree.take() {
            self.apply_tree(tree, HashMap::new());
        }
        true
    }

    /// Loads the workspace off the UI thread and emits `WorkspaceScanLoaded` +
    /// `RequestsReloaded` when complete. Coalesces overlapping requests.
    pub fn load_workspace_async(&mut self, cx: &mut Context<Self>, announce_opened: bool) {
        if self.scan.load_in_flight {
            self.scan.load_pending = Some(announce_opened);
            return;
        }
        self.scan.load_in_flight = true;
        let store = self.store.clone();
        let folder_name = self
            .workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string();
        let workspace_path = self.workspace_path.clone();
        cx.spawn(async move |this, cx| {
            let mut scan = cx
                .background_executor()
                .spawn(async move { Self::scan_workspace_blocking(store, folder_name) })
                .await;
            this.update(cx, |col, cx| {
                if !Arc::ptr_eq(&scan.store, &col.store) {
                    return;
                }
                col.scan.load_in_flight = false;
                if col.apply_collection_scan(&mut scan) {
                    cx.emit(CollectionEvent::WorkspaceScanLoaded {
                        workspace_meta: scan.workspace_meta.take(),
                        local_state: Box::new(std::mem::take(&mut scan.local_state)),
                        environment: Box::new(scan.environment),
                        cookies: std::mem::take(&mut scan.cookies),
                        folder_name: scan.folder_name.clone(),
                        store: scan.store.clone(),
                        announce_opened,
                        workspace_path,
                    });
                    cx.emit(CollectionEvent::RequestsReloaded { req_ids: None });
                    cx.notify();
                }
                if let Some(announce) = std::mem::take(&mut col.scan.load_pending) {
                    col.load_workspace_async(cx, announce);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Watcher-driven tree rescan off the UI thread, preserving dirty in-memory
    /// requests. Coalesces overlapping rescans.
    pub fn reload_tree_preserving_async(
        &mut self,
        persistence: WeakEntity<PersistenceState>,
        dirty_at_start: HashSet<String>,
        cx: &mut Context<Self>,
    ) {
        if self.scan.rescan_in_flight {
            self.scan.rescan_pending = true;
            return;
        }
        self.scan.rescan_in_flight = true;
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let scan_store = store.clone();
            let tree = cx
                .background_executor()
                .spawn(async move { scan_store.scan_tree() })
                .await;
            this.update(cx, |col, cx| {
                if !Arc::ptr_eq(&store, &col.store) {
                    return;
                }
                col.scan.rescan_in_flight = false;
                let mut preserved = dirty_at_start;
                preserved.extend(std::mem::take(&mut col.scan.edits_during_rescan));
                let current_dirty = persistence
                    .upgrade()
                    .map(|persistence| persistence.read(cx).dirty_request_ids())
                    .unwrap_or_default();
                preserved.extend(current_dirty.iter().cloned());
                if let Ok(tree) = tree {
                    col.apply_tree(tree, col.capture_kept_requests(&preserved));
                    cx.emit(CollectionEvent::RequestsReloaded { req_ids: None });
                    cx.notify();
                }
                if std::mem::take(&mut col.scan.rescan_pending) {
                    col.reload_tree_preserving_async(persistence, current_dirty, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn reload_tree(&mut self) {
        self.reload_tree_preserving(&HashSet::new());
    }

    pub fn reload_tree_preserving(&mut self, dirty: &HashSet<String>) {
        let kept_requests = self.capture_kept_requests(dirty);
        if let Ok(tree) = self.store.scan_tree() {
            self.apply_tree(tree, kept_requests);
        }
    }

    fn capture_kept_requests(&self, dirty: &HashSet<String>) -> HashMap<String, RequestItem> {
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

    fn apply_tree(&mut self, tree: WorkspaceTree, mut kept_requests: HashMap<String, RequestItem>) {
        let ws_name = self.workspace_scope.name.clone();
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
}
