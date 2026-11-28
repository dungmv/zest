use super::*;
use crate::models::{Auth, HttpMethod, KeyValuePair, Protocol};
use crate::storage::FsWorkspaceStore;

fn setup_test_workspace(name: &str) -> (tempfile::TempDir, FsWorkspaceStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FsWorkspaceStore::new(dir.path());
    store.init_workspace(name).expect("init_workspace");
    (dir, store)
}

fn dummy_request_item(id: &str, url: &str) -> RequestItem {
    let mut item = RequestItem::new("ws", "Req", HttpMethod::Get, url);
    item.id = id.to_string();
    item
}

#[test]
fn test_coordinator_coalesces_pending_writes() {
    let mut coord = PersistenceCoordinator::new();
    let target = PersistTarget::Request("users.yaml".to_string());

    let rev1 = coord.enqueue(
        target.clone(),
        PersistPayload::Request(Box::new(dummy_request_item("users.yaml", "http://a"))),
        Duration::from_millis(500),
    );
    let rev2 = coord.enqueue(
        target.clone(),
        PersistPayload::Request(Box::new(dummy_request_item("users.yaml", "http://b"))),
        Duration::from_millis(500),
    );

    assert_eq!(rev1, 1);
    assert_eq!(rev2, 2);
    assert_eq!(coord.status(&target), TargetStatus::Pending);

    // Beginning in-flight takes the latest coalesced write (revision 2)
    let write = coord.begin_in_flight(&target).expect("write ready");
    assert_eq!(write.revision, 2);
    if let PersistPayload::Request(r) = write.payload {
        assert_eq!(r.url, "http://b");
    } else {
        panic!("Expected request payload");
    }

    assert_eq!(coord.status(&target), TargetStatus::InFlight);
    assert!(coord.is_in_flight(&target));

    // Mark success cleans target
    let is_clean = coord.mark_success(&target, 2);
    assert!(is_clean);
    assert_eq!(coord.status(&target), TargetStatus::Clean);
}

#[test]
fn test_coordinator_interleaved_write_race() {
    let mut coord = PersistenceCoordinator::new();
    let target = PersistTarget::Request("race.yaml".to_string());

    // First write begins
    coord.enqueue(
        target.clone(),
        PersistPayload::Request(Box::new(dummy_request_item("race.yaml", "http://v1"))),
        Duration::from_millis(500),
    );
    let write1 = coord.begin_in_flight(&target).unwrap();
    assert_eq!(write1.revision, 1);

    // Edit arrives while write1 is in-flight
    coord.enqueue(
        target.clone(),
        PersistPayload::Request(Box::new(dummy_request_item("race.yaml", "http://v2"))),
        Duration::from_millis(500),
    );

    // In-flight guard prevents starting a second concurrent write
    assert!(coord.is_in_flight(&target));
    assert!(coord.begin_in_flight(&target).is_none());

    // write1 finishes
    let is_clean = coord.mark_success(&target, 1);
    assert!(!is_clean, "Target must remain dirty because v2 is pending");
    assert_eq!(coord.status(&target), TargetStatus::Pending);

    // Now write2 can begin
    let write2 = coord.begin_in_flight(&target).unwrap();
    assert_eq!(write2.revision, 2);
    let is_clean2 = coord.mark_success(&target, 2);
    assert!(is_clean2);
    assert_eq!(coord.status(&target), TargetStatus::Clean);
}

#[test]
fn test_coordinator_failure_retains_dirty_and_retries() {
    let mut coord = PersistenceCoordinator::new();
    let target = PersistTarget::Environment("prod".to_string());
    let env = Environment::new("ws", "prod");

    coord.enqueue(
        target.clone(),
        PersistPayload::Environment {
            name: "prod".to_string(),
            env: Box::new(env.clone()),
        },
        Duration::from_millis(500),
    );

    let write = coord.begin_in_flight(&target).unwrap();
    coord.mark_failure(
        &target,
        write.revision,
        write.payload,
        "Disk full".to_string(),
        Duration::from_millis(500),
    );

    assert_eq!(coord.status(&target), TargetStatus::Failed);
    assert_eq!(coord.last_error(&target), Some("Disk full"));
    assert!(coord.is_dirty(&target));

    // Retry succeeds
    let retry_write = coord.begin_in_flight(&target).expect("restored for retry");
    assert_eq!(retry_write.revision, 1);
    coord.mark_success(&target, 1);

    assert_eq!(coord.status(&target), TargetStatus::Clean);
    assert!(coord.last_error(&target).is_none());
}

#[test]
fn test_coordinator_remap_and_cancel() {
    let mut coord = PersistenceCoordinator::new();
    let old_t = PersistTarget::Request("old.yaml".to_string());
    let new_t = PersistTarget::Request("new.yaml".to_string());

    coord.enqueue(
        old_t.clone(),
        PersistPayload::Request(Box::new(dummy_request_item("old.yaml", "http://x"))),
        Duration::from_millis(500),
    );

    coord.remap_target(&old_t, new_t.clone());
    assert_eq!(coord.status(&old_t), TargetStatus::Clean);
    assert_eq!(coord.status(&new_t), TargetStatus::Pending);

    coord.cancel_target(&new_t);
    assert_eq!(coord.status(&new_t), TargetStatus::Clean);
}

#[test]
fn test_coordinator_workspace_generation_switch() {
    let mut coord = PersistenceCoordinator::new();
    let target = PersistTarget::Request("req.yaml".to_string());
    coord.enqueue(
        target.clone(),
        PersistPayload::Request(Box::new(dummy_request_item("req.yaml", "http://x"))),
        Duration::from_millis(500),
    );

    let gen1 = coord.workspace_generation();
    let gen2 = coord.bump_workspace_generation();
    assert_eq!(gen2, gen1 + 1);
    assert_eq!(coord.status(&target), TargetStatus::Clean);
}

#[test]
fn test_coordinator_independent_targets_do_not_interfere() {
    let mut coord = PersistenceCoordinator::new();
    let req_target = PersistTarget::Request("req1.yaml".to_string());
    let env_target = PersistTarget::Environment("staging".to_string());

    let mut req = RequestItem::new("ws", "Req1", HttpMethod::Get, "http://api.com/1");
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
    let mut req = RequestItem::new("ws", "Login", HttpMethod::Post, "https://api.com/login");
    req.id = store
        .create_request(Some("auth"), "Login", HttpMethod::Post, "", Protocol::Http)
        .unwrap()
        .1;
    req.headers.push(KeyValuePair {
        key: "X-App".to_string(),
        value: "Zest".to_string(),
        description: None,
        enabled: true,
    });
    let req_payload = PersistPayload::Request(Box::new(req.clone()));
    req_payload.write_to_store(&store).unwrap();

    let loaded_req = store.read_request("auth/login.yaml").unwrap();
    assert_eq!(loaded_req.name, "Login");
    assert_eq!(loaded_req.url, "https://api.com/login");
    assert_eq!(loaded_req.headers.len(), 1);

    // 2. Environment & Secrets Group
    let mut env = Environment::new("ws", "dev");
    env.variables.push(crate::models::EnvironmentVariable {
        key: "host".to_string(),
        initial_value: "localhost".to_string(),
        current_value: "localhost".to_string(),
        secret: false,
    });
    env.variables.push(crate::models::EnvironmentVariable {
        key: "api_key".to_string(),
        initial_value: "sec_init_123".to_string(),
        current_value: "sec_cur_456".to_string(),
        secret: true,
    });
    let env_payload = PersistPayload::Environment {
        name: "dev".to_string(),
        env: Box::new(env),
    };
    env_payload.write_to_store(&store).unwrap();

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
    globals.variables.push(crate::models::EnvironmentVariable {
        key: "global_host".to_string(),
        initial_value: "example.org".to_string(),
        current_value: "example.org".to_string(),
        secret: false,
    });
    let globals_payload = PersistPayload::Globals(Box::new(globals));
    globals_payload.write_to_store(&store).unwrap();

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
    meta_payload.write_to_store(&store).unwrap();

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
    folder_payload.write_to_store(&store).unwrap();

    let loaded_folder = store.load_folder_meta("auth").unwrap();
    assert_eq!(loaded_folder.name, "Auth Group");
    assert_eq!(loaded_folder.order, vec!["auth/login.yaml".to_string()]);

    // 6. Pins
    let pins = vec!["auth/login.yaml".to_string()];
    let pins_payload = PersistPayload::Pins(pins.clone());
    pins_payload.write_to_store(&store).unwrap();

    let loaded_pins = store.load_pins().unwrap();
    assert_eq!(loaded_pins, pins);

    // 7. Local State
    let local_state = LocalState {
        active_request: Some("auth/login.yaml".to_string()),
        active_environment: Some("dev".to_string()),
        ..Default::default()
    };
    let state_payload = PersistPayload::LocalState(Box::new(local_state));
    state_payload.write_to_store(&store).unwrap();

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
