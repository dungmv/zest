use super::SidebarState;
use gpui_kit::Context;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct SidebarSelection {
    requests: HashSet<String>,
    folders: HashSet<String>,
}

impl SidebarSelection {
    fn len(&self) -> usize {
        self.requests.len() + self.folders.len()
    }

    fn clear(&mut self) {
        self.requests.clear();
        self.folders.clear();
    }

    fn drain(&mut self) -> (Vec<String>, Vec<String>) {
        (
            self.requests.drain().collect(),
            self.folders.drain().collect(),
        )
    }

    fn select(&mut self, id: &str, folder: bool) -> bool {
        let selected = if folder {
            &self.folders
        } else {
            &self.requests
        };
        if self.len() == 1 && selected.contains(id) {
            return false;
        }
        self.clear();
        let selected = if folder {
            &mut self.folders
        } else {
            &mut self.requests
        };
        selected.insert(id.to_string());
        true
    }

    fn seed_active_request(&mut self, active: Option<&str>) {
        if self.len() == 0 {
            if let Some(active) = active.filter(|id| *id != "scratch") {
                self.requests.insert(active.to_string());
            }
        }
    }

    fn toggle_request(&mut self, id: &str, active: Option<&str>) {
        self.seed_active_request(active.filter(|active| *active != id));
        if !self.requests.remove(id) {
            self.requests.insert(id.to_string());
        }
    }

    fn toggle_folder(&mut self, id: &str, active: Option<&str>) {
        self.seed_active_request(active);
        if !self.folders.remove(id) {
            self.folders.insert(id.to_string());
        }
    }
}

impl SidebarState {
    pub fn selected_requests(&self) -> &HashSet<String> {
        &self.selection.requests
    }

    pub fn selected_folders(&self) -> &HashSet<String> {
        &self.selection.folders
    }

    pub fn is_request_selected(&self, id: &str) -> bool {
        self.selection.requests.contains(id)
    }

    pub fn is_folder_selected(&self, id: &str) -> bool {
        self.selection.folders.contains(id)
    }

    pub fn selected_count(&self) -> usize {
        self.selection.len()
    }

    pub fn is_multi_selected(&self) -> bool {
        self.selected_count() > 1
    }

    fn selection_changed(&mut self, cx: &mut Context<Self>) {
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.len() != 0 {
            self.selection.clear();
            self.selection_changed(cx);
        }
    }

    pub fn take_selection(&mut self, cx: &mut Context<Self>) -> (Vec<String>, Vec<String>) {
        let selection = self.selection.drain();
        self.selection_changed(cx);
        selection
    }

    pub fn select_single_request(&mut self, req_id: &str, cx: &mut Context<Self>) {
        if self.selection.select(req_id, false) {
            self.selection_changed(cx);
        }
    }

    pub fn select_single_folder(&mut self, folder_id: &str, cx: &mut Context<Self>) {
        if self.selection.select(folder_id, true) {
            self.selection_changed(cx);
        }
    }

    pub fn toggle_select_request(
        &mut self,
        req_id: &str,
        active_req_id: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.selection.toggle_request(req_id, active_req_id);
        self.selection_changed(cx);
    }

    pub fn toggle_select_folder(
        &mut self,
        folder_id: &str,
        active_req_id: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.selection.toggle_folder(folder_id, active_req_id);
        self.selection_changed(cx);
    }
}
