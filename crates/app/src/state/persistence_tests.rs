use super::*;
use gpui::AppContext;
use std::time::Duration;
use zest_core::models::{Auth, Environment, HttpMethod, HttpRequestItem, KeyValuePair};
use zest_core::storage::models::{FolderFile, LocalState, WorkspaceMeta};
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

fn setup_test_workspace(name: &str) -> (tempfile::TempDir, std::sync::Arc<FsWorkspaceStore>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = std::sync::Arc::new(FsWorkspaceStore::new(dir.path()));
    store.init_workspace(name).expect("init_workspace");
    (dir, store)
}

#[test]
fn test_coordinator_independent_targets_do_not_interfere() {
    let mut coord = PersistenceCoordinator::new();
    let req_target = PersistTarget::Request("req1.yaml".to_string());
    let env_target = PersistTarget::Environment("staging".to_string());

    let mut req = HttpRequestItem::new("ws", "Req1", HttpMethod::Get, "http://api.com/1");
    req.id = "req1.yaml".to_string();

    let env = Environment::new("ws", "staging");

    coord.enqueue(
        req_target.clone(),
        PersistPayload::Request(Box::new(req)),
        Duration::from_millis(500),
    );
    coord.enqueue(
        env_target.clone(),
        PersistPayload::Environment {
            name: "staging".to_string(),
            env: Box::new(env),
        },
        Duration::from_millis(500),
    );

    assert_eq!(coord.status(&req_target), TargetStatus::Pending);
    assert_eq!(coord.status(&env_target), TargetStatus::Pending);

    // Drain request target only
    let req_write = coord.begin_in_flight(&req_target).unwrap();
    assert_eq!(req_write.revision, 1);
    assert_eq!(coord.status(&req_target), TargetStatus::InFlight);
    assert_eq!(coord.status(&env_target), TargetStatus::Pending);

    coord.mark_success(&req_target, 1);
    assert_eq!(coord.status(&req_target), TargetStatus::Clean);
    assert_eq!(coord.status(&env_target), TargetStatus::Pending);

    let env_write = coord.begin_in_flight(&env_target).unwrap();
    coord.mark_success(&env_target, env_write.revision);
    assert_eq!(coord.status(&env_target), TargetStatus::Clean);
}

#[test]
fn test_persist_payload_atomic_writes_to_store() {
    let (_dir, store) = setup_test_workspace("AtomicStore");

    // 1. Request
    let mut req = HttpRequestItem::new("ws", "Login", HttpMethod::Post, "https://api.com/login");
    req.id = "auth/login.yaml".to_string();
    req.headers.push(KeyValuePair {
        key: "X-App".to_string(),
        value: "Zest".to_string(),
        description: None,
        enabled: true,
    });
    let req_payload = PersistPayload::Request(Box::new(req.clone()));
    req_payload.write_to_store(store.as_ref()).unwrap();

    let loaded_req = store.read_request("auth/login.yaml").unwrap();
    assert_eq!(loaded_req.name, "Login");
    assert_eq!(loaded_req.url, "https://api.com/login");
    assert_eq!(loaded_req.headers.len(), 1);

    // 2. Environment & Secrets Group
    let mut env = Environment::new("ws", "dev");
    env.variables.push(zest_core::models::EnvironmentVariable {
        key: "host".to_string(),
        initial_value: "localhost".to_string(),
        current_value: "localhost".to_string(),
        secret: false,
    });
    env.variables.push(zest_core::models::EnvironmentVariable {
        key: "api_key".to_string(),
        initial_value: "sec_init_123".to_string(),
        current_value: "sec_cur_456".to_string(),
        secret: true,
    });
    let env_payload = PersistPayload::Environment {
        name: "dev".to_string(),
        env: Box::new(env),
    };
    env_payload.write_to_store(store.as_ref()).unwrap();

    let loaded_env = store.load_environment("dev").unwrap();
    let sec_var = loaded_env
        .variables
        .iter()
        .find(|v| v.name == "api_key")
        .unwrap();
    assert_eq!(sec_var.value.as_deref(), Some("sec_cur_456"));
    assert!(sec_var.secret);

    // 3. Globals
    let mut globals = Environment::new("ws", "Globals");
    globals
        .variables
        .push(zest_core::models::EnvironmentVariable {
            key: "global_host".to_string(),
            initial_value: "example.org".to_string(),
            current_value: "example.org".to_string(),
            secret: false,
        });
    let globals_payload = PersistPayload::Globals(Box::new(globals));
    globals_payload.write_to_store(store.as_ref()).unwrap();

    let loaded_globals = store.load_global_environment().unwrap();
    assert_eq!(loaded_globals.variables[0].name, "global_host");
    assert_eq!(
        loaded_globals.variables[0].value.as_deref(),
        Some("example.org")
    );

    // 4. Workspace Meta
    let meta = WorkspaceMeta {
        version: 1,
        name: "Renamed WS".to_string(),
        headers: vec![KeyValuePair {
            key: "Auth".to_string(),
            value: "Bearer xyz".to_string(),
            description: None,
            enabled: true,
        }],
        auth: Auth::None,
    };
    let meta_payload = PersistPayload::WorkspaceMeta(Box::new(meta));
    meta_payload.write_to_store(store.as_ref()).unwrap();

    let loaded_meta = store.load_workspace_meta().unwrap();
    assert_eq!(loaded_meta.name, "Renamed WS");
    assert_eq!(loaded_meta.headers.len(), 1);

    // 5. Folder Meta
    let folder_meta = FolderFile {
        name: "Auth Group".to_string(),
        order: vec!["auth/login.yaml".to_string()],
        headers: vec![],
        auth: Auth::Inherit,
    };
    let folder_payload = PersistPayload::FolderMeta {
        folder_rel: "auth".to_string(),
        meta: Box::new(folder_meta),
    };
    folder_payload.write_to_store(store.as_ref()).unwrap();

    let loaded_folder = store.load_folder_meta("auth").unwrap();
    assert_eq!(loaded_folder.name, "Auth Group");
    assert_eq!(loaded_folder.order, vec!["auth/login.yaml".to_string()]);

    // 6. Pins
    let pins = vec!["auth/login.yaml".to_string()];
    let pins_payload = PersistPayload::Pins(pins.clone());
    pins_payload.write_to_store(store.as_ref()).unwrap();

    let loaded_pins = store.load_pins().unwrap();
    assert_eq!(loaded_pins, pins);

    // 7. Local State
    let local_state = LocalState {
        active_request: Some("auth/login.yaml".to_string()),
        active_environment: Some("dev".to_string()),
        ..Default::default()
    };
    let state_payload = PersistPayload::LocalState(Box::new(local_state));
    state_payload.write_to_store(store.as_ref()).unwrap();

    let loaded_state = store.load_state().unwrap();
    assert_eq!(
        loaded_state.active_request.as_deref(),
        Some("auth/login.yaml")
    );
    assert_eq!(loaded_state.active_environment.as_deref(), Some("dev"));
}

#[test]
fn test_coordinator_barrier_take_all_pending() {
    let mut coord = PersistenceCoordinator::new();
    let target1 = PersistTarget::Pins;
    let target2 = PersistTarget::WorkspaceMeta;

    coord.enqueue(
        target1.clone(),
        PersistPayload::Pins(vec!["req_a".to_string()]),
        Duration::from_millis(500),
    );
    coord.enqueue(
        target2.clone(),
        PersistPayload::WorkspaceMeta(Box::default()),
        Duration::from_millis(500),
    );

    let drained = coord.take_all_pending();
    assert_eq!(drained.len(), 2);
    assert_eq!(coord.status(&target1), TargetStatus::InFlight);
    assert_eq!(coord.status(&target2), TargetStatus::InFlight);

    for (t, p) in drained {
        coord.mark_success(&t, p.revision);
    }
    assert_eq!(coord.status(&target1), TargetStatus::Clean);
    assert_eq!(coord.status(&target2), TargetStatus::Clean);
}

#[gpui::test]
fn test_environment_mutations_emit_events_and_update_state(cx: &mut gpui::TestAppContext) {
    use std::cell::RefCell;
    use std::rc::Rc;

    let (_dir, store) = setup_test_workspace("EnvMutations");
    let env_state = cx.new(|_| EnvironmentState::new(store.clone(), "TestWS".to_string(), None));

    let recorded = Rc::new(RefCell::new(Vec::new()));
    let recorded_cb = recorded.clone();

    struct TestListener;
    let _listener = cx.new(|cx| {
        cx.subscribe(
            &env_state,
            move |_this, _env, ev: &EnvironmentEvent, _cx| {
                recorded_cb.borrow_mut().push(ev.clone());
            },
        )
        .detach();
        TestListener
    });

    // 1. Create and mutate an environment
    env_state.update(cx, |env, cx| {
        let env_name = env.create_environment(Some("testing".to_string()), cx);
        assert_eq!(env_name, "testing");
        env.add_variable("testing", cx);
        env.update_variable_key("testing", 0, "api_base".to_string(), cx);
        env.update_variable_initial_value("testing", 0, "http://init.com".to_string(), cx);
        env.update_variable_current_value("testing", 0, "http://curr.com".to_string(), cx);
        env.toggle_variable_secret("testing", 0, cx);
    });

    // Verify in-memory state updated synchronously
    env_state.read_with(cx, |env, _| {
        let test_env = env
            .environments
            .iter()
            .find(|e| e.name == "testing")
            .unwrap();
        assert_eq!(test_env.variables.len(), 1);
        assert_eq!(test_env.variables[0].key, "api_base");
        assert_eq!(test_env.variables[0].initial_value, "http://init.com");
        assert_eq!(test_env.variables[0].current_value, "http://curr.com");
        assert!(test_env.variables[0].secret);
    });

    // 2. Mutate Globals
    env_state.update(cx, |env, cx| {
        env.add_variable("globals", cx);
        env.update_variable_key("globals", 0, "token".to_string(), cx);
        env.update_variable_current_value("globals", 0, "xyz123".to_string(), cx);
    });

    env_state.read_with(cx, |env, _| {
        assert_eq!(env.global_environment.variables.len(), 1);
        assert_eq!(env.global_environment.variables[0].key, "token");
        assert_eq!(env.global_environment.variables[0].current_value, "xyz123");
    });

    // Verify emitted events
    let events = recorded.borrow();
    assert!(events
        .iter()
        .any(|e| matches!(e, EnvironmentEvent::EnvironmentModified(n) if n == "testing")));
    assert!(events
        .iter()
        .any(|e| matches!(e, EnvironmentEvent::GlobalsModified)));
}

#[gpui::test]
fn test_persistence_state_write_behind_flush(cx: &mut gpui::TestAppContext) {
    let (dir, store) = setup_test_workspace("WriteBehindFlush");
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));

    // Schedule saves through persistence state
    per_state.update(cx, |per, cx| {
        let mut env = Environment::new("ws", "prod");
        env.variables.push(zest_core::models::EnvironmentVariable {
            key: "db_pass".to_string(),
            initial_value: "init_pwd".to_string(),
            current_value: "super_secret".to_string(),
            secret: true,
        });
        per.schedule_save_environment("prod".to_string(), env, cx);

        let mut globals = Environment::new("ws", "Globals");
        globals
            .variables
            .push(zest_core::models::EnvironmentVariable {
                key: "shared_var".to_string(),
                initial_value: "global_val".to_string(),
                current_value: "global_val".to_string(),
                secret: false,
            });
        per.schedule_save_globals(globals, cx);

        // Target should be pending/in-flight in coordinator
        assert!(per.is_target_dirty(&PersistTarget::Environment("prod".to_string())));
        assert!(per.is_target_dirty(&PersistTarget::Globals));

        // Immediate flush
        per.flush_all(cx);
    });

    cx.run_until_parked();

    // Verify written to store
    let loaded_env = store.load_environment("prod").expect("load_environment");
    let loaded_sec = store.load_secrets("prod").expect("load_secrets");
    assert_eq!(
        loaded_sec.get("db_pass").map(|s| s.as_str()),
        Some("super_secret")
    );
    let var = loaded_env
        .variables
        .iter()
        .find(|v| v.name == "db_pass")
        .unwrap();
    assert_eq!(var.value.as_deref(), Some("super_secret"));

    let loaded_globals = store
        .load_global_environment()
        .expect("load_global_environment");
    assert_eq!(loaded_globals.variables[0].name, "shared_var");
    assert_eq!(
        loaded_globals.variables[0].value.as_deref(),
        Some("global_val")
    );
}

#[gpui::test]
fn test_request_persistence_state_and_remap(cx: &mut gpui::TestAppContext) {
    let (dir, store) = setup_test_workspace("RequestRemap");
    let ws_state =
        cx.new(|_| WorkspaceDataState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));

    // 1. Create request in workspace
    let req_id = ws_state.update(cx, |ws, cx| {
        let (_slug, rel_path) = ws
            .store
            .create_request(None, "Get Users", HttpMethod::Get, "https://api.com/users")
            .unwrap();
        let item =
            HttpRequestItem::new("ws", "Get Users", HttpMethod::Get, "https://api.com/users");
        let mut item = item;
        item.id = rel_path.clone();
        ws.requests.push(item);
        cx.notify();
        rel_path
    });

    // 2. Modify request and schedule save
    ws_state.update(cx, |ws, _| {
        if let Some(r) = ws.find_request_mut(&req_id) {
            r.url = "https://api.com/v2/users".to_string();
        }
    });

    let ws_clone = ws_state.clone();
    let r_id_clone = req_id.clone();
    per_state.update(cx, |per, cx| {
        per.schedule_save_active_request(r_id_clone, ws_clone, cx);
        assert!(per.is_dirty(&req_id));
        per.flush_all(cx);
    });

    cx.run_until_parked();

    // Verify written to disk
    let loaded = store.read_request(&req_id).unwrap();
    assert_eq!(loaded.url, "https://api.com/v2/users");

    // 3. Remap request ID (simulating rename/move)
    let new_req_id = "users_v2.yaml".to_string();
    per_state.update(cx, |per, cx| {
        per.mark_dirty(req_id.clone(), cx);
        per.remap_request_ids(&[(req_id.clone(), new_req_id.clone())], cx);
        assert!(!per.is_dirty(&req_id));
        assert!(per.is_dirty(&new_req_id));
    });

    // 4. Cancel target (simulating delete)
    per_state.update(cx, |per, cx| {
        per.cancel_request(&new_req_id, cx);
        assert!(!per.is_dirty(&new_req_id));
    });
}

#[gpui::test]
fn test_fs_batch_recreated_request_not_dropped(cx: &mut gpui::TestAppContext) {
    use zest_core::storage::FsEvent;

    let (dir, store) = setup_test_workspace("WatcherRecreated");
    let ws_state =
        cx.new(|_| WorkspaceDataState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let env_state =
        cx.new(|_| EnvironmentState::new(store.clone(), "WatcherRecreated".to_string(), None));
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));

    // Create a request file on disk and load it into workspace state
    let req_id = store
        .create_request(None, "Get Users", HttpMethod::Get, "https://api.com/users")
        .unwrap()
        .1;
    ws_state.update(cx, |ws, _| ws.reload_tree());
    let req_file_path = dir.path().join("collections").join(&req_id);
    assert!(req_file_path.is_file());
    let in_memory = |cx: &mut gpui::TestAppContext| {
        ws_state.read_with(cx, |ws, _| ws.find_request(&req_id).is_some())
    };
    assert!(in_memory(cx));

    let git_state = cx.new(|_| GitState::new(dir.path().to_path_buf()));
    let dispatch = |events: Vec<FsEvent>, cx: &mut gpui::TestAppContext| {
        let ws = ws_state.clone();
        let env = env_state.clone();
        let git = git_state.clone();
        per_state.update(cx, |per, cx| {
            per.handle_fs_events_batch(events, ws, env, git, cx);
        });
    };

    // 1. Atomic replace in one batch (`git restore`): Remove + Create. The
    //    file still exists, so the request must never be dropped.
    dispatch(
        vec![
            FsEvent::RequestDeleted(req_id.clone()),
            FsEvent::RequestChanged(req_id.clone()),
        ],
        cx,
    );
    assert!(in_memory(cx), "replaced-in-place request was dropped");

    // 2. Split batches: real deletion first...
    let yaml = std::fs::read_to_string(&req_file_path).unwrap();
    std::fs::remove_file(&req_file_path).unwrap();
    dispatch(vec![FsEvent::RequestDeleted(req_id.clone())], cx);
    assert!(!in_memory(cx), "truly deleted request was kept");

    // 3. ...then the file reappears on disk (late `git restore` finish): the
    //    follow-up change event must re-add it via a tree rescan.
    std::fs::write(&req_file_path, yaml).unwrap();
    dispatch(vec![FsEvent::RequestChanged(req_id.clone())], cx);
    assert!(in_memory(cx), "recreated request was not re-added");
}

#[gpui::test]
fn test_external_change_reconciles_clean_vs_dirty(cx: &mut gpui::TestAppContext) {
    use zest_core::storage::FsEvent;

    let (dir, store) = setup_test_workspace("test_reconcile");
    let ws_state =
        cx.new(|_| WorkspaceDataState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));
    let env_state =
        cx.new(|_| EnvironmentState::new(store.clone(), "test_reconcile".to_string(), None));
    let git_state = cx.new(|_| GitState::new(dir.path().to_path_buf()));

    let req_id = store
        .create_request(
            None,
            "Original Name",
            HttpMethod::Get,
            "https://api.com/users",
        )
        .unwrap()
        .1;
    ws_state.update(cx, |ws, _| ws.reload_tree());

    // 1. Clean request: external edit on disk is automatically reloaded into memory
    let req_file_path = dir.path().join("collections").join(&req_id);
    let mut modified_yaml = std::fs::read_to_string(&req_file_path).unwrap();
    modified_yaml = modified_yaml.replace("Original Name", "External Name");
    std::fs::write(&req_file_path, &modified_yaml).unwrap();

    let ws = ws_state.clone();
    let env = env_state.clone();
    let git = git_state.clone();
    per_state.update(cx, |per, cx| {
        per.handle_fs_events_batch(
            vec![FsEvent::RequestChanged(req_id.clone())],
            ws,
            env,
            git,
            cx,
        );
    });

    let current_name =
        ws_state.read_with(cx, |ws, _| ws.find_request(&req_id).map(|r| r.name.clone()));
    assert_eq!(current_name.as_deref(), Some("External Name"));
    assert!(!per_state.read_with(cx, |per, _| per.conflicts.contains_key(&req_id)));

    // 2. Dirty request: external edit triggers conflict, keeps in-memory edits intact
    per_state.update(cx, |per, cx| {
        per.mark_dirty(req_id.clone(), cx);
    });
    ws_state.update(cx, |ws, _| {
        if let Some(r) = ws.requests.iter_mut().find(|r| r.id == req_id) {
            r.name = "In-Memory Edits".to_string();
        }
    });

    let mut external_cli_yaml = std::fs::read_to_string(&req_file_path).unwrap();
    external_cli_yaml = external_cli_yaml.replace("External Name", "CLI Branch Edit");
    std::fs::write(&req_file_path, &external_cli_yaml).unwrap();

    let ws2 = ws_state.clone();
    let env2 = env_state.clone();
    let git2 = git_state.clone();
    per_state.update(cx, |per, cx| {
        per.handle_fs_events_batch(
            vec![FsEvent::RequestChanged(req_id.clone())],
            ws2,
            env2,
            git2,
            cx,
        );
    });

    // In-memory edits preserved
    let memory_name =
        ws_state.read_with(cx, |ws, _| ws.find_request(&req_id).map(|r| r.name.clone()));
    assert_eq!(memory_name.as_deref(), Some("In-Memory Edits"));
    // Conflict registered
    assert!(per_state.read_with(cx, |per, _| per.conflicts.contains_key(&req_id)));

    // 3. Discard cancellation: user discards changes
    per_state.update(cx, |per, cx| {
        per.cancel_request(&req_id, cx);
        per.mark_clean(&req_id, cx);
        per.conflicts.remove(&req_id);
    });
    assert!(!per_state.read_with(cx, |per, _| per.is_dirty(&req_id)));
    assert!(!per_state.read_with(cx, |per, _| per.conflicts.contains_key(&req_id)));
}
