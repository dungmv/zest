use super::*;
use crate::state::{EnvironmentEvent, EnvironmentState};
use gpui_kit::AppContext;
use std::time::Duration;
use zest_core::models::{Environment, HttpMethod, Protocol, RequestItem};
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

pub(super) fn setup_test_workspace(
    name: &str,
) -> (tempfile::TempDir, std::sync::Arc<FsWorkspaceStore>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = std::sync::Arc::new(FsWorkspaceStore::new(dir.path()));
    store.init_workspace(name).expect("init_workspace");
    (dir, store)
}

#[gpui_kit::test]
fn environment_rename_preserves_pending_and_concurrent_edits(cx: &mut gpui_kit::TestAppContext) {
    let (dir, store) = setup_test_workspace("EnvironmentRename");
    let environment =
        cx.new(|_| EnvironmentState::new(store.clone(), "EnvironmentRename".into(), None));
    let persistence = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));
    let edit = |value: &str, cx: &mut gpui_kit::TestAppContext| {
        let snapshot = environment.update(cx, |env, _| {
            let local = env
                .environments
                .iter_mut()
                .find(|env| env.id == "local")
                .unwrap();
            local.variables = vec![zest_core::models::EnvironmentVariable {
                key: "TOKEN".into(),
                initial_value: value.into(),
                current_value: value.into(),
                secret: true,
            }];
            local.clone()
        });
        persistence.update(cx, |state, cx| {
            state.schedule_save_environment("local".into(), snapshot, cx);
        });
    };
    edit("pending", cx);
    persistence.update(cx, |state, cx| {
        state.queue_environment_operation(environment.clone(), "local", Some("renamed".into()), cx);
    });
    edit("latest", cx);
    cx.run_until_parked();

    assert!(!store.list_environments().unwrap().contains(&"local".into()));
    assert_eq!(store.load_secrets("renamed").unwrap()["TOKEN"], "pending");
    environment.read_with(cx, |env, _| {
        let active = env.active_environment().unwrap();
        assert_eq!(active.id, "renamed");
        assert_eq!(active.variables[0].current_value, "latest");
    });

    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(store.load_secrets("renamed").unwrap()["TOKEN"], "latest");
    assert!(store.load_secrets("local").unwrap().is_empty());
}

#[gpui_kit::test]
fn environment_delete_waits_for_in_flight_save(cx: &mut gpui_kit::TestAppContext) {
    let (dir, store) = setup_test_workspace("EnvironmentDelete");
    let environment =
        cx.new(|_| EnvironmentState::new(store.clone(), "EnvironmentDelete".into(), None));
    let persistence = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));
    let snapshot = environment.read_with(cx, |env, _| env.active_environment().unwrap().clone());
    let target = PersistTarget::Environment("local".into());
    persistence.update(cx, |state, cx| {
        state.schedule_save_environment("local".into(), snapshot.clone(), cx);
        state.flush_target_if_ready(&target, cx);
        assert!(state.writes.coordinator.is_in_flight(&target));
        state.schedule_save_environment("local".into(), snapshot, cx);
        state.queue_environment_operation(environment.clone(), "local", None, cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1000));
    cx.run_until_parked();

    assert!(!store.list_environments().unwrap().contains(&"local".into()));
    assert!(!store.secrets_dir().join("local.yaml").exists());
    assert!(environment.read_with(cx, |env, _| env.environments.is_empty()));
    assert_eq!(
        persistence.read_with(cx, |state, _| state.target_status(&target)),
        TargetStatus::Clean
    );
}

#[gpui_kit::test]
fn test_environment_mutations_emit_events_and_update_state(cx: &mut gpui_kit::TestAppContext) {
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
    });
    cx.run_until_parked();
    env_state.update(cx, |env, cx| {
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

#[gpui_kit::test]
fn test_persistence_state_write_behind_flush(cx: &mut gpui_kit::TestAppContext) {
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

#[gpui_kit::test]
fn test_request_persistence_state_and_remap(cx: &mut gpui_kit::TestAppContext) {
    let (dir, store) = setup_test_workspace("RequestRemap");
    let ws_state =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));

    // 1. Create request in workspace
    let req_id = ws_state.update(cx, |ws, cx| {
        let (_slug, rel_path) = ws
            .store
            .create_request(
                None,
                "Get Users",
                HttpMethod::Get,
                "https://api.com/users",
                Protocol::Http,
            )
            .unwrap();
        let item = RequestItem::new("ws", "Get Users", HttpMethod::Get, "https://api.com/users");
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

#[gpui_kit::test]
fn workspace_flush_waits_for_latest_revision(cx: &mut gpui_kit::TestAppContext) {
    let (dir, store) = setup_test_workspace("Origin");
    let (_, id) = store
        .create_request(None, "Request", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    let collection =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let persistence = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));

    let mut barrier = cx.update(|cx| {
        collection.update(cx, |collection, _| {
            collection.find_request_mut(&id).unwrap().url = "https://example.test/first".into();
        });
        persistence.update(cx, |persistence, cx| {
            persistence.schedule_save_active_request(id.clone(), collection.clone(), cx);
            persistence.flush_target_if_ready(&PersistTarget::Request(id.clone()), cx);
        });
        collection.update(cx, |collection, _| {
            collection.find_request_mut(&id).unwrap().url = "https://example.test/latest".into();
        });
        persistence.update(cx, |persistence, cx| {
            persistence.schedule_save_active_request(id.clone(), collection.clone(), cx);
            persistence.flush_before_switch(cx).unwrap()
        })
    });
    assert_eq!(
        barrier.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    );
    cx.run_until_parked();
    assert_eq!(barrier.try_recv().unwrap(), Ok(()));
    assert_eq!(
        store.read_request(&id).unwrap().url,
        "https://example.test/latest"
    );
}

#[gpui_kit::test]
fn workspace_flush_keeps_failed_save_available_for_retry(cx: &mut gpui_kit::TestAppContext) {
    let (dir, store) = setup_test_workspace("Failure");
    let (_, id) = store
        .create_request(None, "Request", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    let collection =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let persistence = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));
    store.delete_request(&id).unwrap();
    let mut barrier = persistence.update(cx, |persistence, cx| {
        persistence.schedule_save_active_request(id.clone(), collection.clone(), cx);
        persistence.flush_before_switch(cx).unwrap()
    });
    cx.run_until_parked();
    assert!(barrier.try_recv().unwrap().is_err());
    assert!(persistence.read_with(cx, |persistence, _| persistence.is_dirty(&id)));
    assert!(store.read_request(&id).is_err());
}

#[test]
fn test_conflict_detection_and_invalid_yaml_handling() {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_ws_conflict_{}",
        uuid::Uuid::new_v4().simple()
    ));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    store.init_workspace("Conflict WS").unwrap();

    let (_slug, rel_path) = store
        .create_request(
            None,
            "Login",
            HttpMethod::Post,
            "https://api.example.com/login",
            Protocol::Http,
        )
        .unwrap();

    let mut ws_state = CollectionState::new_for_path(temp_dir.clone(), store.clone());
    ws_state.reload_tree();
    assert_eq!(ws_state.requests.len(), 1);

    let mut per_state = PersistenceState::new(store.clone(), temp_dir.clone());

    // 1. External modification while dirty triggers conflict
    per_state.changes.unsaved_requests.insert(rel_path.clone());
    if per_state.changes.unsaved_requests.contains(&rel_path) {
        per_state.changes.requests.insert(
            rel_path.clone(),
            "File was modified externally while you have unsaved changes.".to_string(),
        );
    }
    assert!(per_state.changes.requests.contains_key(&rel_path));

    // Resolve keep mine
    per_state.changes.requests.remove(&rel_path);
    per_state.changes.unsaved_requests.remove(&rel_path);
    assert!(!per_state.changes.requests.contains_key(&rel_path));

    // 2. Invalid YAML: test parse error recording and keeping good version
    let req_file_path = temp_dir.join("collections").join(&rel_path);
    std::fs::write(&req_file_path, "invalid yaml : : :").unwrap();
    match store.read_request(&rel_path) {
        Ok(_) => panic!("Expected parse error"),
        Err(e) => {
            ws_state
                .parse_errors
                .insert(rel_path.clone(), e.to_string());
        }
    }
    assert!(ws_state.parse_errors.contains_key(&rel_path));
    // In-memory version is preserved
    assert_eq!(ws_state.requests.len(), 1);
    assert_eq!(ws_state.requests[0].name, "Login");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
