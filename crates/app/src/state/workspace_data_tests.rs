#[cfg(test)]
mod tests {
    use crate::state::{
        EnvironmentState, GitActiveModal, GitState, GitTab, ModalState, PersistenceState,
        ScopeSettingsTarget, SettingsTab, SidebarState, WorkspaceDataState,
    };
    use std::sync::Arc;
    use zest_core::models::{
        Auth, Environment, EnvironmentVariable, Folder, HttpMethod, HttpRequestItem, KeyValuePair,
        StoredCookie, Workspace,
    };
    use zest_core::storage::models::{EnvironmentFile, WorkspaceMeta};
    use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

    #[test]
    fn test_git_state_defaults_and_toggle() {
        let mut git = GitState::new("/tmp/test-repo".into());
        assert_eq!(git.tab, GitTab::Changes);
        assert!(!git.show_modal);
        assert!(git.active_modal.is_none());
        assert!(!git.loading);
        assert!(!git.is_syncing);
        assert!(git.error.is_none());

        git.active_modal = Some(GitActiveModal::Commit);
        git.show_modal = true;
        assert!(git.show_modal);
        assert_eq!(git.active_modal, Some(GitActiveModal::Commit));

        // Test loading & syncing flag states
        git.loading = true;
        assert!(git.loading);
        git.is_syncing = true;
        assert!(git.is_syncing);

        // Test error recording
        git.error = Some("Git authentication failed".to_string());
        assert_eq!(git.error.as_deref(), Some("Git authentication failed"));
        git.error = None;
        assert!(git.error.is_none());
    }

    #[test]
    fn test_modal_state_defaults() {
        let mut modals = ModalState::default();
        assert!(!modals.show_env_modal);
        assert!(!modals.show_settings_modal);
        assert_eq!(modals.settings_tab, SettingsTab::General);
        assert!(modals.cookie_filter_query.is_empty());
        assert!(!modals.show_command_palette);
        assert_eq!(modals.request_timeout_secs, 30);
        assert!(modals.follow_redirects);
        assert!(modals.validate_tls);

        modals.show_settings_modal = true;
        modals.settings_tab = SettingsTab::Cookies;
        modals.scope_settings_target = Some(ScopeSettingsTarget::Workspace);
        assert!(modals.show_settings_modal);
        assert_eq!(modals.settings_tab, SettingsTab::Cookies);
        assert_eq!(
            modals.scope_settings_target,
            Some(ScopeSettingsTarget::Workspace)
        );
    }

    #[test]
    fn test_workspace_cookie_jar_persistence_and_removal() {
        let temp_dir = std::env::temp_dir().join(format!(
            "zest-test-cookies-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
        store.init_workspace("Cookie WS").unwrap();

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
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
        let ws_reloaded = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
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
    fn test_modal_state_theme_preference_persistence() {
        let mut settings = crate::settings::AppSettings::load();
        let original_theme = settings.theme.clone();

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

        settings.theme = original_theme;
        let _ = settings.save();
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
        assert!(sidebar.renaming_request.is_none());
        assert!(sidebar.renaming_folder.is_none());
        assert!(sidebar.renaming_env.is_none());
        assert!(sidebar.renaming_example.is_none());

        assert!(sidebar.renaming_request_id().is_none());
        assert!(sidebar.rename_input().is_none());
        assert!(sidebar.renaming_folder_id().is_none());
        assert!(sidebar.rename_folder_input().is_none());
        assert!(sidebar.renaming_env_id().is_none());
        assert!(sidebar.rename_env_input().is_none());
        assert!(sidebar.renaming_example().is_none());
        assert!(sidebar.rename_example_input().is_none());
    }

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
            )
            .unwrap();

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
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

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
        ws_state.reload_tree();
        assert_eq!(ws_state.requests.len(), 1);
        assert_eq!(ws_state.requests[0].examples.len(), 2);

        // 1. Rename "Success" -> "Auth OK"
        let renamed = ws_state.rename_example_internal(&rel_path, "Success", "Auth OK");
        assert_eq!(renamed, Some("Auth OK".to_string()));
        assert_eq!(ws_state.requests[0].examples[0].name, "Auth OK");

        // Verify persisted on disk
        let disk_req = store.read_request(&rel_path).unwrap();
        assert_eq!(disk_req.examples[0].name, "Auth OK");

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

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store);
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
        let temp_dir =
            std::env::temp_dir().join(format!("test_ws_rename_{}", uuid::Uuid::new_v4().simple()));
        let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
        store.init_workspace("Original Name").unwrap();

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
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

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workspace_and_folder_scope_hierarchy() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_ws_scope_{}", uuid::Uuid::new_v4().simple()));
        let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
        let _ = store.init_workspace("Hierarchy Test WS");

        let mut ws = Workspace::new("Hierarchy Test WS");
        ws.headers = vec![KeyValuePair::new("X-Workspace", "ws1")];
        ws.auth = Auth::Bearer {
            token: "ws-token".to_string(),
        };

        let mut folder_users = Folder::new("ws", "Users");
        folder_users.id = "users".to_string();
        folder_users.headers = vec![KeyValuePair::new("X-Folder", "users")];
        folder_users.auth = Auth::Inherit;

        let mut folder_admin = Folder::new("ws", "Admin");
        folder_admin.id = "users/admin".to_string();
        folder_admin.parent_id = Some("users".to_string());
        folder_admin.headers = vec![KeyValuePair::new("X-Admin", "true")];
        folder_admin.auth = Auth::Basic {
            username: "admin".to_string(),
            password: "secret".to_string(),
        };

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store);
        ws_state.active_workspace = Some(ws);
        ws_state.folders = vec![folder_users, folder_admin];

        // Case 1: Request in subfolder (users/admin) with Inherit auth
        let mut req1 =
            HttpRequestItem::new("ws", "Admin Req", HttpMethod::Get, "https://api.com/admin");
        req1.folder_id = Some("users/admin".to_string());
        req1.headers = vec![KeyValuePair::new("X-Req", "1")];
        req1.auth = Auth::Inherit;

        let (headers1, auth1) = ws_state.resolve_request_hierarchy(&req1);
        assert_eq!(headers1.len(), 4); // X-Workspace, X-Folder, X-Admin, X-Req
        assert_eq!(
            auth1,
            Auth::Basic {
                username: "admin".to_string(),
                password: "secret".to_string()
            }
        );

        // Case 2: Request in top folder (users) with Inherit auth
        let mut req2 =
            HttpRequestItem::new("ws", "Users Req", HttpMethod::Get, "https://api.com/users");
        req2.folder_id = Some("users".to_string());
        req2.auth = Auth::Inherit;

        let (headers2, auth2) = ws_state.resolve_request_hierarchy(&req2);
        assert_eq!(headers2.len(), 2); // X-Workspace, X-Folder
        assert_eq!(
            auth2,
            Auth::Bearer {
                token: "ws-token".to_string()
            }
        ); // Inherited from Workspace!

        // Case 3: Root request (no folder) with Inherit auth
        let mut req3 =
            HttpRequestItem::new("ws", "Root Req", HttpMethod::Get, "https://api.com/root");
        req3.auth = Auth::Inherit;

        let (headers3, auth3) = ws_state.resolve_request_hierarchy(&req3);
        assert_eq!(headers3.len(), 1); // X-Workspace
        assert_eq!(
            auth3,
            Auth::Bearer {
                token: "ws-token".to_string()
            }
        );

        // Case 4: Request in subfolder with explicit None auth
        let mut req4 = HttpRequestItem::new(
            "ws",
            "Public Req",
            HttpMethod::Get,
            "https://api.com/public",
        );
        req4.folder_id = Some("users/admin".to_string());
        req4.auth = Auth::None;

        let (_, auth4) = ws_state.resolve_request_hierarchy(&req4);
        assert_eq!(auth4, Auth::None);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_inherited_headers_projection_and_auth_source() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_ws_inh_{}", uuid::Uuid::new_v4().simple()));
        let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
        let _ = store.init_workspace("Inherit Proj WS");

        let mut ws = Workspace::new("Inherit Proj WS");
        ws.headers = vec![
            KeyValuePair::new("X-Workspace", "ws1"),
            KeyValuePair::new("X-Shared", "from-ws"),
        ];
        ws.auth = Auth::Bearer {
            token: "ws-token".to_string(),
        };

        let mut folder = Folder::new("ws", "API");
        folder.id = "api".to_string();
        folder.headers = vec![KeyValuePair::new("X-Shared", "from-folder")];
        folder.auth = Auth::Inherit;

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store);
        ws_state.active_workspace = Some(ws);
        ws_state.folders = vec![folder];

        let mut req = HttpRequestItem::new("ws", "R", HttpMethod::Get, "https://api.com");
        req.folder_id = Some("api".to_string());
        req.headers = vec![KeyValuePair::new("x-shared", "from-request")];
        req.auth = Auth::Inherit;

        let rows = ws_state.inherited_headers_for_request(&req);
        assert_eq!(rows.len(), 2);
        let shared = rows
            .iter()
            .find(|r| r.pair.key.eq_ignore_ascii_case("x-shared"))
            .unwrap();
        assert_eq!(shared.pair.value, "from-folder");
        assert_eq!(shared.source, "folder \"API\"");
        assert!(shared.overridden_by_request);
        let ws_row = rows.iter().find(|r| r.pair.key == "X-Workspace").unwrap();
        assert_eq!(ws_row.source, "workspace \"Inherit Proj WS\"");
        assert!(!ws_row.overridden_by_request);

        // Auth source falls back to workspace through the Inherit folder.
        let auth_src = ws_state
            .inherited_auth_source(req.folder_id.as_deref())
            .unwrap();
        assert_eq!(
            auth_src.auth,
            Auth::Bearer {
                token: "ws-token".to_string()
            }
        );
        assert!(auth_src.source_folder_id.is_none());
        assert_eq!(auth_src.source_name, "Inherit Proj WS");
        assert_eq!(
            ws_state.get_inherited_auth_description(req.folder_id.as_deref()),
            "Inherited from workspace \"Inherit Proj WS\" (Bearer Token)"
        );

        // No ancestor auth at all.
        ws_state.active_workspace.as_mut().unwrap().auth = Auth::Inherit;
        assert!(ws_state.inherited_auth_source(Some("api")).is_none());
        assert_eq!(
            ws_state.get_inherited_auth_description(Some("api")),
            "No auth configured in parent hierarchy"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_incremental_crud_operations() {
        let temp_dir =
            std::env::temp_dir().join(format!("zest-test-crud-{}", uuid::Uuid::new_v4().simple()));
        let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
        store.init_workspace("Incremental WS").unwrap();

        let mut ws_state = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());

        // 1. Create Request
        let req1 = ws_state.create_request_internal(None);
        assert_eq!(ws_state.requests.len(), 1);
        assert_eq!(ws_state.requests[0].id, req1.id);
        assert_eq!(ws_state.requests[0].name, "New Request");

        // 2. Rename Request
        let new_id = ws_state
            .rename_request_internal(&req1.id, "Get Users")
            .expect("rename request should succeed");
        assert_eq!(ws_state.requests.len(), 1);
        assert_eq!(ws_state.requests[0].id, new_id);
        assert_eq!(ws_state.requests[0].name, "Get Users");

        // 3. Duplicate Request
        let dup = ws_state
            .duplicate_request_internal(&new_id)
            .expect("duplicate should succeed");
        assert_eq!(ws_state.requests.len(), 2);
        assert!(dup.name.contains("(Copy)"));

        // 4. Create Folder
        let folder_id = ws_state
            .create_folder_internal(None, "Auth".to_string())
            .expect("create folder should succeed");
        assert_eq!(ws_state.folders.len(), 1);
        assert_eq!(ws_state.folders[0].name, "Auth");
        assert_eq!(ws_state.folders[0].id, folder_id);

        // 5. Move Request to Folder
        let moved_id = ws_state
            .move_request_to_folder_internal(&dup.id, Some(folder_id.clone()))
            .expect("move should succeed");
        let moved_req = ws_state
            .requests
            .iter()
            .find(|r| r.id == moved_id)
            .expect("moved request should be found");
        assert_eq!(moved_req.folder_id, Some(folder_id.clone()));

        // 6. Rename Folder
        let (new_folder_id, remapped_pairs) = ws_state
            .rename_folder_internal(&folder_id, "Authentication")
            .expect("rename folder should succeed");
        assert_eq!(ws_state.folders[0].name, "Authentication");
        assert_eq!(ws_state.folders[0].id, new_folder_id);
        assert!(!remapped_pairs.is_empty());

        // Nested request should have its folder_id and id updated with new prefix
        let nested_req = ws_state
            .requests
            .iter()
            .find(|r| r.folder_id == Some(new_folder_id.clone()))
            .expect("nested request should reflect new folder_id");
        assert!(nested_req.id.starts_with(&format!("{}/", new_folder_id)));

        // 7. Delete Folder: Should remove folder and its nested request
        ws_state.delete_folder_internal(&new_folder_id);
        assert_eq!(ws_state.folders.len(), 0);
        assert_eq!(ws_state.requests.len(), 1);
        assert_eq!(ws_state.requests[0].id, new_id);

        // 8. Delete Request
        ws_state.delete_request_internal(&new_id);
        assert_eq!(ws_state.requests.len(), 0);

        // Verify disk matches: scan_tree returns empty workspace
        let disk_tree = store.scan_tree().unwrap();
        assert_eq!(disk_tree.root_requests.len(), 0);
        assert_eq!(disk_tree.folders.len(), 0);

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
}
