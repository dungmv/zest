use super::*;
use gpui_kit::{AppContext, TestAppContext};
use zest_core::models::HttpMethod;
use zest_core::scripting::PostResponseOutput;
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

#[gpui_kit::test]
fn cancelled_attempt_cannot_finish_a_retry_or_publish_to_another_request(cx: &mut TestAppContext) {
    use zest_core::models::HttpResponseRecord;

    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(FsWorkspaceStore::new(directory.path()));
    store.init_workspace("Test").unwrap();
    let (_, sent) = store
        .create_request(None, "Sent", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    let (_, selected) = store
        .create_request(None, "Selected", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    let workspace =
        cx.new(|_| WorkspaceState::new_for_path(directory.path().into(), store.clone()));
    let collection =
        cx.new(|_| CollectionState::new_for_path(directory.path().into(), store.clone()));
    let environment = cx.new(|_| EnvironmentState::new(store.clone(), "Test".into(), None));
    let session = cx.new(|_| RequestSessionState::new());
    let response = cx.new(|_| ResponseState::new());
    let modal = cx.new(|_| ModalState::new());
    let (old_tx, old_rx) = mpsc::channel(4);
    let (new_tx, new_rx) = mpsc::channel(4);

    let retry = cx.update(|cx| {
        let executor = workspace.read(cx).executor.clone();
        let runner = Runner::new(
            session.clone(),
            workspace,
            collection,
            environment,
            response.clone(),
            modal,
            Engines::new(executor),
        );
        let first = session.update(cx, |session, cx| {
            session.set_active_request_id(&sent, cx);
            session.start_sending(Some(sent.clone()), oneshot::channel().0, cx)
        });
        runner.spawn_send_pump(cx, first, Protocol::Http, old_rx);
        let retry = session.update(cx, |session, cx| {
            session.cancel_send();
            session.stop_sending(cx);
            session.start_sending(Some(sent.clone()), oneshot::channel().0, cx)
        });
        runner.spawn_send_pump(cx, retry, Protocol::Http, new_rx);
        session.update(cx, |session, cx| {
            session.set_active_request_id(&selected, cx)
        });
        response.update(cx, |response, cx| {
            let record = Arc::new(HttpResponseRecord {
                request_id: selected.clone(),
                ..Default::default()
            });
            response.set_response_for_request(selected.clone(), record, true, cx);
        });
        retry
    });

    assert!(old_tx.try_send(SendEvent::Cancelled).is_ok());
    drop(old_tx);
    cx.run_until_parked();
    assert!(session.read_with(cx, |session, _| session.is_current_send(retry)));

    let record = Arc::new(HttpResponseRecord {
        request_id: sent.clone(),
        ..Default::default()
    });
    assert!(new_tx
        .try_send(SendEvent::HttpSuccess {
            record,
            post_output: Box::default(),
        })
        .is_ok());
    drop(new_tx);
    cx.run_until_parked();
    assert!(!session.read_with(cx, |session, _| session.is_sending()));
    response.update(cx, |response, _| {
        assert_eq!(
            response.current_response.as_ref().unwrap().request_id,
            selected
        );
        assert!(response.cache.get(&sent).is_some());
    });
}

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
            session.clone(),
            workspace.clone(),
            collection.clone(),
            environment.clone(),
            response.clone(),
            modal,
            Engines::new(executor.clone()),
        );
        let (cancel, _receiver) = oneshot::channel();
        let send = session.update(cx, |session, cx| {
            session.start_sending(Some(id.clone()), cancel, cx)
        });
        runner.spawn_send_pump(cx, send, Protocol::Http, rx);
        response.update(cx, |response, cx| {
            response.set_response_for_request(id.clone(), record.clone(), true, cx)
        });
        workspace.update(cx, |workspace, cx| {
            workspace.activate_workspace(new_dir.path().to_path_buf(), new_store.clone(), cx);
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
