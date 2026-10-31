use crate::components::*;
use crate::services::GitService;
use crate::state::{
    EnvironmentState, GitState, ModalState, PersistenceState, RequestSessionState, SidebarState,
    ToastState, WorkspaceDataState,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{App, Div, Entity, ParentElement, Window};

pub struct ModalOverlayEntities<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub persistence_state: &'a Entity<PersistenceState>,
    pub modal_state: &'a Entity<ModalState>,
    pub sidebar_state: &'a Entity<SidebarState>,
    pub git_state: &'a Entity<GitState>,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub toast_state: &'a Entity<ToastState>,
}

pub fn attach_modal_overlays(
    root: Div,
    entities: ModalOverlayEntities,
    _send_request_cb: impl 'static + Clone + Fn(&mut Window, &mut App),
    cx: &mut App,
) -> Div {
    let workspace_state = entities.workspace_state;
    let env_state = entities.env_state;
    let persistence_state = entities.persistence_state;
    let modal_state = entities.modal_state;
    let sidebar_state = entities.sidebar_state;
    let git_state = entities.git_state;
    let editor_state = entities.editor_state;
    let toast_state = entities.toast_state;

    let modal = modal_state.read(cx);
    let show_env_modal = modal.show_env_modal;
    let show_settings_modal = modal.show_settings_modal;
    let show_scope_settings_modal = modal.scope_settings_target.is_some();
    let show_import_modal = modal.show_import_modal;
    let show_command_palette = modal.show_command_palette;

    let show_git_modal = git_state.read(cx).show_modal;

    let ws_entity = workspace_state.clone();
    let env_entity = env_state.clone();
    let per_entity = persistence_state.clone();
    let modal_entity = modal_state.clone();
    let sidebar_entity = sidebar_state.clone();
    let git_entity = git_state.clone();
    let ed_entity = editor_state.clone();
    let toast_entity = toast_state.clone();

    root.when(show_env_modal, |this| {
        this.child(EnvironmentModal::new(
            env_entity.clone(),
            modal_entity.clone(),
            sidebar_entity.clone(),
        ))
    })
    .when(show_settings_modal, |this| {
        this.child(SettingsModal::new(ws_entity.clone(), modal_entity.clone()))
    })
    .when(show_scope_settings_modal, |this| {
        this.child(ScopeSettingsModal::new(
            ws_entity.clone(),
            env_entity.clone(),
            modal_entity.clone(),
        ))
    })
    .when(show_git_modal, |this| {
        this.child(GitModal::new(
            git_entity.clone(),
            ws_entity.clone(),
            GitService::new(git_entity.clone(), toast_entity.clone()),
        ))
    })
    .when(show_import_modal, |this| {
        this.child(ImportModal::new(
            ws_entity.clone(),
            modal_entity.clone(),
            toast_entity.clone(),
        ))
    })
    .when(show_command_palette, move |this| {
        this.child(CommandPalette::new(
            ws_entity.clone(),
            env_entity.clone(),
            per_entity.clone(),
            modal_entity.clone(),
            sidebar_entity.clone(),
            ed_entity.clone(),
        ))
    })
    // Toast Notifications container bottom-right
    .child(ToastContainer::new(toast_entity))
}
