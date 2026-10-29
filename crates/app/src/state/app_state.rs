use crate::state::{
    EnvironmentState, GitState, ModalState, PersistenceState, RequestSessionState, ResponseState,
    SidebarState, ToastState, WorkspaceDataState,
};
use gpui_kit::{App, AppContext as _, Entity};

pub struct AppState {
    pub workspace: Entity<WorkspaceDataState>,
    pub env: Entity<EnvironmentState>,
    pub persistence: Entity<PersistenceState>,
    pub sidebar: Entity<SidebarState>,
    pub modal: Entity<ModalState>,
    pub git: Entity<GitState>,
    pub session: Entity<RequestSessionState>,
    pub response: Entity<ResponseState>,
    pub toast: Entity<ToastState>,
}

impl AppState {
    pub fn new(cx: &mut App) -> Self {
        let workspace = cx.new(|_| WorkspaceDataState::new());
        let (ws_dir, ws_store, ws_name) = {
            let ws = workspace.read(cx);
            (
                ws.workspace_path.clone(),
                ws.store.clone(),
                ws.active_workspace
                    .as_ref()
                    .map(|w| w.name.clone())
                    .unwrap_or_else(|| "Workspace".to_string()),
            )
        };
        let persistence = cx.new(|_| PersistenceState::new(ws_store.clone(), ws_dir.clone()));
        let initial_active_env = persistence.read(cx).local_state.active_environment.clone();
        let env = cx.new(|_| EnvironmentState::new(ws_store, ws_name, initial_active_env));

        let git = cx.new(|_| GitState::new(ws_dir));
        git.update(cx, |g, cx| {
            g.refresh_status(cx);
        });

        persistence.update(cx, |p, cx| {
            p.start_watcher(workspace.clone(), env.clone(), git.clone(), cx);
        });

        let sidebar = cx.new(|_| SidebarState::new());
        let modal = cx.new(|_| ModalState::new());
        let session = cx.new(|_| RequestSessionState::new());
        let response = cx.new(|_| ResponseState::new());
        let toast = cx.new(|_| ToastState::new());

        Self {
            workspace,
            env,
            persistence,
            sidebar,
            modal,
            git,
            session,
            response,
            toast,
        }
    }
}
