use crate::components::*;
use crate::modals::*;
use crate::services::GitService;
use crate::state::{
    EnvironmentState, GitState, ModalKind, ModalState, PersistenceState, RequestSessionState,
    SidebarState, WorkspaceDataState,
};
use gpui_kit::{App, Div, Entity, IntoElement, ParentElement};

pub struct ModalOverlayEntities<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub persistence_state: &'a Entity<PersistenceState>,
    pub modal_state: &'a Entity<ModalState>,
    pub sidebar_state: &'a Entity<SidebarState>,
    pub git_state: &'a Entity<GitState>,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub env_var_table: &'a Entity<EnvVarTable>,
}

pub fn attach_modal_overlays(root: Div, entities: ModalOverlayEntities, cx: &mut App) -> Div {
    let active_modal = entities.modal_state.read(cx).active_modal;
    let show_git_modal = entities.git_state.read(cx).modal.is_open;

    // GitModal is managed by GitState outside the ModalKind registry.
    if show_git_modal {
        return root.child(
            GitModal::new(
                entities.git_state.clone(),
                entities.workspace_state.clone(),
                GitService::new(entities.git_state.clone()),
            )
            .into_any_element(),
        );
    }

    match active_modal {
        Some(ModalKind::Environment) => root.child(
            EnvironmentModal::new(
                entities.env_state.clone(),
                entities.persistence_state.clone(),
                entities.modal_state.clone(),
                entities.sidebar_state.clone(),
                entities.env_var_table.clone(),
            )
            .into_any_element(),
        ),
        Some(ModalKind::Settings) => root.child(
            SettingsModal::new(
                entities.workspace_state.clone(),
                entities.modal_state.clone(),
            )
            .into_any_element(),
        ),
        Some(ModalKind::WorkspaceSetting) => root.child(
            WorkspaceSettingModal::new(
                entities.workspace_state.clone(),
                entities.env_state.clone(),
                entities.modal_state.clone(),
            )
            .into_any_element(),
        ),
        Some(ModalKind::Import) => root.child(
            ImportModal::new(
                entities.workspace_state.clone(),
                entities.modal_state.clone(),
            )
            .into_any_element(),
        ),
        Some(ModalKind::Export) => root.child(
            ExportModal::new(
                entities.workspace_state.clone(),
                entities.modal_state.clone(),
            )
            .into_any_element(),
        ),
        Some(ModalKind::About) => {
            root.child(AboutModal::new(entities.modal_state.clone()).into_any_element())
        }
        Some(ModalKind::CommandPalette) => root.child(
            CommandPalette::new(
                entities.workspace_state.clone(),
                entities.env_state.clone(),
                entities.persistence_state.clone(),
                entities.modal_state.clone(),
                entities.sidebar_state.clone(),
                entities.editor_state.clone(),
            )
            .into_any_element(),
        ),
        None => root,
    }
}
