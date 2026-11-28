use super::CollectionState;
use crate::state::CollectionEvent;
#[cfg(test)]
use crate::state::FolderMoveOutcome;
use gpui_kit::{App, Context};
use zest_core::models::Folder;

/// Folder lifecycle, folder moves and id repathing operations of
/// [`CollectionState`], split from `workspace_crud.rs` to keep each file
/// under the 650 LOC budget.
enum FolderMove {
    Unchanged,
    Reordered,
    Into(Option<String>),
}

impl CollectionState {
    #[cfg(test)]
    pub fn create_folder_internal(
        &mut self,
        parent: Option<String>,
        name: String,
    ) -> Option<String> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let (_, id) = self.store.create_group(parent.as_deref(), name).ok()?;
        self.insert_folder(id.clone(), name, parent);
        Some(id)
    }

    fn insert_folder(&mut self, id: String, name: &str, parent: Option<String>) {
        let mut folder = Folder::new(&self.workspace_scope.name, name);
        folder.id = id;
        folder.parent_id = parent;
        self.folders.push(folder);
        self.bump_tree_version();
    }

    pub fn create_folder(
        &mut self,
        parent: Option<String>,
        name: String,
        cx: &mut Context<Self>,
        on_created: impl FnOnce(String, &mut App) + 'static,
    ) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let target = parent.clone();
        let disk_name = name.clone();
        self.edit_disk(
            "Could Not Create Folder",
            move |store| store.create_group(target.as_deref(), &disk_name),
            move |state, (_, id), cx| {
                state.insert_folder(id.clone(), &name, parent);
                cx.notify();
                on_created(id, cx);
            },
            cx,
        );
    }

    #[cfg(test)]
    pub fn delete_folder_internal(&mut self, folder_id: &str) -> anyhow::Result<Vec<String>> {
        self.store.delete_group(folder_id)?;
        Ok(self.remove_folder(folder_id))
    }

    fn remove_folder(&mut self, folder_id: &str) -> Vec<String> {
        let prefix = format!("{}/", folder_id);
        self.folders
            .retain(|f| f.id != folder_id && !f.id.starts_with(&prefix));
        let mut deleted_requests = Vec::new();
        self.requests.retain(|request| {
            if request.id.starts_with(&prefix) {
                deleted_requests.push(request.id.clone());
                false
            } else {
                true
            }
        });
        self.parse_errors.retain(|id, _| !id.starts_with(&prefix));
        self.pins
            .retain(|id| id != folder_id && !id.starts_with(&prefix));
        self.bump_tree_version();
        deleted_requests
    }

    pub fn delete_folder(&mut self, folder_id: &str, cx: &mut Context<Self>) {
        let folder = folder_id.to_string();
        let disk_id = folder.clone();
        self.edit_disk(
            "Could Not Delete Folder",
            move |store| store.delete_group(&disk_id),
            move |state, (), cx| {
                for id in state.remove_folder(&folder) {
                    cx.emit(CollectionEvent::RequestDeleted(id));
                }
                cx.emit(CollectionEvent::PinsModified);
                cx.notify();
            },
            cx,
        );
    }

    #[cfg(test)]
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
        let new_path = self.store.rename_group(folder_id, trimmed).ok()?;
        let changed = self.apply_folder_rename(folder_id, &new_path, trimmed);
        Some((new_path, changed))
    }

    fn apply_folder_rename(&mut self, old: &str, new: &str, name: &str) -> Vec<(String, String)> {
        if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == old) {
            folder.name = name.to_string();
        }
        let changed = self.repath_folder_descendants(old, new);
        self.bump_tree_version();
        changed
    }

    pub fn rename_folder(&mut self, folder_id: &str, new_name: String, cx: &mut Context<Self>) {
        let name = new_name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let old = folder_id.to_string();
        let disk_id = old.clone();
        let disk_name = name.clone();
        self.edit_disk(
            "Could Not Rename Folder",
            move |store| store.rename_group(&disk_id, &disk_name),
            move |state, new, cx| {
                let pairs = state.apply_folder_rename(&old, &new, &name);
                Self::notify_folder_changed(pairs, cx);
            },
            cx,
        );
    }

    fn notify_folder_changed(pairs: Vec<(String, String)>, cx: &mut Context<Self>) {
        if pairs.is_empty() {
            cx.emit(CollectionEvent::PinsModified);
        } else {
            cx.emit(CollectionEvent::IdsRemapped(pairs));
        }
        cx.notify();
    }

    /// Moves a folder into another folder (or to the workspace root) and
    /// persists the move on disk. Dropping onto the current parent only
    /// reorders siblings in memory, matching the legacy behavior.
    fn prepare_folder_move(
        &mut self,
        src_folder_id: &str,
        target_folder_id: Option<String>,
    ) -> Result<FolderMove, String> {
        let Some(src) = self.folders.iter().find(|f| f.id == src_folder_id).cloned() else {
            return Ok(FolderMove::Unchanged);
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
                    return Ok(FolderMove::Reordered);
                }
            }
            return Ok(FolderMove::Unchanged);
        }

        // A folder cannot be moved into itself or one of its descendants.
        if let Some(t) = &target {
            if self.folder_is_in_subtree(t, src_folder_id) {
                return Err("Cannot move a folder into itself".to_string());
            }
        }

        Ok(FolderMove::Into(target))
    }

    fn apply_folder_move(
        &mut self,
        old: &str,
        new: &str,
        parent: Option<String>,
    ) -> Vec<(String, String)> {
        let pairs = self.repath_folder_descendants(old, new);
        if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == new) {
            folder.parent_id = parent;
        }
        self.bump_tree_version();
        pairs
    }

    #[cfg(test)]
    pub fn move_folder_internal(
        &mut self,
        src_folder_id: &str,
        target_folder_id: Option<String>,
    ) -> Result<FolderMoveOutcome, String> {
        match self.prepare_folder_move(src_folder_id, target_folder_id)? {
            FolderMove::Unchanged => Ok(FolderMoveOutcome::Unchanged),
            FolderMove::Reordered => Ok(FolderMoveOutcome::Reordered),
            FolderMove::Into(parent) => {
                let new = self
                    .store
                    .move_group(src_folder_id, parent.as_deref())
                    .map_err(|e| format!("Failed to move folder: {e}"))?;
                Ok(FolderMoveOutcome::Moved(self.apply_folder_move(
                    src_folder_id,
                    &new,
                    parent,
                )))
            }
        }
    }

    pub fn move_folder(&mut self, source: &str, parent: Option<String>, cx: &mut Context<Self>) {
        match self.prepare_folder_move(source, parent) {
            Ok(FolderMove::Unchanged) => {}
            Ok(FolderMove::Reordered) => cx.notify(),
            Ok(FolderMove::Into(parent)) => {
                let old = source.to_string();
                let disk_id = old.clone();
                let target = parent.clone();
                self.edit_disk(
                    "Cannot rename or move",
                    move |store| store.move_group(&disk_id, target.as_deref()),
                    move |state, new, cx| {
                        let pairs = state.apply_folder_move(&old, &new, parent);
                        Self::notify_folder_changed(pairs, cx);
                    },
                    cx,
                );
            }
            Err(message) => cx.emit(CollectionEvent::CrudFailed {
                title: "Cannot rename or move",
                message,
            }),
        }
    }

    #[cfg(test)]
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
        let Some(request) = self.find_request(src_req_id) else {
            return;
        };
        if self.effective_folder_id(&target_folder_id)
            == self.effective_folder_id(&request.folder_id)
        {
            return;
        }
        let old = src_req_id.to_string();
        let disk_id = old.clone();
        let target = target_folder_id.clone();
        self.edit_disk(
            "Could Not Move Request",
            move |store| store.move_request(&disk_id, target.as_deref()),
            move |state, new, cx| {
                state.apply_request_move(&old, &new, target_folder_id);
                cx.emit(CollectionEvent::IdsRemapped(vec![(old, new)]));
                cx.notify();
            },
            cx,
        );
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
        folder_id == ancestor_id
            || zest_core::storage::collection::folder_ancestors(&self.folders, Some(folder_id))
                .iter()
                .any(|folder| folder.id == ancestor_id)
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
        zest_core::storage::collection::repath_subtree(
            &mut self.folders,
            &mut self.requests,
            &mut self.pins,
            &mut self.parse_errors,
            old_id,
            new_id,
        )
    }

    /// Moves a request to a different effective folder on disk. Duplicate
    /// stems in the destination are allowed: the store suffixes the file
    /// (login-2.yaml) while the display name stays as typed. Returns the new
    /// id, or `None` when the request already lives in the destination folder
    /// or the store rejected the move.
    #[cfg(test)]
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

        self.flush_request_to_disk(src_req_id).ok()?;
        let new_rel_path = self
            .store
            .move_request(src_req_id, target_folder_id.as_deref())
            .ok()?;

        self.apply_request_move(src_req_id, &new_rel_path, target_folder_id);
        Some(new_rel_path)
    }

    fn apply_request_move(&mut self, old: &str, new: &str, folder: Option<String>) {
        if let Some(request) = self.find_request_mut(old) {
            request.id = new.to_string();
            request.folder_id = folder;
        }
        if let Some(pin) = self.pins.iter_mut().find(|pin| pin.as_str() == old) {
            *pin = new.to_string();
        }
        if let Some(error) = self.parse_errors.remove(old) {
            self.parse_errors.insert(new.to_string(), error);
        }
        self.bump_tree_version();
    }
}
