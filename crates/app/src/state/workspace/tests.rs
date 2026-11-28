use super::*;

#[test]
fn test_workspace_cookie_jar_persistence_and_removal() {
    let temp_dir = std::env::temp_dir().join(format!(
        "zest-test-cookies-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    store.init_workspace("Cookie WS").unwrap();

    let mut ws_state = WorkspaceState::new_for_path(temp_dir.clone(), store.clone());
    assert!(ws_state.cookies.is_empty());

    let initial_cookies = vec![
        StoredCookie {
            name: "auth_token".to_string(),
            value: "secret_jwt".to_string(),
            domain: "api.example.com".to_string(),
            path: "/".to_string(),
            expires: None,
            secure: true,
            http_only: true,
            same_site: Some("Strict".to_string()),
        },
        StoredCookie {
            name: "prefs".to_string(),
            value: "compact".to_string(),
            domain: "app.example.com".to_string(),
            path: "/".to_string(),
            expires: None,
            secure: false,
            http_only: false,
            same_site: None,
        },
    ];

    ws_state.executor.restore_stored_cookies(&initial_cookies);
    ws_state.sync_cookies_from_executor();
    assert_eq!(ws_state.cookies.len(), 2);

    // Reloading workspace state from disk should restore cookies from .local/cookies.json
    let ws_reloaded = WorkspaceState::new_for_path(temp_dir.clone(), store.clone());
    assert_eq!(ws_reloaded.cookies.len(), 2);
    assert_eq!(ws_reloaded.cookies[0].name, "auth_token");

    // Remove single cookie
    ws_state.remove_cookie_internal("api.example.com", "/", "auth_token");
    assert_eq!(ws_state.cookies.len(), 1);
    assert_eq!(ws_state.cookies[0].name, "prefs");

    // Clear all cookies
    ws_state.clear_cookies_internal();
    assert!(ws_state.cookies.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rename_workspace() {
    let temp_cfg = std::env::temp_dir().join(format!(
        "zest_test_cfg_rename_{}.json",
        uuid::Uuid::new_v4()
    ));
    crate::settings::set_test_config_path(Some(temp_cfg.clone()));

    let temp_dir =
        std::env::temp_dir().join(format!("test_ws_rename_{}", uuid::Uuid::new_v4().simple()));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    store.init_workspace("Original Name").unwrap();

    let mut ws_state = WorkspaceState::new_for_path(temp_dir.clone(), store.clone());
    ws_state.workspace_meta = WorkspaceMeta {
        name: "Original Name".to_string(),
        ..Default::default()
    };
    ws_state.recent_workspaces = vec!["/path/one".to_string(), "/path/two".to_string()];

    // Rename workspace
    let success = ws_state.rename_workspace_internal("Production API");
    assert!(success);
    assert_eq!(ws_state.workspace_meta.name, "Production API");
    assert_eq!(ws_state.workspace_name(), "Production API");

    assert_eq!(store.load_workspace_meta().unwrap().name, "Original Name");

    // Blank name should fail
    assert!(!ws_state.rename_workspace_internal("   "));
    assert_eq!(ws_state.workspace_meta.name, "Production API");

    crate::settings::set_test_config_path(None);
    let _ = std::fs::remove_file(temp_cfg);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_add_recent_workspaces_normalization_and_persistence() {
    let temp_cfg =
        std::env::temp_dir().join(format!("zest_test_recent_{}.json", uuid::Uuid::new_v4()));
    crate::settings::set_test_config_path(Some(temp_cfg.clone()));

    let mut settings = crate::settings::AppSettings::load();
    assert!(settings.recent_workspaces.is_empty());

    settings.add_recent_workspace("/path/to/ws1/");
    assert_eq!(settings.recent_workspaces.len(), 1);
    assert_eq!(settings.recent_workspaces[0], "/path/to/ws1");
    assert_eq!(settings.current_workspace.as_deref(), Some("/path/to/ws1"));

    // Adding same path with or without trailing slash shouldn't duplicate
    settings.add_recent_workspace("/path/to/ws1");
    assert_eq!(settings.recent_workspaces.len(), 1);

    // Adding another workspace should prepend
    settings.add_recent_workspace("/path/to/ws2");
    assert_eq!(settings.recent_workspaces.len(), 2);
    assert_eq!(settings.recent_workspaces[0], "/path/to/ws2");
    assert_eq!(settings.recent_workspaces[1], "/path/to/ws1");
    assert_eq!(settings.current_workspace.as_deref(), Some("/path/to/ws2"));

    // Re-adding ws1 should move it to the front
    settings.add_recent_workspace("/path/to/ws1");
    assert_eq!(settings.recent_workspaces.len(), 2);
    assert_eq!(settings.recent_workspaces[0], "/path/to/ws1");
    assert_eq!(settings.recent_workspaces[1], "/path/to/ws2");

    // Verify reloading from disk maintains state
    let reloaded = crate::settings::AppSettings::load();
    assert_eq!(reloaded.recent_workspaces.len(), 2);
    assert_eq!(reloaded.recent_workspaces[0], "/path/to/ws1");
    assert_eq!(reloaded.current_workspace.as_deref(), Some("/path/to/ws1"));

    crate::settings::set_test_config_path(None);
    let _ = std::fs::remove_file(temp_cfg);
}
