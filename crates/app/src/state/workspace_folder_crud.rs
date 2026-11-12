use super::{FolderMoveOutcome, WorkspaceDataState, WorkspaceEvent};
use gpui_kit::Context;
use std::collections::HashMap;
use zest_core::models::Folder;

/// Folder lifecycle, folder moves and id repathing operations of
/// [`WorkspaceDataState`], split from `workspace_crud.rs` to keep each file
/// under the 650 LOC budget.
impl WorkspaceDataState {
    pub fn create_folder_internal(
        &mut self,
        parent_id: Option<String>,
        name: String,
    ) -> Option<String> {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            if let Ok((_dir_name, rel_path)) =
                self.store.create_group(parent_id.as_deref(), trimmed)
            {
                let ws_name = self.workspace_meta.name.clone();
                let mut f = Folder::new(&ws_name, trimmed);
                f.id = rel_path.clone();
                f.parent_id = parent_id;
                self.folders.push(f);
                self.bump_tree_version();
                return Some(rel_path);
            }
        }
        None
    }

    pub fn create_folder(
        &mut self,
        parent_id: Option<String>,
        name: String,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let res = self.create_folder_internal(parent_id, name);
        if res.is_some() {
            cx.notify();
        }
        res
    }

    pub fn delete_folder_internal(&mut self, folder_id: &str) {
        let _ = self.store.delete_group(folder_id);
        let prefix = format!("{}/", folder_id);
        self.folders
            .retain(|f| f.id != folder_id && !f.id.starts_with(&prefix));
        self.requests.retain(|r| {
            if let Some(fid) = &r.folder_id {
                fid != folder_id && !fid.starts_with(&prefix)
            } else {
                !r.id.starts_with(&prefix)
            }
        });
        self.pins
            .retain(|p| p != folder_id && !p.starts_with(&prefix));
        let _ = self.store.save_pins(&self.pins);
        self.bump_tree_version();
    }

    pub fn delete_folder(&mut self, folder_id: &str, cx: &mut Context<Self>) {
        self.delete_folder_internal(folder_id);
        cx.notify();
    }

    pub fn rename_folder_internal(
        &mut self,
        folder_id: &str,
        new_name: &str,
    ) -> Option<(String, Vec<(String, String)>)> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return None;
        }
        // Duplicate display names are allowed: the store suffixes the
        // directory (auth-2) while the display name stays as typed.
        if let Ok(new_rel_path) = self.store.rename_group(folder_id, trimmed) {
            if let Some(f) = self.folders.iter_mut().find(|f| f.id == folder_id) {
                f.name = trimmed.to_string();
            }
            // Remap the folder id and every descendant id to the new prefix.
            let remapped_pairs = self.repath_folder_descendants(folder_id, &new_rel_path);
            self.bump_tree_version();
            Some((new_rel_path, remapped_pairs))
        } else {
            None
        }
    }

    pub fn rename_folder(&mut self, folder_id: &str, new_name: String, cx: &mut Context<Self>) {
        if let Some((_new_path, pairs)) = self.rename_folder_internal(folder_id, &new_name) {
            if !pairs.is_empty() {
                cx.emit(WorkspaceEvent::IdsRemapped(pairs));
            }
            cx.notify();
        }
    }

    /// Moves a folder into another folder (or to the workspace root) and
    /// persists the move on disk. Dropping onto the current parent only
    /// reorders siblings in memory, matching the legacy behavior.
    pub fn move_folder_internal(
        &mut self,
        src_folder_id: &str,
        target_folder_id: Option<String>,
    ) -> Result<FolderMoveOutcome, String> {
        let Some(src) = self.folders.iter().find(|f| f.id == src_folder_id).cloned() else {
            return Ok(FolderMoveOutcome::Unchanged);
        };
        let target = target_folder_id.filter(|t| !t.trim().is_empty() && t != src_folder_id);

        let current_parent = self.effective_folder_id(&src.parent_id);
        let target_parent = target
            .as_ref()
            .map(|t| self.effective_folder_id(&Some(t.clone())))
            .unwrap_or_default();

        if target_parent == current_parent {
            if let Some(target_id) = &target {
                // Same-parent drop: cosmetic reorder within the sidebar only.
                let src_pos = self.folders.iter().position(|f| f.id == src_folder_id);
                if let Some(s_idx) = src_pos {
                    let folder = self.folders.remove(s_idx);
                    let new_t_idx = self
                        .folders
                        .iter()
                        .position(|f| f.id == *target_id)
                        .unwrap_or(self.folders.len());
                    self.folders.insert(new_t_idx, folder);
                    self.bump_tree_version();
                    return Ok(FolderMoveOutcome::Reordered);
                }
            }
            return Ok(FolderMoveOutcome::Unchanged);
        }

        // A folder cannot be moved into itself or one of its descendants.
        if let Some(t) = &target {
            if self.folder_is_in_subtree(t, src_folder_id) {
                return Err("Cannot move a folder into itself".to_string());
            }
        }

        // Duplicate directory names in the destination are allowed: the
        // store suffixes the moved directory (shared-2) while the display
        // name stays as typed.
        match self.store.move_group(src_folder_id, target.as_deref()) {
            Ok(new_rel_path) => {
                // Remap the folder id and every descendant id to the new path.
                let pairs = self.repath_folder_descendants(src_folder_id, &new_rel_path);
                if let Some(f) = self.folders.iter_mut().find(|f| f.id == new_rel_path) {
                    f.parent_id = target;
                }
                self.bump_tree_version();
                Ok(FolderMoveOutcome::Moved(pairs))
            }
            Err(e) => Err(format!("Failed to move folder: {}", e)),
        }
    }

    pub fn move_folder(
        &mut self,
        src_folder_id: &str,
        target_folder_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        match self.move_folder_internal(src_folder_id, target_folder_id) {
            Ok(FolderMoveOutcome::Unchanged) => {}
            Ok(FolderMoveOutcome::Reordered) => {
                cx.notify();
            }
            Ok(FolderMoveOutcome::Moved(pairs)) => {
                if !pairs.is_empty() {
                    cx.emit(WorkspaceEvent::IdsRemapped(pairs));
                }
                cx.notify();
            }
            Err(message) => {
                cx.emit(WorkspaceEvent::CrudFailed {
                    title: "Cannot rename or move",
                    message,
                });
            }
        }
    }

    pub fn move_request_to_folder_internal(
        &mut self,
        src_req_id: &str,
        target_folder_id: Option<String>,
    ) -> Option<String> {
        self.move_request_across_folders(src_req_id, target_folder_id)
    }

    pub fn move_request_to_folder(
        &mut self,
        src_req_id: &str,
        target_folder_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(new_id) = self.move_request_to_folder_internal(src_req_id, target_folder_id) {
            cx.emit(WorkspaceEvent::IdsRemapped(vec![(
                src_req_id.to_string(),
                new_id,
            )]));
            cx.notify();
        }
    }

    // ------------------------------------------------------------------
    // Folder-tree & id remapping helpers
    // ------------------------------------------------------------------

    /// Effective folder for sidebar grouping: a folder id that no longer
    /// matches an existing folder renders at root level (mirrors the sidebar
    /// filter, so grouping here and in the sidebar never disagree).
    pub(super) fn effective_folder_id(&self, folder_id: &Option<String>) -> Option<String> {
        match folder_id {
            Some(id) if !id.trim().is_empty() && self.folders.iter().any(|f| &f.id == id) => {
                Some(id.clone())
            }
            _ => None,
        }
    }

    /// Whether `folder_id` is `ancestor_id` itself or lives inside its subtree.
    pub(super) fn folder_is_in_subtree(&self, folder_id: &str, ancestor_id: &str) -> bool {
        if folder_id == ancestor_id {
            return true;
        }
        let mut cur = self
            .folders
            .iter()
            .find(|f| f.id == folder_id)
            .and_then(|f| f.parent_id.clone());
        while let Some(pid) = cur {
            if pid == ancestor_id {
                return true;
            }
            cur = self
                .folders
                .iter()
                .find(|f| f.id == pid)
                .and_then(|f| f.parent_id.clone());
        }
        false
    }

    /// Writes the in-memory item state to disk before an id-changing operation
    /// (rename/move) so edits still waiting in the 500 ms save debounce are
    /// not left behind on the old path.
    /// Remaps the id of `old_id` and of every descendant folder/request to the
    /// `new_id/...` prefix, following a folder rename/move on disk. Returns
    /// the (old, new) id pairs of all affected folders and requests so
    /// downstream states can follow the change.
    pub(super) fn repath_folder_descendants(
        &mut self,
        old_id: &str,
        new_id: &str,
    ) -> Vec<(String, String)> {
        let old_prefix = format!("{}/", old_id);
        let new_prefix = format!("{}/", new_id);
        let repath = |id: &str| -> String {
            if let Some(rest) = id.strip_prefix(&old_prefix) {
                format!("{}{}", new_prefix, rest)
            } else if id == old_id {
                new_id.to_string()
            } else {
                id.to_string()
            }
        };

        let mut pairs = Vec::new();

        for f in self.folders.iter_mut() {
            let new_fid = repath(&f.id);
            if new_fid != f.id {
                pairs.push((f.id.clone(), new_fid.clone()));
                f.id = new_fid;
            }
            let new_parent = f.parent_id.as_ref().map(|p| repath(p));
            if new_parent != f.parent_id {
                f.parent_id = new_parent;
            }
        }

        for r in self.requests.iter_mut() {
            let new_folder = r.folder_id.as_ref().map(|p| repath(p));
            if new_folder != r.folder_id {
                r.folder_id = new_folder;
            }
            let new_rid = repath(&r.id);
            if new_rid != r.id {
                pairs.push((r.id.clone(), new_rid.clone()));
                r.id = new_rid;
            }
        }

        let mut pins_changed = false;
        let mut new_pins = Vec::with_capacity(self.pins.len());
        for p in &self.pins {
            let np = repath(p);
            if np != *p {
                pins_changed = true;
            }
            new_pins.push(np);
        }
        if pins_changed {
            self.pins = new_pins;
            let _ = self.store.save_pins(&self.pins);
        }

        let mut errors_changed = false;
        let mut new_errors = HashMap::with_capacity(self.parse_errors.len());
        for (k, v) in &self.parse_errors {
            let nk = repath(k);
            if nk != *k {
                errors_changed = true;
            }
            new_errors.insert(nk, v.clone());
        }
        if errors_changed {
            self.parse_errors = new_errors;
        }

        pairs
    }

    /// Moves a request to a different effective folder on disk. Duplicate
    /// stems in the destination are allowed: the store suffixes the file
    /// (login-2.yaml) while the display name stays as typed. Returns the new
    /// id, or `None` when the request already lives in the destination folder
    /// or the store rejected the move.
    pub(super) fn move_request_across_folders(
        &mut self,
        src_req_id: &str,
        target_folder_id: Option<String>,
    ) -> Option<String> {
        let src = self.requests.iter().find(|r| r.id == src_req_id).cloned()?;
        let target_eff = self.effective_folder_id(&target_folder_id);
        if target_eff == self.effective_folder_id(&src.folder_id) {
            return None;
        }

        self.flush_request_to_disk(src_req_id);
        let new_rel_path = self
            .store
            .move_request(src_req_id, target_folder_id.as_deref())
            .ok()?;

        if let Some(r) = self.requests.iter_mut().find(|r| r.id == src_req_id) {
            r.id = new_rel_path.clone();
            r.folder_id = target_folder_id;
        }
        if let Some(pos) = self.pins.iter().position(|p| p == src_req_id) {
            self.pins[pos] = new_rel_path.clone();
            let _ = self.store.save_pins(&self.pins);
        }
        self.bump_tree_version();
        Some(new_rel_path)
    }
}
