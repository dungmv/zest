use std::collections::HashSet;
use zest_core::storage::{
    EnvironmentSnapshot, FsEvent, RequestFile, WorkspaceMeta, WorkspaceStore,
};

#[derive(Default)]
pub(crate) struct WatchBatch {
    pub(crate) git_changed: bool,
    pub(crate) tree_changed: bool,
    pub(crate) workspace_meta: Option<WorkspaceMeta>,
    pub(crate) changed_envs: HashSet<String>,
    pub(crate) changed_requests: HashSet<String>,
    pub(crate) deleted_requests: HashSet<String>,
    pub(crate) request_files: Vec<RequestReload>,
    pub(crate) environment: Option<EnvironmentSnapshot>,
}

pub(crate) struct RequestReload {
    pub(crate) id: String,
    pub(crate) file: Result<RequestFile, String>,
    pub(crate) last_status: Option<u16>,
}

impl RequestReload {
    fn from_read(
        id: String,
        file: anyhow::Result<RequestFile>,
        store: &dyn WorkspaceStore,
    ) -> Self {
        let file = file.map_err(|error| error.to_string());
        let last_status = store
            .load_response_cache(&id)
            .ok()
            .flatten()
            .map(|cache| cache.response.status);
        Self {
            id,
            file,
            last_status,
        }
    }

    pub(crate) fn read(id: String, store: &dyn WorkspaceStore) -> Self {
        let file = store.read_request(&id);
        Self::from_read(id, file, store)
    }

    pub(crate) fn read_if_present(id: String, store: &dyn WorkspaceStore) -> Option<Self> {
        let file = store.read_request(&id);
        if file
            .as_ref()
            .err()
            .and_then(|error| error.downcast_ref::<std::io::Error>())
            .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        {
            None
        } else {
            Some(Self::from_read(id, file, store))
        }
    }
}

impl WatchBatch {
    pub(crate) fn read(
        events: Vec<FsEvent>,
        store: &dyn WorkspaceStore,
        workspace_name: &str,
    ) -> Self {
        let mut batch = Self::default();
        let mut workspace_meta_changed = false;
        for event in events {
            match event {
                FsEvent::GitChanged => batch.git_changed = true,
                FsEvent::TreeChanged => batch.tree_changed = true,
                FsEvent::WorkspaceMetaChanged => workspace_meta_changed = true,
                FsEvent::EnvironmentChanged(name) => {
                    batch.changed_envs.insert(name);
                }
                FsEvent::RequestChanged(id) => {
                    batch.changed_requests.insert(id);
                }
                FsEvent::RequestDeleted(id) => {
                    batch.deleted_requests.insert(id);
                }
            }
        }

        if workspace_meta_changed {
            batch.workspace_meta = store.load_workspace_meta().ok();
            batch.git_changed = true;
        }

        // An atomic replace can deliver both removal and creation. Recognize
        // the replacement even when the events arrive in different batches.
        let collections = store.workspace_root().join("collections");
        batch.deleted_requests.retain(|id| {
            if collections.join(id).is_file() {
                batch.tree_changed = true;
                batch.changed_requests.insert(id.clone());
                false
            } else {
                batch.changed_requests.remove(id);
                true
            }
        });

        if !batch.tree_changed {
            batch.request_files = batch
                .changed_requests
                .iter()
                .map(|id| RequestReload::read(id.clone(), store))
                .collect();
        }
        if !batch.changed_envs.is_empty() {
            batch.environment = Some(EnvironmentSnapshot::load(store, workspace_name));
        }
        batch
    }
}
