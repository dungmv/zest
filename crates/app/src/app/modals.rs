use crate::components::*;
use crate::services::GitService;
use crate::state::{
    EnvironmentState, GitState, ModalKind, ModalState, PersistenceState, RequestSessionState,
    SidebarState, WorkspaceDataState,
};
use gpui_kit::{App, Div, Entity, IntoElement, ParentElement, Window};

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
    let env_var_table = entities.env_var_table;

    let active_modal = modal_state.read(cx).active_modal;
    let show_git_modal = git_state.read(cx).modal.is_open;

    let ws_entity = workspace_state.clone();
    let env_entity = env_state.clone();
    let per_entity = persistence_state.clone();
    let modal_entity = modal_state.clone();
    let sidebar_entity = sidebar_state.clone();
    let git_entity = git_state.clone();
    let ed_entity = editor_state.clone();
    let var_table_entity = env_var_table.clone();

    if show_git_modal {
        return root.child(
            GitModal::new(
                git_entity.clone(),
                ws_entity.clone(),
                GitService::new(git_entity.clone()),
            )
            .into_any_element(),
        );
    }

    match active_modal {
        Some(ModalKind::Environment) => root.child(
            EnvironmentModal::new(
                env_entity.clone(),
                per_entity.clone(),
                modal_entity.clone(),
                sidebar_entity.clone(),
                var_table_entity,
            )
            .into_any_element(),
        ),
        Some(ModalKind::Settings) => root
            .child(SettingsModal::new(ws_entity.clone(), modal_entity.clone()).into_any_element()),
        Some(ModalKind::WorkspaceSetting) => root.child(
            WorkspaceSettingModal::new(ws_entity.clone(), env_entity.clone(), modal_entity.clone())
                .into_any_element(),
        ),
        Some(ModalKind::Import) => {
            root.child(ImportModal::new(ws_entity.clone(), modal_entity.clone()).into_any_element())
        }
        Some(ModalKind::Export) => {
            root.child(ExportModal::new(ws_entity.clone(), modal_entity.clone()).into_any_element())
        }
        Some(ModalKind::About) => {
            root.child(AboutModal::new(modal_entity.clone()).into_any_element())
        }
        Some(ModalKind::CommandPalette) => root.child(
            CommandPalette::new(
                ws_entity.clone(),
                env_entity.clone(),
                per_entity.clone(),
                modal_entity.clone(),
                sidebar_entity.clone(),
                ed_entity.clone(),
            )
            .into_any_element(),
        ),
        None => root,
    }
}
