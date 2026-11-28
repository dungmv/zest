//! Sidebar selection, navigation, layout, and tree cache.

mod navigation;
mod renaming;
mod selection;

use gpui_kit::{px, Context, Pixels, UniformListScrollHandle};
use navigation::SidebarNavigation;
use renaming::SidebarRenaming;
use selection::SidebarSelection;
use std::collections::HashSet;

pub struct SidebarState {
    filter_query: String,
    pub sidebar_collapsed: bool,
    pub sidebar_width: Pixels,
    expanded_folders: HashSet<String>,
    expanded_requests: HashSet<String>,
    pinned_expanded: bool,
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
            expanded_folders: HashSet::new(),
            expanded_requests: HashSet::new(),
            pinned_expanded: true,
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

    pub fn reset_workspace_expansion(&mut self, cx: &mut Context<Self>) {
        self.expanded_folders.clear();
        self.expanded_requests.clear();
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub fn toggle_folder_expansion(&mut self, folder_id: &str, cx: &mut Context<Self>) {
        if !self.expanded_folders.remove(folder_id) {
            self.expanded_folders.insert(folder_id.to_string());
        }
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub fn toggle_request_expansion(&mut self, req_id: &str, cx: &mut Context<Self>) {
        if !self.expanded_requests.remove(req_id) {
            self.expanded_requests.insert(req_id.to_string());
        }
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub fn toggle_pinned_expansion(&mut self, cx: &mut Context<Self>) {
        self.pinned_expanded = !self.pinned_expanded;
        self.ui_version = self.ui_version.wrapping_add(1);
        cx.notify();
    }

    pub(crate) fn filter_query(&self) -> &str {
        &self.filter_query
    }
    pub(crate) fn expanded_folders(&self) -> &HashSet<String> {
        &self.expanded_folders
    }
    pub(crate) fn expanded_requests(&self) -> &HashSet<String> {
        &self.expanded_requests
    }
    pub(crate) fn is_pinned_expanded(&self) -> bool {
        self.pinned_expanded
    }
    pub(crate) fn view_revision(&self) -> usize {
        self.ui_version
    }

    pub(crate) fn expand_folder(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.expanded_folders.insert(id.to_string()) {
            self.ui_version = self.ui_version.wrapping_add(1);
            cx.notify();
        }
    }

    pub(crate) fn expand_pins(&mut self, cx: &mut Context<Self>) {
        if !self.pinned_expanded {
            self.pinned_expanded = true;
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
