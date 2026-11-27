use super::*;
use gpui_kit::{AppContext, TestAppContext};
use zest_core::models::HttpMethod;
use zest_core::scripting::PostResponseOutput;
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

#[gpui_kit::test]
fn late_http_result_cannot_mutate_a_different_workspace(cx: &mut TestAppContext) {
    let old_dir = tempfile::tempdir().unwrap();
    let new_dir = tempfile::tempdir().unwrap();
    let old_store = Arc::new(FsWorkspaceStore::new(old_dir.path()));
    let new_store = Arc::new(FsWorkspaceStore::new(new_dir.path()));
    old_store.init_workspace("Origin").unwrap();
    new_store.init_workspace("Destination").unwrap();
    let (_, id) = new_store
        .create_request(None, "Same ID", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    old_store
        .create_request(None, "Same ID", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    let workspace =
        cx.new(|_| WorkspaceState::new_for_path(old_dir.path().to_path_buf(), old_store.clone()));
    let collection =
        cx.new(|_| CollectionState::new_for_path(old_dir.path().to_path_buf(), old_store.clone()));
    let environment = cx.new(|_| EnvironmentState::new(old_store.clone(), "Origin".into(), None));
    let response = cx.new(|_| ResponseState::new());
    let session = cx.new(|_| RequestSessionState::new());
    let modal = cx.new(|_| ModalState::new());

    let (tx, rx) = mpsc::channel(8);
    let record = Arc::new(zest_core::models::HttpResponseRecord {
        id: "old-response".into(),
        request_id: id.clone(),
        status_code: 200,
        status_text: "OK".into(),
        duration_ms: 0,
        size_bytes: 2,
        headers: vec![],
        cookies: vec![],
        body: b"ok".to_vec(),
        timing: Default::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    });
    cx.update(|cx| {
        let executor = workspace.read(cx).executor.clone();
        let runner = Runner::new(
            session,
            workspace.clone(),
            collection.clone(),
            environment.clone(),
            response.clone(),
            modal,
            Engines::new(executor.clone()),
        );
        runner.spawn_send_pump(cx, Protocol::Http, rx);
        response.update(cx, |response, cx| {
            response.set_response_for_request(id.clone(), record.clone(), cx)
        });
        workspace.update(cx, |workspace, cx| {
            workspace.activate_workspace(
                new_dir.path().to_path_buf(),
                new_store.clone(),
                "Destination",
                cx,
            );
        });
        assert!(!Arc::ptr_eq(&executor, &workspace.read(cx).executor));
        collection.update(cx, |collection, cx| {
            collection.switch_workspace(
                new_store.clone(),
                new_dir.path().to_path_buf(),
                "Destination".into(),
                cx,
            );
        });
        environment.update(cx, |environment, _| {
            *environment = EnvironmentState::empty(new_store.clone(), "Destination".into());
        });
        response.update(cx, |response, cx| response.reset_workspace(cx));
    });

    let output = PostResponseOutput {
        env_mutations: vec![("late".into(), Some("from-origin".into()))],
        ..Default::default()
    };
    tx.try_send(SendEvent::HttpSuccess {
        record,
        post_output: Box::new(output),
    })
    .unwrap_or_else(|_| panic!("queue must accept the late event"));
    drop(tx);
    cx.run_until_parked();

    assert!(new_store.load_response_cache(&id).unwrap().is_none());
    assert!(environment.read_with(cx, |environment, _| environment
        .global_environment
        .variables
        .is_empty()));
    assert!(
        response.read_with(cx, |response, _| response.current_response.is_none()
            && response.cache.is_empty())
    );
    assert!(collection.read_with(cx, |collection, _| collection
        .find_request(&id)
        .unwrap()
        .last_status_code
        .is_none()));
}
