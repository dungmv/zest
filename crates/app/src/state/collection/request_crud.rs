use super::CollectionState;
use crate::state::{CollectionEvent, RequestSessionState};
use gpui_kit::{App, Context, Entity};
use std::path::PathBuf;
use zest_core::models::{HttpMethod, Protocol, RequestItem};
use zest_core::storage::models::RequestFile;

/// Pairs `update_active_request` with a `mark_dirty` on the session state —
/// the standard "edit the active request and flag it for persistence"
/// boilerplate shared by every composer control.
pub fn edit_active_request(
    workspace_state: &Entity<CollectionState>,
    session_state: &Entity<RequestSessionState>,
    req_id: &str,
    cx: &mut App,
    edit: impl FnOnce(&mut RequestItem),
) {
    workspace_state.update(cx, |ws, _| {
        ws.update_active_request(req_id, edit);
    });
    session_state.update(cx, |ed, cx| {
        ed.mark_dirty(cx);
    });
}

impl CollectionState {
    /// Conventional HTTP verb persisted for a request of the given protocol.
    /// WS handshake is a GET; gRPC and GraphQL ride on HTTP POST.
    fn protocol_method(protocol: Protocol) -> HttpMethod {
        match protocol {
            Protocol::Http | Protocol::WebSocket | Protocol::Sse => HttpMethod::Get,
            Protocol::Grpc | Protocol::GraphQL => HttpMethod::Post,
        }
    }

    /// Creates a request of `protocol` in memory and on disk (scratch excepted).
    pub fn create_request_internal(
        &mut self,
        folder_rel: Option<String>,
        protocol: Protocol,
    ) -> RequestItem {
        let method = Self::protocol_method(protocol);
        let (_slug, rel_path) = self
            .store
            .create_request(folder_rel.as_deref(), "New Request", method, "", protocol)
            .unwrap_or_else(|_| {
                (
                    format!("req-{}", uuid::Uuid::new_v4().simple()),
                    "new-request.yaml".to_string(),
                )
            });

        let ws_name = self.workspace_scope.name.clone();
        let mut item =
            RequestItem::new_with_protocol(&ws_name, "New Request", method, "", protocol);
        item.id = rel_path.clone();
        item.folder_id = folder_rel;

        self.requests.push(item.clone());
        self.bump_tree_version();
        item
    }

    pub fn create_request(
        &mut self,
        folder_rel: Option<String>,
        protocol: Protocol,
        cx: &mut Context<Self>,
    ) -> RequestItem {
        let item = self.create_request_internal(folder_rel, protocol);
        cx.notify();
        item
    }

    pub fn duplicate_request_internal(&mut self, req_id: &str) -> anyhow::Result<RequestItem> {
        let existing = self
            .find_request(req_id)
            .ok_or_else(|| anyhow::anyhow!("Request no longer exists"))?;
        let folder = existing.folder_id.clone();
        let status = existing.last_status_code;
        let mut file = RequestFile::from_item(existing);
        file.name = format!("{} (Copy)", file.name);
        let (_, path) = self
            .store
            .create_request_file(folder.as_deref(), file.clone())?;
        let mut duplicate = file.into_item(&path, &self.workspace_scope.name, folder);
        duplicate.last_status_code = status;
        self.requests.push(duplicate.clone());
        self.bump_tree_version();
        Ok(duplicate)
    }

    pub fn duplicate_request(
        &mut self,
        req_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<RequestItem> {
        match self.duplicate_request_internal(req_id) {
            Ok(request) => {
                cx.notify();
                Some(request)
            }
            Err(error) => {
                cx.emit(CollectionEvent::CrudFailed {
                    title: "Could Not Duplicate Request",
                    message: error.to_string(),
                });
                None
            }
        }
    }

    pub fn delete_request_internal(&mut self, req_id: &str) -> anyhow::Result<()> {
        self.store.delete_request(req_id)?;
        self.requests.retain(|r| r.id != req_id);
        self.parse_errors.remove(req_id);
        self.pins.retain(|p| p != req_id);
        self.bump_tree_version();
        Ok(())
    }

    pub fn delete_request(&mut self, req_id: &str, cx: &mut Context<Self>) -> bool {
        match self.delete_request_internal(req_id) {
            Ok(()) => {
                cx.emit(CollectionEvent::RequestDeleted(req_id.to_string()));
                cx.emit(CollectionEvent::PinsModified);
                cx.notify();
                true
            }
            Err(error) => {
                cx.emit(CollectionEvent::CrudFailed {
                    title: "Could Not Delete Request",
                    message: error.to_string(),
                });
                false
            }
        }
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
        self.flush_request_to_disk(req_id).ok()?;
        if let Ok(new_rel_path) = self.store.rename_request(req_id, trimmed) {
            if let Some(r) = self.requests.iter_mut().find(|r| r.id == req_id) {
                r.id = new_rel_path.clone();
                r.name = trimmed.to_string();
            }

            if let Some(pos) = self.pins.iter().position(|p| p == req_id) {
                self.pins[pos] = new_rel_path.clone();
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
                cx.emit(CollectionEvent::IdsRemapped(vec![(
                    req_id.to_string(),
                    new_id.clone(),
                )]));
            }
            cx.notify();
        }
        res
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
                    cx.emit(CollectionEvent::IdsRemapped(vec![(
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

        let request = self.find_request_mut(req_id)?;
        let mut unique_name = trimmed.to_string();
        let mut idx = 2;
        while request.examples.iter().any(|e| e.name == unique_name) {
            unique_name = format!("{trimmed}-{idx}");
            idx += 1;
        }
        let example = request.examples.iter_mut().find(|e| e.name == old_name)?;
        example.name = unique_name.clone();
        self.bump_tree_version();
        Some(unique_name)
    }

    pub fn rename_example(
        &mut self,
        req_id: &str,
        old_name: &str,
        new_name: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(unique_name) = self.rename_example_internal(req_id, old_name, new_name) {
            cx.emit(CollectionEvent::RequestContentModified(req_id.to_string()));
            cx.emit(CollectionEvent::ExampleRenamed {
                req_id: req_id.to_string(),
                old_name: old_name.to_string(),
                new_name: unique_name,
            });
            cx.notify();
        }
    }

    pub fn toggle_pin(&mut self, req_id: &str, cx: &mut Context<Self>) {
        if self.pins.contains(&req_id.to_string()) {
            self.pins.retain(|p| p != req_id);
        } else {
            self.pins.push(req_id.to_string());
        }
        cx.emit(CollectionEvent::PinsModified);
        self.bump_tree_version();
        cx.notify();
    }

    pub fn is_pinned(&self, req_id: &str) -> bool {
        self.pins.contains(&req_id.to_string())
    }

    pub fn reorder_pins(&mut self, new_pins: Vec<String>, cx: &mut Context<Self>) {
        self.pins = new_pins;
        cx.emit(CollectionEvent::PinsModified);
        self.bump_tree_version();
        cx.notify();
    }

    pub fn get_request_file(&self, req_id: &str) -> Option<RequestFile> {
        self.find_request(req_id).map(RequestFile::from_item)
    }

    pub(super) fn flush_request_to_disk(&self, req_id: &str) -> anyhow::Result<()> {
        let request = self
            .find_request(req_id)
            .ok_or_else(|| anyhow::anyhow!("Request no longer exists"))?;
        self.store.update_request_from_item(request)
    }

    pub fn save_request_file(
        &mut self,
        req_id: &str,
        req_file: &RequestFile,
        cx: &mut Context<Self>,
    ) {
        if let Some(request) = self.find_request_mut(req_id) {
            request.examples.clone_from(&req_file.examples);
            self.bump_tree_version();
            cx.emit(CollectionEvent::RequestContentModified(req_id.to_string()));
            cx.notify();
        }
    }
}
