use super::{WorkspaceDataState, WorkspaceEvent};
use crate::settings::AppSettings;
use gpui_kit::Context;
use std::path::PathBuf;
use zest_core::models::{HttpMethod, HttpRequestItem, Protocol};
use zest_core::storage::models::RequestFile;

impl WorkspaceDataState {
    /// Conventional HTTP verb persisted for a request of the given protocol.
    /// WS handshake is a GET; gRPC rides on HTTP/2 POST.
    fn protocol_method(protocol: Protocol) -> HttpMethod {
        match protocol {
            Protocol::Http | Protocol::WebSocket => HttpMethod::Get,
            Protocol::Grpc => HttpMethod::Post,
        }
    }

    pub fn create_request_internal(&mut self, folder_rel: Option<String>) -> HttpRequestItem {
        self.create_request_internal_typed(folder_rel, Protocol::Http)
    }

    pub fn create_request_internal_typed(
        &mut self,
        folder_rel: Option<String>,
        protocol: Protocol,
    ) -> HttpRequestItem {
        let method = Self::protocol_method(protocol);
        let (_slug, rel_path) = self
            .store
            .create_request_typed(folder_rel.as_deref(), "New Request", method, "", protocol)
            .unwrap_or_else(|_| {
                (
                    format!("req-{}", uuid::Uuid::new_v4().simple()),
                    "new-request.yaml".to_string(),
                )
            });

        let ws_name = self.workspace_meta.name.clone();
        let mut item =
            HttpRequestItem::new_with_protocol(&ws_name, "New Request", method, "", protocol);
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

    pub fn create_request_typed(
        &mut self,
        folder_rel: Option<String>,
        protocol: Protocol,
        cx: &mut Context<Self>,
    ) -> HttpRequestItem {
        let item = self.create_request_internal_typed(folder_rel, protocol);
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
            cx.emit(crate::state::WorkspaceEvent::WorkspaceMetaModified);
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
        cx.emit(crate::state::WorkspaceEvent::PinsModified);
        self.bump_tree_version();
        cx.notify();
    }

    pub fn is_pinned(&self, req_id: &str) -> bool {
        self.pins.contains(&req_id.to_string())
    }

    pub fn reorder_pins(&mut self, new_pins: Vec<String>, cx: &mut Context<Self>) {
        self.pins = new_pins;
        let _ = self.store.save_pins(&self.pins);
        cx.emit(crate::state::WorkspaceEvent::PinsModified);
        self.bump_tree_version();
        cx.notify();
    }

    pub fn get_request_file(&self, req_id: &str) -> Option<RequestFile> {
        self.store.read_request(req_id).ok()
    }

    pub(super) fn flush_request_to_disk(&self, req_id: &str) {
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
