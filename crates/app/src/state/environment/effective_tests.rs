use super::*;
use zest_core::models::EnvironmentVariable;
use zest_core::storage::FsWorkspaceStore;

#[test]
fn test_effective_environment_override() {
    let mut global = Environment::new("default", "Globals");
    global.variables.push(EnvironmentVariable {
        key: "HOST".to_string(),
        initial_value: "global.example.com".to_string(),
        current_value: "".to_string(),
        secret: false,
    });
    global.variables.push(EnvironmentVariable {
        key: "GLOBAL_TOKEN".to_string(),
        initial_value: "token_123".to_string(),
        current_value: "".to_string(),
        secret: false,
    });
    global.variables.push(EnvironmentVariable {
        key: "ONLY_GLOBAL".to_string(),
        initial_value: "yes".to_string(),
        current_value: "".to_string(),
        secret: false,
    });

    let mut active = Environment::new("active_env_id", "Production");
    active.variables.push(EnvironmentVariable {
        key: "HOST".to_string(),
        initial_value: "prod.example.com".to_string(),
        current_value: "".to_string(),
        secret: false,
    });
    active.variables.push(EnvironmentVariable {
        key: "PORT".to_string(),
        initial_value: "443".to_string(),
        current_value: "".to_string(),
        secret: false,
    });

    let temp_dir =
        std::env::temp_dir().join(format!("test_ws_eff_{}", uuid::Uuid::new_v4().simple()));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    let _ = store.init_workspace("Test WS");

    let mut env_state =
        EnvironmentState::new(store, "Test WS".to_string(), Some(active.id.clone()));
    env_state.environments = vec![active.clone()];
    env_state.global_environment = global;
    env_state.active_environment_id = Some(active.id.clone());

    let effective = env_state.effective_environment();
    let host_var = effective
        .variables
        .iter()
        .find(|v| v.key == "HOST")
        .unwrap();
    assert_eq!(host_var.resolved_value(), "prod.example.com");

    let port_var = effective
        .variables
        .iter()
        .find(|v| v.key == "PORT")
        .unwrap();
    assert_eq!(port_var.resolved_value(), "443");

    let token_var = effective
        .variables
        .iter()
        .find(|v| v.key == "GLOBAL_TOKEN")
        .unwrap();
    assert_eq!(token_var.resolved_value(), "token_123");

    assert!(effective.variables.iter().any(|v| v.key == "ONLY_GLOBAL"));
    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Regression guard for duplicated env-file rows (a workspace YAML can
/// carry the same key twice): the first occurrence must win everywhere so
/// the hover-tooltip value always equals the value sent on the wire.
#[test]
fn test_effective_environment_duplicate_keys_first_wins() {
    let mut global = Environment::new("default", "Globals");
    global.variables.push(EnvironmentVariable {
        key: "TOKEN".to_string(),
        initial_value: "global-first".to_string(),
        current_value: String::new(),
        secret: false,
    });
    global.variables.push(EnvironmentVariable {
        key: "TOKEN".to_string(),
        initial_value: "global-stale".to_string(),
        current_value: String::new(),
        secret: false,
    });

    let mut active = Environment::new("active_env_id", "prod");
    active.variables.push(EnvironmentVariable {
        key: "TOKEN".to_string(),
        initial_value: String::new(),
        current_value: "active-new".to_string(),
        secret: false,
    });
    active.variables.push(EnvironmentVariable {
        key: "TOKEN".to_string(),
        initial_value: "active-stale-init".to_string(),
        current_value: "active-stale".to_string(),
        secret: true,
    });
    active.variables.push(EnvironmentVariable {
        key: "PORT".to_string(),
        initial_value: "8080".to_string(),
        current_value: String::new(),
        secret: false,
    });

    let temp_dir =
        std::env::temp_dir().join(format!("test_ws_dup_{}", uuid::Uuid::new_v4().simple()));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    let _ = store.init_workspace("Dup WS");

    let active_id = active.id.clone();
    let mut env_state = EnvironmentState::new(store, "Dup WS".to_string(), Some(active_id.clone()));
    env_state.environments = vec![active];
    env_state.global_environment = global;
    env_state.active_environment_id = Some(active_id);

    let effective = env_state.effective_environment();

    // The effective merge carries exactly one row per key and the first
    // occurrence within the active layer wins.
    let token_count = effective
        .variables
        .iter()
        .filter(|v| v.key == "TOKEN")
        .count();
    assert_eq!(token_count, 1);
    let token = effective
        .variables
        .iter()
        .find(|v| v.key == "TOKEN")
        .unwrap();
    assert_eq!(token.resolved_value(), "active-new");

    // Interpolation over the effective env must agree with the merge.
    let res = zest_core::env::interpolate("Bearer {{TOKEN}}", Some(&effective));
    assert_eq!(res.text, "Bearer active-new");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_effective_environment_caching_and_invalidation() {
    let temp_dir =
        std::env::temp_dir().join(format!("test_env_cache_{}", uuid::Uuid::new_v4().simple()));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    let _ = store.init_workspace("Test Cache");

    let mut env_state = EnvironmentState::new(store.clone(), "Test Cache".to_string(), None);
    env_state
        .global_environment
        .variables
        .push(EnvironmentVariable {
            key: "API_URL".to_string(),
            initial_value: "https://api.dev.com".to_string(),
            current_value: "".to_string(),
            secret: false,
        });

    // 1. Initial call populates cache
    let arc1 = env_state.effective_environment();
    assert_eq!(arc1.variables.len(), 1);
    assert_eq!(arc1.variables[0].key, "API_URL");

    // 2. Subsequent call returns the exact same Arc (O(1) pointer reuse, no allocation/clone)
    let arc2 = env_state.effective_environment();
    assert!(
        Arc::ptr_eq(&arc1, &arc2),
        "Subsequent effective_environment calls must reuse the cached Arc"
    );

    // 3. Mutating global environment invalidates cache
    let mut new_global = env_state.global_environment.clone();
    new_global.variables.push(EnvironmentVariable {
        key: "API_KEY".to_string(),
        initial_value: "secret_123".to_string(),
        current_value: "".to_string(),
        secret: true,
    });
    env_state.save_global_environment_silently(new_global);

    // 4. Next call produces fresh Arc with updated values
    let arc3 = env_state.effective_environment();
    assert!(
        !Arc::ptr_eq(&arc1, &arc3),
        "Cache must be invalidated after modifying global environment"
    );
    assert_eq!(arc3.variables.len(), 2);

    // 5. Explicit invalidate_cache works
    env_state.invalidate_cache();
    let arc4 = env_state.effective_environment();
    assert!(
        !Arc::ptr_eq(&arc3, &arc4),
        "Explicit invalidate_cache must trigger recomputation"
    );
    assert_eq!(arc4.variables.len(), 2);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
