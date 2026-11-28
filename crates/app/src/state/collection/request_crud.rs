use super::CollectionState;
use crate::services::collection_io;
use crate::state::{CollectionEvent, RequestSessionState};
use gpui_kit::{App, Context, Entity};
use std::path::PathBuf;
use zest_core::models::{Protocol, RequestItem};
use zest_core::storage::{RequestFile, WorkspaceStore};

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
    #[cfg(test)]
    pub fn create_request_internal(
        &mut self,
        folder_rel: Option<String>,
        protocol: Protocol,
    ) -> RequestItem {
        let (id, file) =
            collection_io::create_request(self.store.as_ref(), folder_rel.as_deref(), protocol)
                .expect("create request fixture");
        self.insert_request(id, folder_rel, file, None)
    }

    fn insert_request(
        &mut self,
        id: String,
        folder: Option<String>,
        file: RequestFile,
        status: Option<u16>,
    ) -> RequestItem {
        let mut item = file.into_item(&id, &self.workspace_scope.name, folder);
        item.last_status_code = status;
        self.requests.push(item.clone());
        self.bump_tree_version();
        item
    }

    pub fn create_request(
        &mut self,
        folder: Option<String>,
        protocol: Protocol,
        cx: &mut Context<Self>,
        on_created: impl FnOnce(RequestItem, &mut App) + 'static,
    ) {
        let target = folder.clone();
        self.edit_disk(
            "Could Not Create Request",
            move |store| collection_io::create_request(store, target.as_deref(), protocol),
            move |state, (id, file), cx| {
                let item = state.insert_request(id, folder, file, None);
                cx.notify();
                on_created(item, cx);
            },
            cx,
        );
    }

    #[cfg(test)]
    pub fn duplicate_request_internal(&mut self, req_id: &str) -> anyhow::Result<RequestItem> {
        let request = self
            .find_request(req_id)
            .ok_or_else(|| anyhow::anyhow!("Request no longer exists"))?;
        let folder = request.folder_id.clone();
        let status = request.last_status_code;
        let (id, file) = collection_io::duplicate_request(
            self.store.as_ref(),
            folder.as_deref(),
            RequestFile::from_item(request),
        )?;
        Ok(self.insert_request(id, folder, file, status))
    }

    pub fn duplicate_request(
        &mut self,
        req_id: &str,
        cx: &mut Context<Self>,
        on_created: impl FnOnce(RequestItem, &mut App) + 'static,
    ) {
        let Some(existing) = self.find_request(req_id) else {
            return;
        };
        let folder = existing.folder_id.clone();
        let target = folder.clone();
        let status = existing.last_status_code;
        let file = RequestFile::from_item(existing);
        self.edit_disk(
            "Could Not Duplicate Request",
            move |store| collection_io::duplicate_request(store, target.as_deref(), file),
            move |state, (id, file), cx| {
                let duplicate = state.insert_request(id, folder, file, status);
                cx.notify();
                on_created(duplicate, cx);
            },
            cx,
        );
    }

    #[cfg(test)]
    pub fn delete_request_internal(&mut self, req_id: &str) -> anyhow::Result<()> {
        self.store.delete_request(req_id)?;
        self.remove_request(req_id);
        Ok(())
    }

    fn remove_request(&mut self, req_id: &str) {
        self.requests.retain(|r| r.id != req_id);
        self.parse_errors.remove(req_id);
        self.pins.retain(|p| p != req_id);
        self.bump_tree_version();
    }

    pub fn delete_request(
        &mut self,
        req_id: &str,
        cx: &mut Context<Self>,
        deleted: impl FnOnce(&mut Self, Option<String>, &mut Context<Self>) + 'static,
    ) {
        let id = req_id.to_string();
        let disk_id = id.clone();
        self.edit_disk(
            "Could Not Delete Request",
            move |store| store.delete_request(&disk_id),
            move |state, (), cx| {
                let index = state
                    .requests
                    .iter()
                    .position(|item| item.id == id)
                    .unwrap_or(0);
                state.remove_request(&id);
                let next = state
                    .requests
                    .get(index.min(state.requests.len().saturating_sub(1)))
                    .map(|request| request.id.clone());
                cx.emit(CollectionEvent::RequestDeleted(id));
                cx.emit(CollectionEvent::PinsModified);
                cx.notify();
                deleted(state, next, cx);
            },
            cx,
        );
    }

    #[cfg(test)]
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
            self.apply_request_rename(req_id, &new_rel_path, trimmed);
            Some(new_rel_path)
        } else {
            None
        }
    }

    fn apply_request_rename(&mut self, old: &str, new: &str, name: &str) {
        if let Some(request) = self.find_request_mut(old) {
            request.id = new.to_string();
            request.name = name.to_string();
        }
        if let Some(pin) = self.pins.iter_mut().find(|pin| pin.as_str() == old) {
            *pin = new.to_string();
        }
        if let Some(error) = self.parse_errors.remove(old) {
            self.parse_errors.insert(new.to_string(), error);
        }
        self.bump_tree_version();
    }

    pub fn rename_request(&mut self, req_id: &str, new_name: &str, cx: &mut Context<Self>) {
        let name = new_name.trim().to_string();
        if name.is_empty() {
            return;
        }
        if self.find_request(req_id).is_none() {
            return;
        }
        let id = req_id.to_string();
        let target = id.clone();
        let disk_name = name.clone();
        self.edit_disk(
            "Could Not Rename Request",
            move |store| store.rename_request(&target, &disk_name),
            move |state, new, cx| {
                state.apply_request_rename(&id, &new, &name);
                if id == new {
                    cx.emit(CollectionEvent::RequestContentModified(id));
                } else {
                    cx.emit(CollectionEvent::IdsRemapped(vec![(id, new)]));
                }
                cx.notify();
            },
            cx,
        );
    }

    pub fn move_request(&mut self, src_req_id: &str, target_req_id: &str, cx: &mut Context<Self>) {
        if src_req_id == target_req_id {
            return;
        }
        let src_req = self.requests.iter().find(|r| r.id == src_req_id);
        let target_req = self.requests.iter().find(|r| r.id == target_req_id);

        if let (Some(src), Some(target)) = (src_req, target_req) {
            let src_eff = self.effective_folder_id(&src.folder_id);
            let target_eff = self.effective_folder_id(&target.folder_id);
            if src_eff != target_eff {
                self.move_request_to_folder(src_req_id, target.folder_id.clone(), cx);
            } else {
                let source_id = src_req_id.to_string();
                let target_id = target_req_id.to_string();
                self.edit_disk_with_snapshot(
                    "Could Not Reorder Requests",
                    move |state| {
                        let folder = state.find_request(&source_id)?.folder_id.clone();
                        let mut requests: Vec<_> = state
                            .requests
                            .iter()
                            .filter(|request| request.folder_id == folder)
                            .collect();
                        let source = requests.iter().position(|r| r.id == source_id)?;
                        let target = requests.iter().position(|r| r.id == target_id)?;
                        let request = requests.remove(source);
                        requests.insert(target, request);
                        let order: Vec<_> = requests
                            .iter()
                            .map(|request| {
                                PathBuf::from(&request.id)
                                    .file_stem()
                                    .and_then(|stem| stem.to_str())
                                    .unwrap_or("req")
                                    .to_string()
                            })
                            .collect();
                        Some(move |store: &dyn WorkspaceStore| {
                            store.reorder_folder(folder.as_deref(), &order)?;
                            Ok((source_id, target_id))
                        })
                    },
                    |state, (source_id, target_id), cx| {
                        let source = state.requests.iter().position(|r| r.id == source_id);
                        let target = state.requests.iter().position(|r| r.id == target_id);
                        if let (Some(source), Some(target)) = (source, target) {
                            let request = state.requests.remove(source);
                            state.requests.insert(target, request);
                            state.bump_tree_version();
                            cx.notify();
                        }
                    },
                    cx,
                );
            }
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

    pub fn reorder_pins(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from >= self.pins.len() || to >= self.pins.len() || from == to {
            return;
        }
        let item = self.pins.remove(from);
        self.pins.insert(to, item);
        cx.emit(CollectionEvent::PinsModified);
        self.bump_tree_version();
        cx.notify();
    }

    pub fn get_request_file(&self, req_id: &str) -> Option<RequestFile> {
        self.find_request(req_id).map(RequestFile::from_item)
    }

    #[cfg(test)]
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
