//! Async background scanning and watcher-driven tree reconciliation for [`CollectionState`].

use super::{CollectionState, WorkspaceScopeSnapshot};
use crate::services::{
    workspace_io::{self, WorkspaceScan},
    workspace_watcher::RequestReload,
};
use crate::state::{CollectionEvent, PersistenceState};
use gpui_kit::{Context, WeakEntity};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use zest_core::models::RequestItem;
use zest_core::storage::collection::CollectionSnapshot;

impl CollectionState {
    /// Synchronous reload used by unit tests and `new_for_path`.
    #[cfg(test)]
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
        let disk_revision = self.disk_revision();
        cx.spawn(async move |this, cx| {
            let mut scan = cx
                .background_executor()
                .spawn(async move { workspace_io::scan_workspace_blocking(store, folder_name) })
                .await;
            this.update(cx, |col, cx| {
                if !Arc::ptr_eq(&scan.store, &col.store) {
                    return;
                }
                col.scan.load_in_flight = false;
                if col.disk_revision() != disk_revision {
                    let announce = col.scan.load_pending.take().unwrap_or(false) || announce_opened;
                    col.load_workspace_async(cx, announce);
                    return;
                }
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

    pub fn request_rescan(&self, cx: &mut Context<Self>) {
        cx.emit(CollectionEvent::RescanRequested);
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
        let name = self.workspace_scope.name.clone();
        let disk_revision = self.disk_revision();
        cx.spawn(async move |this, cx| {
            let scan_store = store.clone();
            let tree = cx
                .background_executor()
                .spawn(async move { workspace_io::scan_collection(scan_store.as_ref(), &name) })
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
                if col.disk_revision() != disk_revision {
                    col.scan.rescan_pending = true;
                } else if let Ok(tree) = tree {
                    let missing = col.apply_tree(tree, col.capture_kept_requests(&preserved));
                    if let Some(persistence) = persistence.upgrade() {
                        persistence.update(cx, |state, cx| {
                            for id in missing {
                                state.changed_request_deleted(&id);
                            }
                            cx.notify();
                        });
                    }
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

    #[cfg(test)]
    pub fn reload_tree(&mut self) {
        self.reload_tree_preserving(&HashSet::new());
    }

    #[cfg(test)]
    pub fn reload_tree_preserving(&mut self, dirty: &HashSet<String>) {
        let kept_requests = self.capture_kept_requests(dirty);
        if let Ok(tree) =
            workspace_io::scan_collection(self.store.as_ref(), &self.workspace_scope.name)
        {
            self.apply_tree(tree, kept_requests);
        }
    }

    pub(crate) fn apply_request_disk_changes(
        &mut self,
        files: impl IntoIterator<Item = RequestReload>,
        deleted: &HashSet<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut reloaded = Vec::new();
        let mut needs_rescan = false;
        let mut changed = !deleted.is_empty();
        for reload in files {
            changed = true;
            if let Ok(file) = reload.file {
                self.parse_errors.remove(&reload.id);
                let workspace_name = self.workspace_scope.name.clone();
                if let Some(request) = self.find_request_mut(&reload.id) {
                    let status = request.last_status_code.or(reload.last_status);
                    *request =
                        file.into_item(&reload.id, &workspace_name, request.folder_id.clone());
                    request.last_status_code = status;
                    reloaded.push(reload.id);
                } else {
                    needs_rescan = true;
                }
            } else if let Err(error) = reload.file {
                self.parse_errors.insert(reload.id, error);
            }
        }
        for id in deleted {
            self.parse_errors.remove(id);
            cx.emit(CollectionEvent::RequestDeleted(id.clone()));
        }
        self.requests
            .retain(|request| !deleted.contains(&request.id));
        let old_pin_count = self.pins.len();
        self.pins.retain(|id| !deleted.contains(id));
        if self.pins.len() != old_pin_count {
            cx.emit(CollectionEvent::PinsModified);
        }
        if changed {
            self.bump_tree_version();
            cx.notify();
        }
        if !reloaded.is_empty() {
            cx.emit(CollectionEvent::RequestsReloaded {
                req_ids: Some(reloaded),
            });
        }
        needs_rescan
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

    fn apply_tree(
        &mut self,
        mut tree: CollectionSnapshot,
        kept_requests: HashMap<String, RequestItem>,
    ) -> Vec<String> {
        let missing_ids = tree.preserve_requests(kept_requests);
        self.requests = tree.requests;
        self.folders = tree.folders;
        self.parse_errors = tree.parse_errors;
        self.bump_tree_version();
        missing_ids
    }
}
