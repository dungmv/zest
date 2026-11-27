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
