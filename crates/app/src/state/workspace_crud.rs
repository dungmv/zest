use super::{FolderMoveOutcome, WorkspaceDataState, WorkspaceEvent};
use crate::settings::AppSettings;
use gpui::Context;
use std::collections::HashMap;
use std::path::PathBuf;
use zest_core::models::{Folder, HttpMethod, HttpRequestItem};
use zest_core::storage::models::RequestFile;

impl WorkspaceDataState {
    pub fn create_request_internal(&mut self, folder_rel: Option<String>) -> HttpRequestItem {
        let (_slug, rel_path) = self
            .store
            .create_request(folder_rel.as_deref(), "New Request", HttpMethod::Get, "")
            .unwrap_or_else(|_| {
                (
                    format!("req-{}", uuid::Uuid::new_v4().simple()),
                    "new-request.yaml".to_string(),
                )
            });

        let ws_name = self.workspace_meta.name.clone();
        let mut item = HttpRequestItem::new(&ws_name, "New Request", HttpMethod::Get, "");
        item.id = rel_path.clone();
        item.folder_id = folder_rel;

        self.requests.push(item.clone());
        self.bump_tree_version();
        item
    }

    pub fn create_request(
        &mut self,
        folder_rel: Option<String>,
        cx: &mut Context<Self>,
    ) -> HttpRequestItem {
        let item = self.create_request_internal(folder_rel);
        cx.notify();
        item
    }

    pub fn duplicate_request_internal(&mut self, req_id: &str) -> Option<HttpRequestItem> {
        let existing = self.requests.iter().find(|r| r.id == req_id)?.clone();
        let mut req_file = self
            .store
            .read_request(req_id)
            .unwrap_or_else(|_| RequestFile {
                name: existing.name.clone(),
                method: existing.method,
                url: existing.url.clone(),
                ..Default::default()
            });

        let new_name = format!("{} (Copy)", req_file.name);
        req_file.name = new_name.clone();

        let folder_rel = existing.folder_id.clone();
        let (_slug, rel_path) = self
            .store
            .create_request(
                folder_rel.as_deref(),
                &new_name,
                req_file.method,
                &req_file.url,
            )
            .ok()?;

        let _ = self.store.write_request(&rel_path, &req_file);
        let ws_name = self.workspace_meta.name.clone();
        let mut dup = req_file.to_item(&rel_path, &ws_name, folder_rel);
        dup.last_status_code = existing.last_status_code;
        self.requests.push(dup.clone());
        self.bump_tree_version();
        Some(dup)
    }

    pub fn duplicate_request(
        &mut self,
        req_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<HttpRequestItem> {
        let res = self.duplicate_request_internal(req_id);
        if res.is_some() {
            cx.notify();
        }
        res
    }

    pub fn delete_request_internal(&mut self, req_id: &str) {
        let _ = self.store.delete_request(req_id);
        self.requests.retain(|r| r.id != req_id);
        self.parse_errors.remove(req_id);
        self.pins.retain(|p| p != req_id);
        let _ = self.store.save_pins(&self.pins);
        self.bump_tree_version();
    }

    pub fn delete_request(&mut self, req_id: &str, cx: &mut Context<Self>) {
        self.delete_request_internal(req_id);
        cx.notify();
    }

    pub fn rename_request_internal(&mut self, req_id: &str, new_name: &str) -> Option<String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return None;
        }
        // Duplicate display names are allowed: the store suffixes the file
        // slug (login-2.yaml) while the display name stays as typed.
        // Flush edits still waiting in the save debounce to the old path
        // before the file is renamed away from under them.
        self.flush_request_to_disk(req_id);
        if let Ok(new_rel_path) = self.store.rename_request(req_id, trimmed) {
            if let Some(r) = self.requests.iter_mut().find(|r| r.id == req_id) {
                r.id = new_rel_path.clone();
                r.name = trimmed.to_string();
            }

            if let Some(pos) = self.pins.iter().position(|p| p == req_id) {
                self.pins[pos] = new_rel_path.clone();
                let _ = self.store.save_pins(&self.pins);
            }

            if let Some(err) = self.parse_errors.remove(req_id) {
                self.parse_errors.insert(new_rel_path.clone(), err);
            }

            self.bump_tree_version();
            Some(new_rel_path)
        } else {
            None
        }
    }

    pub fn rename_request(
        &mut self,
        req_id: &str,
        new_name: &str,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let res = self.rename_request_internal(req_id, new_name);
        if let Some(new_id) = &res {
            if new_id != req_id {
                cx.emit(WorkspaceEvent::IdsRemapped(vec![(
                    req_id.to_string(),
                    new_id.clone(),
                )]));
            }
            cx.notify();
        }
        res
    }

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

    pub fn move_request(&mut self, src_req_id: &str, target_req_id: &str, cx: &mut Context<Self>) {
        if src_req_id == target_req_id {
            return;
        }
        let src_req = self.requests.iter().find(|r| r.id == src_req_id).cloned();
        let target_req = self
            .requests
            .iter()
            .find(|r| r.id == target_req_id)
            .cloned();

        if let (Some(src), Some(target)) = (src_req, target_req) {
            let src_eff = self.effective_folder_id(&src.folder_id);
            let target_eff = self.effective_folder_id(&target.folder_id);
            if src_eff != target_eff {
                if let Some(new_id) =
                    self.move_request_across_folders(src_req_id, target.folder_id.clone())
                {
                    cx.emit(WorkspaceEvent::IdsRemapped(vec![(
                        src_req_id.to_string(),
                        new_id,
                    )]));
                }
            } else {
                let folder_id = src.folder_id.clone();
                let mut stems: Vec<String> = self
                    .requests
                    .iter()
                    .filter(|r| r.folder_id == folder_id)
                    .map(|r| {
                        PathBuf::from(&r.id)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("req")
                            .to_string()
                    })
                    .collect();

                let src_stem = PathBuf::from(src_req_id)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("req")
                    .to_string();
                let target_stem = PathBuf::from(target_req_id)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("req")
                    .to_string();

                if let Some(s_pos) = stems.iter().position(|s| s == &src_stem) {
                    stems.remove(s_pos);
                    let t_pos = stems
                        .iter()
                        .position(|s| s == &target_stem)
                        .unwrap_or(stems.len());
                    stems.insert(t_pos, src_stem);
                    let _ = self.store.reorder_folder(folder_id.as_deref(), &stems);
                    let src_idx = self.requests.iter().position(|r| r.id == src_req_id);
                    let target_idx = self.requests.iter().position(|r| r.id == target_req_id);
                    if let (Some(si), Some(ti)) = (src_idx, target_idx) {
                        let item = self.requests.remove(si);
                        self.requests.insert(ti, item);
                    }
                    self.bump_tree_version();
                }
            }
        }
        cx.notify();
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

    pub fn rename_example_internal(
        &mut self,
        req_id: &str,
        old_name: &str,
        new_name: &str,
    ) -> Option<String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() || trimmed == old_name {
            return None;
        }

        if let Ok(mut req_file) = self.store.read_request(req_id) {
            let mut unique_name = trimmed.to_string();
            let mut idx = 2;
            while req_file
                .examples
                .iter()
                .any(|e| e.name == unique_name && e.name != old_name)
            {
                unique_name = format!("{}-{}", trimmed, idx);
                idx += 1;
            }
            if let Some(ex) = req_file.examples.iter_mut().find(|e| e.name == old_name) {
                ex.name = unique_name.clone();
            }
            let _ = self.store.write_request(req_id, &req_file);
            if let Some(r) = self.requests.iter_mut().find(|r| r.id == req_id) {
                if let Some(ex) = r.examples.iter_mut().find(|e| e.name == old_name) {
                    ex.name = unique_name.clone();
                }
            }
            self.bump_tree_version();
            return Some(unique_name);
        }
        None
    }

    pub fn rename_example(
        &mut self,
        req_id: &str,
        old_name: &str,
        new_name: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(unique_name) = self.rename_example_internal(req_id, old_name, new_name) {
            cx.emit(WorkspaceEvent::ExampleRenamed {
                req_id: req_id.to_string(),
                old_name: old_name.to_string(),
                new_name: unique_name,
            });
            cx.notify();
        }
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

    pub fn sync_cookies_from_executor(&mut self) {
        self.cookies = self.executor.get_stored_cookies();
        if let Ok(val) = serde_json::to_value(&self.cookies) {
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

    pub fn toggle_pin(&mut self, req_id: &str, cx: &mut Context<Self>) {
        if self.pins.contains(&req_id.to_string()) {
            self.pins.retain(|p| p != req_id);
        } else {
            self.pins.push(req_id.to_string());
        }
        let _ = self.store.save_pins(&self.pins);
        self.bump_tree_version();
        cx.notify();
    }

    pub fn is_pinned(&self, req_id: &str) -> bool {
        self.pins.contains(&req_id.to_string())
    }

    pub fn reorder_pins(&mut self, new_pins: Vec<String>, cx: &mut Context<Self>) {
        self.pins = new_pins;
        let _ = self.store.save_pins(&self.pins);
        self.bump_tree_version();
        cx.notify();
    }

    pub fn get_request_file(&self, req_id: &str) -> Option<RequestFile> {
        self.store.read_request(req_id).ok()
    }

    // ------------------------------------------------------------------
    // Folder-tree & id remapping helpers
    // ------------------------------------------------------------------

    /// Effective folder for sidebar grouping: a folder id that no longer
    /// matches an existing folder renders at root level (mirrors the sidebar
    /// filter, so grouping here and in the sidebar never disagree).
    fn effective_folder_id(&self, folder_id: &Option<String>) -> Option<String> {
        match folder_id {
            Some(id) if !id.trim().is_empty() && self.folders.iter().any(|f| &f.id == id) => {
                Some(id.clone())
            }
            _ => None,
        }
    }

    /// Whether `folder_id` is `ancestor_id` itself or lives inside its subtree.
    fn folder_is_in_subtree(&self, folder_id: &str, ancestor_id: &str) -> bool {
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
    fn flush_request_to_disk(&self, req_id: &str) {
        if req_id == "scratch" {
            return;
        }
        let Some(item) = self.find_request(req_id) else {
            return;
        };
        let mut req_file = self
            .store
            .read_request(req_id)
            .unwrap_or_else(|_| RequestFile {
                name: item.name.clone(),
                method: item.method,
                url: item.url.clone(),
                ..Default::default()
            });
        req_file.update_from_item(item);
        let _ = self.store.write_request(req_id, &req_file);
    }

    /// Remaps the id of `old_id` and of every descendant folder/request to the
    /// `new_id/...` prefix, following a folder rename/move on disk. Returns
    /// the (old, new) id pairs of all affected folders and requests so
    /// downstream states can follow the change.
    fn repath_folder_descendants(&mut self, old_id: &str, new_id: &str) -> Vec<(String, String)> {
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
    fn move_request_across_folders(
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

    pub fn save_request_file_internal(&mut self, req_id: &str, req_file: &RequestFile) {
        let _ = self.store.write_request(req_id, req_file);
        if let Some(r) = self.requests.iter_mut().find(|r| r.id == req_id) {
            let prev_status = r.last_status_code;
            let folder_id = r.folder_id.clone();
            *r = req_file.to_item(req_id, &self.workspace_meta.name, folder_id);
            r.last_status_code = prev_status;
        }
        self.bump_tree_version();
    }

    pub fn save_request_file(
        &mut self,
        req_id: &str,
        req_file: &RequestFile,
        cx: &mut Context<Self>,
    ) {
        self.save_request_file_internal(req_id, req_file);
        cx.notify();
    }
}
