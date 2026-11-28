use super::*;
use gpui_kit::{AppContext as _, TestAppContext};
use tempfile::tempdir;
use zest_core::models::EnvironmentVariable;
use zest_core::storage::models::{EnvVarFile, EnvironmentFile};
use zest_core::storage::FsWorkspaceStore;

#[test]
fn test_reload_environments_sorted_alphabetically() {
    let temp = tempdir().expect("create temp dir");
    let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(temp.path()));
    let _ = store.init_workspace("TestWS");

    let _ = store.save_environment("production", &EnvironmentFile::default());
    let _ = store.save_environment("alpha", &EnvironmentFile::default());
    let _ = store.save_environment("Development", &EnvironmentFile::default());
    let _ = store.save_environment("beta", &EnvironmentFile::default());

    let state = EnvironmentState::new(store, "TestWS".to_string(), None);
    let names: Vec<&str> = state.environments.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["alpha", "beta", "Development", "local", "production"]
    );
}

#[gpui_kit::test]
fn test_apply_disk_changes_preserves_dirty_env_and_globals(cx: &mut TestAppContext) {
    let temp = tempdir().expect("create temp dir");
    let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(temp.path()));
    store.init_workspace("TestWS").unwrap();

    let mut disk_dev = EnvironmentFile::default();
    disk_dev.variables.push(EnvVarFile {
        name: "HOST".to_string(),
        value: Some("disk-dev".to_string()),
        secret: false,
        enabled: true,
    });
    store.save_environment("dev", &disk_dev).unwrap();

    let mut disk_staging = EnvironmentFile::default();
    disk_staging.variables.push(EnvVarFile {
        name: "HOST".to_string(),
        value: Some("disk-staging-v1".to_string()),
        secret: false,
        enabled: true,
    });
    store.save_environment("staging", &disk_staging).unwrap();

    let env_entity = cx.new(|_| EnvironmentState::new(store.clone(), "TestWS".to_string(), None));

    // Simulate in-progress user typing in `dev` and `globals`
    env_entity.update(cx, |env, cx| {
        let dev_idx = env
            .environments
            .iter()
            .position(|e| e.name == "dev")
            .unwrap();
        env.environments[dev_idx].variables[0].current_value = "user-typing-abxy".to_string();
        env.global_environment.variables.push(EnvironmentVariable {
            key: "G_KEY".to_string(),
            initial_value: "g_typing".to_string(),
            current_value: "g_typing".to_string(),
            secret: false,
        });

        // Meanwhile disk changes for both `dev` (dirty) and `staging` (clean)
        let mut ext_staging = EnvironmentFile::default();
        ext_staging.variables.push(EnvVarFile {
            name: "HOST".to_string(),
            value: Some("disk-staging-v2".to_string()),
            secret: false,
            enabled: true,
        });
        store.save_environment("staging", &ext_staging).unwrap();

        let changed: HashSet<String> = ["dev".to_string(), "staging".to_string()]
            .into_iter()
            .collect();
        let dirty: HashSet<String> = ["dev".to_string(), "globals".to_string()]
            .into_iter()
            .collect();

        let snapshot = EnvironmentSnapshot::load(env.store.as_ref(), &env.workspace_name);
        let conflicts = env.apply_disk_changes(snapshot, &changed, &dirty, cx);
        assert_eq!(conflicts, vec!["dev".to_string()]);

        // Dirty `dev` and `globals` must retain user's in-memory text
        let dev = env.environments.iter().find(|e| e.name == "dev").unwrap();
        assert_eq!(dev.variables[0].current_value, "user-typing-abxy");
        assert_eq!(
            env.global_environment.variables[0].current_value,
            "g_typing"
        );

        // Clean `staging` must update to the new disk value
        let staging = env
            .environments
            .iter()
            .find(|e| e.name == "staging")
            .unwrap();
        assert_eq!(staging.variables[0].initial_value, "disk-staging-v2");

        // Reloading `dev` explicitly from disk discards the dirty memory copy
        env.reload_environment_from_disk("dev", cx);
        let dev_reloaded = env.environments.iter().find(|e| e.name == "dev").unwrap();
        assert_eq!(dev_reloaded.variables[0].initial_value, "disk-dev");
        assert_eq!(dev_reloaded.variables[0].current_value, "");
    });
}

#[gpui_kit::test]
fn test_variable_resolution_and_addition(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(dir.path()));
    store.init_workspace("TestWS2").unwrap();

    let env_entity = cx.new(|_| EnvironmentState::new(store.clone(), "TestWS2".to_string(), None));

    let env_id = env_entity.update(cx, |env, cx| {
        env.create_environment(Some("Production".to_string()), cx)
    });
    cx.run_until_parked();
    env_entity.update(cx, |env, cx| {
        env.set_active_environment(Some(env_id.clone()), cx);

        // 1. Variable not found anywhere -> falls back to active env
        assert_eq!(env.resolve_variable_env_id("token"), env_id);
        assert!(!env.has_variable(&env_id, "token"));

        // 2. Add variable with key
        env.add_variable_with_key(&env_id, "token".to_string(), cx);
        assert!(env.has_variable(&env_id, "token"));
        assert_eq!(env.resolve_variable_env_id("token"), env_id);

        // 3. Global variable resolution
        env.add_variable_with_key("globals", "global_key".to_string(), cx);
        assert!(env.has_variable("globals", "global_key"));
        assert_eq!(env.resolve_variable_env_id("global_key"), "globals");
    });
}

#[test]
fn test_flush_environments() {
    let temp_dir =
        std::env::temp_dir().join(format!("zest_test_flush_env_{}", uuid::Uuid::new_v4()));
    let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(&temp_dir));
    let _ = store.init_workspace("Test WS");

    let mut env_state = EnvironmentState::new(store.clone(), "Test WS".to_string(), None);

    let mut test_env = Environment::new("ws-test", "Staging");
    test_env.variables.push(EnvironmentVariable {
        key: "API_URL".to_string(),
        initial_value: "https://staging.api.com".to_string(),
        current_value: "http://localhost:3000".to_string(),
        secret: false,
    });

    env_state.environments.push(test_env);
    // Direct save environments to disk
    for env in &env_state.environments {
        let (file, secrets) = EnvironmentFile::from_environment(env);
        let _ = env_state.store.save_environment(&env.name, &file);
        let _ = env_state.store.save_secrets(&env.name, &secrets);
    }

    // Verify files exist in storage
    let loaded = store.list_environments().unwrap();
    assert!(loaded.contains(&"Staging".to_string()));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
