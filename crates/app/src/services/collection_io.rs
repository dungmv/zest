use crate::state::{CollectionEvent, CollectionState};
use gpui_kit::Context;
use std::sync::Arc;
use tokio::sync::watch;
use zest_core::models::{HttpMethod, Protocol, RequestItem};
use zest_core::storage::{RequestFile, WorkspaceStore};

pub(crate) fn create_request(
    store: &dyn WorkspaceStore,
    folder: Option<&str>,
    protocol: Protocol,
) -> anyhow::Result<(String, RequestFile)> {
    let method = match protocol {
        Protocol::Http | Protocol::WebSocket | Protocol::Sse => HttpMethod::Get,
        Protocol::Grpc | Protocol::GraphQL => HttpMethod::Post,
    };
    let (_, id) = store.create_request(folder, "New Request", method, "", protocol)?;
    let request = RequestItem::new_with_protocol("", "New Request", method, "", protocol);
    Ok((id, RequestFile::from_item(&request)))
}

pub(crate) fn duplicate_request(
    store: &dyn WorkspaceStore,
    folder: Option<&str>,
    mut file: RequestFile,
) -> anyhow::Result<(String, RequestFile)> {
    file.name = format!("{} (Copy)", file.name);
    let (_, path) = store.create_request_file(folder, file.clone())?;
    Ok((path, file))
}

/// Disk edits run in user order. The next one waits for the previous result
/// to be applied on the UI thread, and the workspace switch can wait on the tail.
pub(crate) fn enqueue<T: Send + 'static, Work>(
    tail: &mut Option<watch::Receiver<()>>,
    store: Arc<dyn WorkspaceStore>,
    title: &'static str,
    prepare: impl FnOnce(&CollectionState) -> Option<Work> + 'static,
    apply: impl FnOnce(&mut CollectionState, T, &mut Context<CollectionState>) + 'static,
    cx: &mut Context<CollectionState>,
) where
    Work: FnOnce(&dyn WorkspaceStore) -> anyhow::Result<T> + Send + 'static,
{
    let (complete, receiver) = watch::channel(());
    let previous = tail.replace(receiver);
    cx.spawn(async move |this, cx| {
        let _complete = complete;
        if let Some(mut previous) = previous {
            let _ = previous.changed().await;
        }
        let Ok(Some(work)) = this.update(cx, |state, _| {
            Arc::ptr_eq(&store, &state.store)
                .then(|| prepare(state))
                .flatten()
        }) else {
            return;
        };
        let disk = store.clone();
        let result = cx
            .background_executor()
            .spawn(async move { work(disk.as_ref()) })
            .await;
        let _ = this.update(cx, |state, cx| {
            if Arc::ptr_eq(&store, &state.store) {
                match result {
                    Ok(value) => apply(state, value, cx),
                    Err(error) => cx.emit(CollectionEvent::CrudFailed {
                        title,
                        message: error.to_string(),
                    }),
                }
            }
        });
    })
    .detach();
}

#[cfg(test)]
#[path = "collection_io_tests.rs"]
mod tests;
