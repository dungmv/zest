use crate::state::{
    CollectionState, EnvironmentState, GitState, ModalState, PersistenceState, RequestSessionState,
    ResponseState, SidebarState, WorkspaceState,
};
use gpui_kit::{App, AppContext as _, Entity};

pub struct AppState {
    pub workspace: Entity<WorkspaceState>,
    pub collection: Entity<CollectionState>,
    pub env: Entity<EnvironmentState>,
    pub persistence: Entity<PersistenceState>,
    pub sidebar: Entity<SidebarState>,
    pub modal: Entity<ModalState>,
    pub git: Entity<GitState>,
    pub session: Entity<RequestSessionState>,
    pub response: Entity<ResponseState>,
}

impl AppState {
    pub fn new(cx: &mut App) -> Self {
        let workspace = cx.new(|_| WorkspaceState::new());
        let (ws_dir, ws_store, ws_name) = {
            let ws = workspace.read(cx);
            (
                ws.workspace_path.clone(),
                ws.store.clone(),
                ws.workspace_name().to_string(),
            )
        };
        // .gitignore upkeep shells out to `git` (up to 30s) — keep it off the UI thread.
        {
            let store = ws_store.clone();
            cx.background_executor()
                .spawn(async move {
                    let _ = store.ensure_gitignore();
                })
                .detach();
        }

        let collection = cx.new(|cx| {
            let mut col = CollectionState::new(ws_store.clone(), ws_dir.clone(), ws_name.clone());
            // The full store scan (meta + every environment/request YAML) is
            // O(tree bytes) — run it on the background executor; initial
            // selection replays via `RequestsReloaded` once it lands.
            col.load_workspace_async(cx, false);
            col
        });

        let persistence = cx.new(|_| PersistenceState::new(ws_store.clone(), ws_dir.clone()));
        let initial_active_env = persistence.read(cx).local_state.active_environment.clone();
        let env = cx.new(|_| EnvironmentState::new(ws_store, ws_name, initial_active_env));

        let git = cx.new(|_| GitState::new(ws_dir));
        git.update(cx, |g, cx| {
            g.refresh_status(cx);
        });

        persistence.update(cx, |p, cx| {
            p.start_watcher(collection.clone(), env.clone(), git.clone(), cx);
        });

        let sidebar = cx.new(|_| SidebarState::new());
        let modal = cx.new(|_| ModalState::new());
        let session = cx.new(|_| RequestSessionState::new());
        let response = cx.new(|_| ResponseState::new());

        Self {
            workspace,
            collection,
            env,
            persistence,
            sidebar,
            modal,
            git,
            session,
            response,
        }
    }
}
