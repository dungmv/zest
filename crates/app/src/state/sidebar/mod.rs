//! Sidebar selection, navigation, layout, and tree cache.

mod navigation;
mod renaming;
mod selection;

use super::CollectionState;
use gpui_kit::{px, Context, Entity, Pixels, UniformListScrollHandle};
use navigation::SidebarNavigation;
use renaming::SidebarRenaming;
use selection::SidebarSelection;
use std::collections::HashSet;

pub struct SidebarState {
    filter_query: String,
    pub sidebar_collapsed: bool,
    pub sidebar_width: Pixels,
    collapsed_folders: HashSet<String>,
    collapsed_requests: HashSet<String>,
    pinned_collapsed: bool,
    pub renaming: SidebarRenaming,
    navigation: SidebarNavigation,
    ui_version: usize,
    pub scroll_handle: UniformListScrollHandle,
    selection: SidebarSelection,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            filter_query: String::new(),
            sidebar_collapsed: false,
            sidebar_width: px(260.),
            collapsed_folders: HashSet::new(),
            collapsed_requests: HashSet::new(),
            pinned_collapsed: false,
            renaming: SidebarRenaming::default(),
            navigation: SidebarNavigation::default(),
            ui_version: 0,
            scroll_handle: UniformListScrollHandle::new(),
            selection: SidebarSelection::default(),
        }
    }
}

impl SidebarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.filter_query != query {
            self.filter_query = query;
            self.ui_version = self.ui_version.wrapping_add(1);
            cx.notify();
        }
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    pub fn set_sidebar_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        if self.sidebar_width != width {
            self.sidebar_width = width;
            cx.notify();
        }
    }

    pub fn collapse_workspace_folders(
        &mut self,
        collection: &Entity<CollectionState>,
        cx: &mut Context<Self>,
    ) {
        self.collapsed_folders = collection
            .read(cx)
            .folders
            .iter()
            .map(|folder| folder.id.clone())
            .collect();
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub fn toggle_folder_collapsed(&mut self, folder_id: &str, cx: &mut Context<Self>) {
        if self.collapsed_folders.contains(folder_id) {
            self.collapsed_folders.remove(folder_id);
        } else {
            self.collapsed_folders.insert(folder_id.to_string());
        }
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub fn toggle_request_collapsed(&mut self, req_id: &str, cx: &mut Context<Self>) {
        if self.collapsed_requests.contains(req_id) {
            self.collapsed_requests.remove(req_id);
        } else {
            self.collapsed_requests.insert(req_id.to_string());
        }
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub fn toggle_pinned_collapsed(&mut self, cx: &mut Context<Self>) {
        self.pinned_collapsed = !self.pinned_collapsed;
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub(crate) fn filter_query(&self) -> &str {
        &self.filter_query
    }
    pub(crate) fn collapsed_folders(&self) -> &HashSet<String> {
        &self.collapsed_folders
    }
    pub(crate) fn collapsed_requests(&self) -> &HashSet<String> {
        &self.collapsed_requests
    }
    pub(crate) fn is_pinned_collapsed(&self) -> bool {
        self.pinned_collapsed
    }
    pub(crate) fn view_revision(&self) -> usize {
        self.ui_version
    }

    pub(crate) fn expand_folder(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.collapsed_folders.remove(id) {
            self.ui_version = self.ui_version.wrapping_add(1);
            cx.notify();
        }
    }

    pub(crate) fn expand_pins(&mut self, cx: &mut Context<Self>) {
        if self.pinned_collapsed {
            self.pinned_collapsed = false;
            self.ui_version = self.ui_version.wrapping_add(1);
            cx.notify();
        }
    }

    pub fn record_navigation(&mut self, req_id: &str) {
        self.navigation.record_navigation(req_id);
    }

    pub fn navigate_back(&mut self) -> Option<String> {
        self.navigation.navigate_back()
    }

    pub fn navigate_forward(&mut self) -> Option<String> {
        self.navigation.navigate_forward()
    }
}

#[cfg(test)]
mod tests;
