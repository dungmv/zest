use super::{EnvironmentState, GitState, PersistenceState, WorkspaceDataState};
use crate::components::sidebar::{
    build_sidebar_req_items, flatten_sidebar_tree, FlatSidebarRow, SidebarCacheKey,
    SidebarFlatCache,
};
use gpui_kit::{px, AppContext, Context, Entity, Pixels, UniformListScrollHandle, Window};
use gpui_kit::component::input::InputState;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

pub struct RenamingState<T> {
    pub id: T,
    pub input: Entity<InputState>,
    pub subscription: gpui_kit::Subscription,
}

pub struct SidebarState {
    pub filter_query: String,
    pub sidebar_collapsed: bool,
    pub sidebar_width: Pixels,
    pub collapsed_folders: HashSet<String>,
    pub collapsed_requests: HashSet<String>,
    pub renaming_request: Option<RenamingState<String>>,
    pub renaming_folder: Option<RenamingState<String>>,
    pub renaming_env: Option<RenamingState<String>>,
    pub renaming_example: Option<RenamingState<(String, String)>>,
    pub nav_history: Vec<String>,
    pub nav_index: usize,
    pub ui_version: usize,
    pub scroll_handle: UniformListScrollHandle,
    pub flat_cache: RefCell<SidebarFlatCache>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            filter_query: String::new(),
            sidebar_collapsed: false,
            sidebar_width: px(260.),
            collapsed_folders: HashSet::new(),
            collapsed_requests: HashSet::new(),
            renaming_request: None,
            renaming_folder: None,
            renaming_env: None,
            renaming_example: None,
            nav_history: Vec::new(),
            nav_index: 0,
            ui_version: 0,
            scroll_handle: UniformListScrollHandle::new(),
            flat_cache: RefCell::new(SidebarFlatCache::default()),
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

    pub fn cached_flat_rows(
        &self,
        ws: &WorkspaceDataState,
        per: &PersistenceState,
        git: &GitState,
    ) -> Arc<Vec<FlatSidebarRow>> {
        let has_query = !self.filter_query.is_empty();
        let url_ver = if has_query { ws.url_version } else { 0 };
        let key = SidebarCacheKey {
            tree_version: ws.tree_version,
            url_version_if_filtering: url_ver,
            git_status_version: git.status_version,
            unsaved_version: per.unsaved_version,
            ui_version: self.ui_version,
        };

        let mut cache = self.flat_cache.borrow_mut();
        if cache.key == Some(key) {
            return cache.rows.clone();
        }

        let req_ver = (ws.tree_version, url_ver);
        if cache.req_items_version != Some(req_ver) {
            let (raw_items, arc_items) = build_sidebar_req_items(&ws.requests);
            cache.req_raw_items = Arc::new(raw_items);
            cache.req_items = Arc::new(arc_items);
            cache.req_items_version = Some(req_ver);
        }

        let query_lower = self.filter_query.to_lowercase();
        let rows = flatten_sidebar_tree(
            &ws.folders,
            &cache.req_raw_items,
            &cache.req_items,
            &ws.pins,
            &query_lower,
            &self.collapsed_folders,
            &self.collapsed_requests,
            git.status.as_ref(),
            &per.unsaved_requests,
        );
        let arc_rows = Arc::new(rows);
        cache.key = Some(key);
        cache.rows = arc_rows.clone();
        arc_rows
    }

    pub fn record_navigation(&mut self, req_id: &str) {
        if self.nav_history.get(self.nav_index).map(|s| s.as_str()) == Some(req_id) {
            return;
        }
        if self.nav_index + 1 < self.nav_history.len() {
            self.nav_history.truncate(self.nav_index + 1);
        }
        self.nav_history.push(req_id.to_string());
        self.nav_index = self.nav_history.len() - 1;
    }

    pub fn navigate_back(&mut self) -> Option<String> {
        if self.nav_index > 0 {
            self.nav_index -= 1;
            self.nav_history.get(self.nav_index).cloned()
        } else {
            None
        }
    }

    pub fn navigate_forward(&mut self) -> Option<String> {
        if self.nav_index + 1 < self.nav_history.len() {
            self.nav_index += 1;
            self.nav_history.get(self.nav_index).cloned()
        } else {
            None
        }
    }

    pub fn cancel_renaming(&mut self, cx: &mut Context<Self>) {
        self.cancel_renaming_request(cx);
    }

    /// Creates the shared inline-rename input: prefilled, fully selected, focused,
    /// and committed on Enter/Blur through `on_commit`. Cancelling always goes
    /// through [`Self::cancel_any_renaming`], which drops the subscription together
    /// with the input so a cancelled rename never commits.
    fn spawn_renaming<T: 'static>(
        &mut self,
        id: T,
        initial_value: String,
        on_commit: impl Fn(&str, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> RenamingState<T> {
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(initial_value, window, cx);
            s.select_all(window, cx);
            s.focus(window, cx);
            s
        });

        let sub = cx.subscribe(
            &input,
            move |this, input, event: &gpui_kit::component::input::InputEvent, cx| match event {
                gpui_kit::component::input::InputEvent::PressEnter { .. }
                | gpui_kit::component::input::InputEvent::Blur => {
                    let value = input.read(cx).value().to_string();
                    this.cancel_any_renaming(cx);
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        on_commit(trimmed, cx);
                    }
                }
                _ => {}
            },
        );

        RenamingState {
            id,
            input,
            subscription: sub,
        }
    }

    pub fn start_renaming_request(
        &mut self,
        req_id: String,
        initial_name: String,
        ws: Entity<WorkspaceDataState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let r_id = req_id.clone();
        self.renaming_request = Some(self.spawn_renaming(
            req_id,
            initial_name,
            move |name, cx| {
                ws.update(cx, |ws, cx| ws.rename_request(&r_id, name, cx));
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn start_renaming_folder(
        &mut self,
        folder_id: String,
        initial_name: String,
        ws: Entity<WorkspaceDataState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let f_id = folder_id.clone();
        self.renaming_folder = Some(self.spawn_renaming(
            folder_id,
            initial_name,
            move |name, cx| {
                ws.update(cx, |ws, cx| ws.rename_folder(&f_id, name.to_string(), cx));
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn start_renaming_example(
        &mut self,
        req_id: String,
        example_name: String,
        ws: Entity<WorkspaceDataState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let r_id = req_id.clone();
        let old_ex_name = example_name.clone();
        self.renaming_example = Some(self.spawn_renaming(
            (req_id, example_name.clone()),
            example_name,
            move |name, cx| {
                if name != old_ex_name {
                    ws.update(cx, |ws, cx| {
                        ws.rename_example(&r_id, &old_ex_name, name, cx)
                    });
                }
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn start_renaming_environment(
        &mut self,
        env_id: String,
        initial_name: String,
        env_state: Entity<EnvironmentState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let e_id = env_id.clone();
        self.renaming_env = Some(self.spawn_renaming(
            env_id,
            initial_name,
            move |name, cx| {
                env_state.update(cx, |env, cx| {
                    env.rename_environment(&e_id, name.to_string(), cx)
                });
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn renaming_request_id(&self) -> Option<&str> {
        self.renaming_request.as_ref().map(|r| r.id.as_str())
    }

    pub fn rename_input(&self) -> Option<&Entity<InputState>> {
        self.renaming_request.as_ref().map(|r| &r.input)
    }

    pub fn renaming_folder_id(&self) -> Option<&str> {
        self.renaming_folder.as_ref().map(|r| r.id.as_str())
    }

    pub fn rename_folder_input(&self) -> Option<&Entity<InputState>> {
        self.renaming_folder.as_ref().map(|r| &r.input)
    }

    pub fn renaming_env_id(&self) -> Option<&str> {
        self.renaming_env.as_ref().map(|r| r.id.as_str())
    }

    pub fn rename_env_input(&self) -> Option<&Entity<InputState>> {
        self.renaming_env.as_ref().map(|r| &r.input)
    }

    pub fn renaming_example(&self) -> Option<&(String, String)> {
        self.renaming_example.as_ref().map(|r| &r.id)
    }

    pub fn rename_example_input(&self) -> Option<&Entity<InputState>> {
        self.renaming_example.as_ref().map(|r| &r.input)
    }

    pub fn cancel_renaming_request(&mut self, cx: &mut Context<Self>) {
        if self.renaming_request.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_renaming_folder(&mut self, cx: &mut Context<Self>) {
        if self.renaming_folder.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_renaming_environment(&mut self, cx: &mut Context<Self>) {
        if self.renaming_env.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_renaming_example(&mut self, cx: &mut Context<Self>) {
        if self.renaming_example.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_any_renaming(&mut self, cx: &mut Context<Self>) -> bool {
        let mut cancelled = false;
        if self.renaming_request.take().is_some() {
            cancelled = true;
        }
        if self.renaming_folder.take().is_some() {
            cancelled = true;
        }
        if self.renaming_env.take().is_some() {
            cancelled = true;
        }
        if self.renaming_example.take().is_some() {
            cancelled = true;
        }
        if cancelled {
            cx.notify();
        }
        cancelled
    }
}
