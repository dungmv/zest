use super::*;
use gpui_kit::{AppContext, TestAppContext};
use zest_core::storage::FsWorkspaceStore;

#[gpui_kit::test]
fn queued_creates_apply_in_order_before_the_workspace_barrier_releases(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(FsWorkspaceStore::new(directory.path()));
    store.init_workspace("Queue").unwrap();
    let collection =
        cx.new(|_| CollectionState::new(store.clone(), directory.path().into(), "Queue".into()));
    collection.update(cx, |state, cx| {
        state.create_request(None, Protocol::Http, cx, |_, _| {});
        state.create_request(None, Protocol::Grpc, cx, |_, _| {});
        assert!(state.requests().is_empty());
        assert!(state.pending_disk_edits().is_some());
    });
    cx.run_until_parked();
    collection.read_with(cx, |state, _| {
        assert!(state.pending_disk_edits().is_none());
        assert_eq!(state.requests().len(), 2);
        assert_eq!(state.requests()[0].protocol, Protocol::Http);
        assert_eq!(state.requests()[1].protocol, Protocol::Grpc);
        for request in state.requests() {
            assert_eq!(
                store.read_request(&request.id).unwrap().protocol,
                request.protocol
            );
        }
    });
}

#[gpui_kit::test]
fn queued_reorders_use_the_latest_order_for_disk_and_sidebar(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(FsWorkspaceStore::new(directory.path()));
    store.init_workspace("Queue").unwrap();
    let collection =
        cx.new(|_| CollectionState::new(store.clone(), directory.path().into(), "Queue".into()));
    let (first, second, third) = collection.update(cx, |state, _| {
        (
            state.create_request_internal(None, Protocol::Http).id,
            state.create_request_internal(None, Protocol::Http).id,
            state.create_request_internal(None, Protocol::Http).id,
        )
    });
    collection.update(cx, |state, cx| {
        state.move_request(&first, &third, cx);
        state.move_request(&second, &first, cx);
    });
    cx.run_until_parked();
    collection.read_with(cx, |state, _| {
        let ids: Vec<_> = state.requests().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, [&third, &first, &second]);
        let folder = store.load_folder_meta("").unwrap();
        let stems: Vec<_> = ids
            .iter()
            .map(|id| {
                std::path::Path::new(id)
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(folder.order, stems);
        let reloaded: Vec<_> = store
            .scan_tree()
            .unwrap()
            .root_requests
            .into_iter()
            .map(|request| request.rel_path)
            .collect();
        assert_eq!(reloaded, ids);
    });
}
