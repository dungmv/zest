use crate::settings::AppSettings;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zest_core::models::StoredCookie;
use zest_core::storage::collection::CollectionSnapshot;
use zest_core::storage::{
    EnvironmentSnapshot, FsWorkspaceStore, LocalState, WorkspaceMeta, WorkspaceStore,
};

/// Read-only snapshot of everything the workspace store holds. Produced by
/// [`CollectionState::scan_workspace_blocking`] on a background thread and
/// applied on the UI thread.
pub(crate) struct WorkspaceScan {
    pub(crate) workspace_meta: Option<WorkspaceMeta>,
    pub(crate) local_state: LocalState,
    pub(crate) environment: EnvironmentSnapshot,
    pub(crate) pins: Vec<String>,
    pub(crate) cookies: Vec<StoredCookie>,
    pub(crate) tree: Option<CollectionSnapshot>,
    pub(crate) folder_name: String,
    pub(crate) store: Arc<dyn WorkspaceStore>,
}

pub(crate) fn scan_workspace_blocking(
    store: Arc<dyn WorkspaceStore>,
    folder_name: String,
) -> WorkspaceScan {
    if !store.workspace_root().join("workspace.yaml").exists() {
        let _ = store.init_workspace(&folder_name);
    }
    let workspace_meta = store.load_workspace_meta().ok();
    let workspace_name = workspace_meta
        .as_ref()
        .map(|meta| meta.name.as_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&folder_name);
    let environment = EnvironmentSnapshot::load(store.as_ref(), workspace_name);
    let tree = scan_collection(store.as_ref(), workspace_name).ok();
    WorkspaceScan {
        workspace_meta,
        local_state: store.load_state().unwrap_or_default(),
        environment,
        pins: store.load_pins().unwrap_or_default(),
        cookies: read_cookies(store.as_ref()),
        tree,
        folder_name,
        store,
    }
}

pub(crate) fn scan_collection(
    store: &dyn WorkspaceStore,
    workspace_name: &str,
) -> anyhow::Result<CollectionSnapshot> {
    store
        .scan_tree()
        .map(|tree| CollectionSnapshot::from_tree(tree, workspace_name))
}

pub(crate) fn read_cookies(store: &dyn WorkspaceStore) -> Vec<StoredCookie> {
    store
        .load_cookies()
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

pub(crate) fn save_cookies(
    store: &dyn WorkspaceStore,
    cookies: Vec<StoredCookie>,
) -> anyhow::Result<()> {
    store.save_cookies(&serde_json::to_value(cookies)?)
}

pub(crate) fn workspace_folder_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Workspace")
}

pub(crate) fn startup_workspace(settings: &AppSettings) -> PathBuf {
    let path = settings
        .current_workspace
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .unwrap_or_else(|| {
            dirs::document_dir()
                .map(|dir| dir.join("zest"))
                .or_else(|| dirs::home_dir().map(|home| home.join("Documents").join("zest")))
                .unwrap_or_else(|| PathBuf::from("./zest"))
        });
    path.canonicalize().unwrap_or(path)
}

pub(crate) fn prepare_workspace(
    path: PathBuf,
    new_name: Option<String>,
) -> anyhow::Result<(PathBuf, Arc<dyn WorkspaceStore>)> {
    let path = path.canonicalize().unwrap_or(path);
    let store = Arc::new(FsWorkspaceStore::new(&path));
    if let Some(name) = new_name {
        store.init_workspace(&name)?;
    } else if !path.join("workspace.yaml").exists() {
        store.init_workspace(workspace_folder_name(&path))?;
    }
    Ok((path, store))
}

pub(crate) fn open_workspace_folder(path: PathBuf) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

pub(crate) fn create_environment(
    store: &dyn WorkspaceStore,
    name: &str,
    workspace: &str,
    template: Option<zest_core::models::Environment>,
) -> anyhow::Result<zest_core::models::Environment> {
    if store
        .list_environments()?
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(name))
    {
        anyhow::bail!("Environment already exists");
    }
    let template = template.unwrap_or_else(|| zest_core::models::Environment::new(workspace, name));
    let (file, secrets) = zest_core::storage::EnvironmentFile::from_environment(&template);
    store.save_environment_values(name, &file, &secrets)?;
    Ok(file.to_environment(name, workspace, name, &secrets))
}

#[derive(Default)]
struct CookieWrites(Option<tokio::sync::watch::Receiver<()>>);
impl gpui_kit::Global for CookieWrites {}

pub(crate) fn save_cookies_later(
    store: Arc<dyn WorkspaceStore>,
    cookies: Vec<StoredCookie>,
    cx: &mut gpui_kit::App,
) {
    if cx.try_global::<CookieWrites>().is_none() {
        cx.set_global(CookieWrites::default());
    }
    let (complete, receiver) = tokio::sync::watch::channel(());
    let previous = cx.global_mut::<CookieWrites>().0.replace(receiver);
    cx.background_executor()
        .spawn(async move {
            let _complete = complete;
            if let Some(mut previous) = previous {
                let _ = previous.changed().await;
            }
            let _ = save_cookies(store.as_ref(), cookies);
        })
        .detach();
}
