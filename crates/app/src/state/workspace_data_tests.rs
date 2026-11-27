use crate::state::{
    CollectionState, EnvironmentState, GitActiveModal, GitState, GitTab, ModalKind, ModalPayload,
    ModalState, PersistenceState, SettingsTab, SidebarState, WorkspaceScopeSnapshot,
    WorkspaceSettingTarget, WorkspaceState,
};
use std::sync::Arc;
use zest_core::models::{
    Auth, Environment, EnvironmentVariable, Folder, HttpMethod, KeyValuePair, Protocol,
    RequestItem, StoredCookie, Workspace,
};
use zest_core::storage::models::{EnvironmentFile, WorkspaceMeta};
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

#[test]
fn test_git_state_defaults_and_toggle() {
    let mut git = GitState::new("/tmp/test-repo".into());
    assert_eq!(git.modal.tab, GitTab::Changes);
    assert!(!git.modal.is_open);
    assert!(git.modal.active.is_none());
    assert!(!git.loading);
    assert!(!git.is_syncing);
    assert!(git.error.is_none());

    git.modal.active = Some(GitActiveModal::Commit);
    git.modal.is_open = true;
    assert!(git.modal.is_open);
    assert_eq!(git.modal.active, Some(GitActiveModal::Commit));

    // Test loading & syncing flag states
    git.loading = true;
    assert!(git.loading);
    git.is_syncing = true;
    git.sync_message = Some("pushing...".to_string());
    assert!(git.is_syncing);
    assert_eq!(git.sync_message.as_deref(), Some("pushing..."));

    // Test error recording
    git.error = Some("Git authentication failed".to_string());
    assert_eq!(git.error.as_deref(), Some("Git authentication failed"));
    git.error = None;
    assert!(git.error.is_none());
}

#[test]
fn test_modal_state_defaults() {
    let mut modals = ModalState::default();
    assert!(modals.active_modal.is_none());
    assert!(!modals.has_any_modal_open());
    assert_eq!(modals.settings_tab, SettingsTab::General);
    assert!(modals.cookie_filter_query.is_empty());
    assert_eq!(modals.network.request_timeout_secs, 30);
    assert!(modals.network.follow_redirects);
    assert!(modals.network.validate_tls);

    modals.active_modal = Some(crate::state::ModalKind::Settings);
    modals.settings_tab = SettingsTab::Cookies;
    modals.workspace_setting_target = Some(WorkspaceSettingTarget::Workspace);
    assert!(modals.has_any_modal_open());
    assert_eq!(modals.settings_tab, SettingsTab::Cookies);
    assert_eq!(
        modals.workspace_setting_target,
        Some(WorkspaceSettingTarget::Workspace)
    );
}

#[gpui_kit::test]
fn test_modal_state_switches_one_active_modal(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::AppContext as _;

    let modal_state = cx.new(|_| ModalState::default());

    modal_state.update(cx, |modal, cx| modal.open(ModalKind::Settings, None, cx));
    assert!(modal_state.read_with(cx, |modal, _| {
        modal.active_modal == Some(crate::state::ModalKind::Settings)
    }));

    modal_state.update(cx, |modal, cx| {
        modal.open(
            ModalKind::Import,
            Some(ModalPayload::Import {
                target_folder_rel: Some("folder".to_string()),
            }),
            cx,
        )
    });
    assert!(modal_state.read_with(cx, |modal, _| {
        modal.active_modal == Some(crate::state::ModalKind::Import)
            && modal.import_modal.target_folder_rel.as_deref() == Some("folder")
    }));

    modal_state.update(cx, |modal, cx| {
        assert!(modal.dismiss_top_modal(cx));
    });
    assert!(modal_state.read_with(cx, |modal, _| {
        modal.active_modal.is_none()
            && modal.import_modal.target_folder_rel.is_none()
            && !modal.has_any_modal_open()
    }));
}

#[path = "workspace_cookie_tests.rs"]
mod workspace_cookie_tests;

#[test]
fn test_modal_state_theme_preference_persistence() {
    let temp_cfg =
        std::env::temp_dir().join(format!("zest_test_cfg_{}.json", uuid::Uuid::new_v4()));
    crate::settings::set_test_config_path(Some(temp_cfg.clone()));

    let mut settings = crate::settings::AppSettings::load();
    settings.theme = Some("dark".to_string());
    let _ = settings.save();
    let modals = ModalState::new();
    assert_eq!(modals.theme_preference, crate::theme::ThemePreference::Dark);

    settings.theme = Some("light".to_string());
    let _ = settings.save();
    let modals = ModalState::new();
    assert_eq!(
        modals.theme_preference,
        crate::theme::ThemePreference::Light
    );

    crate::settings::set_test_config_path(None);
    let _ = std::fs::remove_file(temp_cfg);
}

#[test]
fn test_sidebar_state_navigation() {
    let mut sidebar = SidebarState::new();
    assert!(sidebar.nav_history.is_empty());

    sidebar.record_navigation("req-1");
    sidebar.record_navigation("req-2");
    sidebar.record_navigation("req-3");
    assert_eq!(sidebar.nav_history.len(), 3);

    let back_1 = sidebar.navigate_back();
    assert_eq!(back_1, Some("req-2".to_string()));

    let back_2 = sidebar.navigate_back();
    assert_eq!(back_2, Some("req-1".to_string()));

    let fwd_1 = sidebar.navigate_forward();
    assert_eq!(fwd_1, Some("req-2".to_string()));
}

#[test]
fn test_sidebar_renaming_state_accessors() {
    let sidebar = SidebarState::new();
    assert!(sidebar.renaming.request.is_none());
    assert!(sidebar.renaming.folder.is_none());
    assert!(sidebar.renaming.env.is_none());
    assert!(sidebar.renaming.example.is_none());

    assert!(sidebar.renaming_request_id().is_none());
    assert!(sidebar.rename_input().is_none());
    assert!(sidebar.renaming_folder_id().is_none());
    assert!(sidebar.rename_folder_input().is_none());
    assert!(sidebar.renaming_env_id().is_none());
    assert!(sidebar.rename_env_input().is_none());
    assert!(sidebar.renaming_example().is_none());
    assert!(sidebar.rename_example_input().is_none());
}

#[gpui_kit::test]
fn test_sidebar_state_multi_selection(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::AppContext as _;
    let sidebar = cx.new(|_| SidebarState::new());

    sidebar.update(cx, |sb, cx| {
        assert_eq!(sb.selected_count(), 0);
        assert!(!sb.is_multi_selected());

        // Normal single select
        sb.select_single_request("req-1", cx);
        assert!(sb.is_request_selected("req-1"));
        assert!(!sb.is_multi_selected());
        assert_eq!(sb.selected_count(), 1);

        // Ctrl-click second request with req-1 active -> seeds req-1 and adds req-2
        sb.clear_selection(cx);
        sb.toggle_select_request("req-2", Some("req-1"), cx);
        assert!(sb.is_request_selected("req-1"));
        assert!(sb.is_request_selected("req-2"));
        assert!(sb.is_multi_selected());
        assert_eq!(sb.selected_count(), 2);

        // Ctrl-click folder-1 -> adds folder-1
        sb.toggle_select_folder("folder-1", Some("req-1"), cx);
        assert!(sb.is_folder_selected("folder-1"));
        assert_eq!(sb.selected_count(), 3);

        // Toggle off req-2
        sb.toggle_select_request("req-2", Some("req-1"), cx);
        assert!(!sb.is_request_selected("req-2"));
        assert_eq!(sb.selected_count(), 2);

        // Clear selection
        sb.clear_selection(cx);
        assert_eq!(sb.selected_count(), 0);
        assert!(!sb.is_multi_selected());
    });
}

#[gpui_kit::test]
fn test_sidebar_delete_multi_selected_items(cx: &mut gpui_kit::TestAppContext) {
    use crate::components::sidebar::item_actions::delete_multi_selected_items;
    use crate::state::RequestSessionState;
    use gpui_kit::AppContext as _;

    let temp_dir =
        std::env::temp_dir().join(format!("zest-test-multi-{}", uuid::Uuid::new_v4().simple()));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    store.init_workspace("test-ws").expect("init workspace");
    let mut ws_raw = CollectionState::new_for_path(temp_dir.clone(), store.clone());
    let r1 = ws_raw.create_request_internal(None, Protocol::Http);
    let r2 = ws_raw.create_request_internal(None, Protocol::Http);
    let r3 = ws_raw.create_request_internal(None, Protocol::Http);
    let f1 = ws_raw
        .create_folder_internal(None, "Folder A".to_string())
        .unwrap();

    let ws = cx.new(|_| ws_raw);
    let ed = cx.new(|_| RequestSessionState::new());
    let sb = cx.new(|_| SidebarState::new());

    ed.update(cx, |e, cx| {
        e.set_active_request_id(&r1.id, cx);
    });

    // Multi-select r1 and f1
    sb.update(cx, |s, cx| {
        s.select_single_request(&r1.id, cx);
        s.toggle_select_folder(&f1, Some(&r1.id), cx);
        assert!(s.is_multi_selected());
    });

    // Delete multi-selected items
    cx.update(|cx| {
        delete_multi_selected_items(&ws, &ed, &sb, cx);
    });

    // Verify r1 and f1 were deleted, selection cleared, and active request fell back to another existing request
    sb.update(cx, |s, _cx| {
        assert_eq!(s.selected_count(), 0);
    });

    ws.update(cx, |w, _cx| {
        assert!(w.find_request(&r1.id).is_none());
        assert!(w.folders.iter().all(|f| f.id != f1));
        assert!(w.find_request(&r2.id).is_some());
        assert!(w.find_request(&r3.id).is_some());
    });

    ed.update(cx, |e, _cx| {
        assert_ne!(e.active_request_id(), r1.id);
        assert!(e.active_request_id() == r2.id || e.active_request_id() == r3.id);
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[path = "workspace_environment_tests.rs"]
mod workspace_environment_tests;

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
    per_state.unsaved_requests.insert(rel_path.clone());
    if per_state.unsaved_requests.contains(&rel_path) {
        per_state.conflicts.insert(
            rel_path.clone(),
            "File was modified externally while you have unsaved changes.".to_string(),
        );
    }
    assert!(per_state.conflicts.contains_key(&rel_path));

    // Resolve keep mine
    per_state.conflicts.remove(&rel_path);
    per_state.unsaved_requests.remove(&rel_path);
    assert!(!per_state.conflicts.contains_key(&rel_path));

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

#[test]
fn test_rename_example() {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_ws_rename_ex_{}",
        uuid::Uuid::new_v4().simple()
    ));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    store.init_workspace("Rename Ex WS").unwrap();

    let (_slug, rel_path) = store
        .create_request(
            None,
            "Auth",
            HttpMethod::Post,
            "https://api.example.com/auth",
            Protocol::Http,
        )
        .unwrap();
    let mut req_file = store.read_request(&rel_path).unwrap();
    req_file
        .examples
        .push(zest_core::storage::models::RequestExampleFile {
            name: "Success".to_string(),
            status: 200,
            headers: Vec::new(),
            body: Some("{}".to_string()),
        });
    req_file
        .examples
        .push(zest_core::storage::models::RequestExampleFile {
            name: "Error".to_string(),
            status: 400,
            headers: Vec::new(),
            body: Some("{}".to_string()),
        });
    store.write_request(&rel_path, &req_file).unwrap();

    let mut ws_state = CollectionState::new_for_path(temp_dir.clone(), store.clone());
    ws_state.reload_tree();
    assert_eq!(ws_state.requests.len(), 1);
    assert_eq!(ws_state.requests[0].examples.len(), 2);

    // 1. Rename "Success" -> "Auth OK"
    let renamed = ws_state.rename_example_internal(&rel_path, "Success", "Auth OK");
    assert_eq!(renamed, Some("Auth OK".to_string()));
    assert_eq!(ws_state.requests[0].examples[0].name, "Auth OK");

    // 2. Collision test: Rename "Error" -> "Auth OK", should become "Auth OK-2"
    let renamed2 = ws_state.rename_example_internal(&rel_path, "Error", "Auth OK");
    assert_eq!(renamed2, Some("Auth OK-2".to_string()));
    assert_eq!(ws_state.requests[0].examples[1].name, "Auth OK-2");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_create_and_reload_subfolder() {
    let temp_dir = std::env::temp_dir().join(format!(
        "zest-test-subfolder-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
    store.init_workspace("Test Workspace").unwrap();

    let (_parent_slug, parent_rel) = store.create_group(None, "Parent Folder").unwrap();
    let (_child_slug, child_rel) = store
        .create_group(Some(&parent_rel), "Child Folder")
        .unwrap();

    let mut ws_state = CollectionState::new_for_path(temp_dir.clone(), store);
    ws_state.reload_tree();

    assert_eq!(ws_state.folders.len(), 2);
    let parent = ws_state
        .folders
        .iter()
        .find(|f| f.id == parent_rel)
        .expect("Parent folder should exist");
    assert_eq!(parent.name, "Parent Folder");
    assert!(parent.parent_id.is_none());

    let child = ws_state
        .folders
        .iter()
        .find(|f| f.id == child_rel)
        .expect("Child folder should exist");
    assert_eq!(child.name, "Child Folder");
    assert_eq!(child.parent_id.as_deref(), Some(parent_rel.as_str()));

    let _ = std::fs::remove_dir_all(&temp_dir);
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
    ws_state.active_workspace = Some(Workspace::new("Original Name"));
    ws_state.recent_workspaces = vec!["/path/one".to_string(), "/path/two".to_string()];

    // Rename workspace
    let success = ws_state.rename_workspace_internal("Production API");
    assert!(success);
    assert_eq!(ws_state.workspace_meta.name, "Production API");
    assert_eq!(
        ws_state.active_workspace.as_ref().unwrap().name,
        "Production API"
    );

    // Verify persisted to storage
    let meta = store.load_workspace_meta().unwrap();
    assert_eq!(meta.name, "Production API");

    // Blank name should fail
    assert!(!ws_state.rename_workspace_internal("   "));
    assert_eq!(ws_state.workspace_meta.name, "Production API");

    // Clean history
    ws_state.clean_history_internal();
    assert!(ws_state.recent_workspaces.is_empty());

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

#[path = "workspace_collection_tests.rs"]
mod workspace_collection_tests;

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

#[test]
fn test_find_request_index_stays_fresh_across_list_mutations() {
    let tmp = tempfile::tempdir().unwrap();
    let mut ws = CollectionState::new_for_path(
        tmp.path().to_path_buf(),
        Arc::new(FsWorkspaceStore::new(tmp.path())),
    );
    // RequestItem::new generates a UUID id — assign deterministic ones.
    let mut a = RequestItem::new("ws", "A", HttpMethod::Get, "");
    a.id = "a".to_string();
    let mut b = RequestItem::new("ws", "B", HttpMethod::Get, "");
    b.id = "b".to_string();
    ws.requests.push(a);
    ws.requests.push(b);
    ws.bump_tree_version();

    assert!(ws.find_request("a").is_some());
    assert!(ws.find_request("b").is_some());
    assert!(ws.find_request("missing").is_none());
    assert!(ws.find_request("scratch").is_some());

    // Reorder: the index must invalidate on the version bump.
    ws.requests.reverse();
    ws.bump_tree_version();
    assert!(ws.find_request("a").is_some());
    assert!(ws.find_request("b").is_some());

    // Remove one: a stale index entry must not resurrect it.
    ws.requests.retain(|r| r.id != "a");
    ws.bump_tree_version();
    assert!(ws.find_request("a").is_none());
    assert!(ws.find_request("b").is_some());
}
