pub mod fs_ops;
pub mod fs_store;
pub mod models;
pub mod serialize;
pub mod slug;
pub mod traits;
pub mod watcher;

pub use fs_store::FsWorkspaceStore;
pub use models::*;
pub use traits::WorkspaceStore;
pub use watcher::{start_workspace_watcher, FsEvent, WorkspaceWatcher};

#[cfg(test)]
mod tests {
    use super::slug::unique_slug;
    use super::*;
    use crate::models::HttpMethod;
    use crate::storage::serialize::to_yaml_string;
    #[test]
    fn test_byte_identical_roundtrip_request() {
        let req = RequestFile {
            name: "Login".to_string(),
            method: HttpMethod::Post,
            url: "{{base_url}}/api/auth/login".to_string(),
            params: vec![RequestKvItem {
                key: "debug".to_string(),
                value: "1".to_string(),
                enabled: false,
                description: None,
            }],
            headers: vec![RequestKvItem {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
                enabled: true,
                description: None,
            }],
            auth: RequestAuth::Bearer {
                token: "token123".to_string(),
            },
            body: RequestBodyFile::Json {
                content: "{\"email\": \"{{email}}\"}".to_string(),
            },
            settings: RequestSettingsFile {
                timeout_ms: 30000,
                follow_redirects: true,
                validate_tls: true,
            },
            script: Some(RequestScriptFile {
                pre_request: None,
                post_response: Some(
                    "if (res.status === 200) { env.token = res.body.token }".to_string(),
                ),
            }),
            examples: vec![RequestExampleFile {
                name: "Success".to_string(),
                status: 200,
                headers: vec![RequestKvItem {
                    key: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                    description: None,
                }],
                body: Some("{\"token\": \"abc\"}".to_string()),
            }],
        };

        // 1. Serialize
        let yaml_1 = to_yaml_string(&req).unwrap();

        // Check LF on Windows too
        assert!(
            !yaml_1.contains("\r\n"),
            "Must use LF line endings on all platforms"
        );
        assert!(yaml_1.ends_with('\n'), "Must have trailing newline");

        // Check key order: examples must be last!
        let examples_pos = yaml_1.find("examples:").expect("examples must be in yaml");
        let name_pos = yaml_1.find("name:").unwrap();
        assert!(name_pos < examples_pos);

        // 2. Deserialize
        let req_deser: RequestFile = serde_yaml::from_str(&yaml_1).unwrap();
        assert_eq!(req_deser.name, "Login");

        // 3. Serialize again: Must be byte-identical!
        let yaml_2 = to_yaml_string(&req_deser).unwrap();
        assert_eq!(
            yaml_1, yaml_2,
            "Roundtrip serialization must be byte-identical"
        );
    }

    #[test]
    fn test_deny_unknown_fields_on_storage_models() {
        let invalid_req = "name: Test\nmethod: GET\nurl: http://localhost\nunknown_field: 123\n";
        let res_req = serde_yaml::from_str::<RequestFile>(invalid_req);
        assert!(res_req.is_err(), "RequestFile must reject unknown fields");

        let invalid_ws = "name: WS\nunknown_ws_key: foo\n";
        let res_ws = serde_yaml::from_str::<WorkspaceMeta>(invalid_ws);
        assert!(res_ws.is_err(), "WorkspaceMeta must reject unknown fields");

        let invalid_folder = "name: Folder\nunknown_folder_key: bar\n";
        let res_folder = serde_yaml::from_str::<FolderFile>(invalid_folder);
        assert!(res_folder.is_err(), "FolderFile must reject unknown fields");

        let invalid_env = "variables: []\nunknown_env_key: baz\n";
        let res_env = serde_yaml::from_str::<EnvironmentFile>(invalid_env);
        assert!(
            res_env.is_err(),
            "EnvironmentFile must reject unknown fields"
        );
    }

    #[test]
    fn test_byte_identical_roundtrip_environment_and_folder() {
        // Environment
        let env = EnvironmentFile {
            variables: vec![
                EnvVarFile {
                    name: "base_url".to_string(),
                    value: Some("http://localhost:8002".to_string()),
                    secret: false,
                    enabled: true,
                },
                EnvVarFile {
                    name: "token".to_string(),
                    value: None,
                    secret: true,
                    enabled: true,
                },
            ],
        };

        let env_yaml_1 = to_yaml_string(&env).unwrap();
        let env_deser: EnvironmentFile = serde_yaml::from_str(&env_yaml_1).unwrap();
        let env_yaml_2 = to_yaml_string(&env_deser).unwrap();
        assert_eq!(env_yaml_1, env_yaml_2);

        // Folder
        let folder = FolderFile {
            name: "Auth".to_string(),
            order: vec![
                "login".to_string(),
                "refresh".to_string(),
                "logout".to_string(),
            ],
            headers: Vec::new(),
            auth: RequestAuth::Inherit,
        };
        let f_yaml_1 = to_yaml_string(&folder).unwrap();
        let f_deser: FolderFile = serde_yaml::from_str(&f_yaml_1).unwrap();
        let f_yaml_2 = to_yaml_string(&f_deser).unwrap();
        assert_eq!(f_yaml_1, f_yaml_2);
    }

    #[test]
    fn test_ordering_with_missing_and_extra_entries() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_order_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Test Order").unwrap();

        // Create folder auth
        let (_, group_path) = store.create_group(None, "Auth").unwrap();
        let folder_name = group_path;

        // Create requests: c_req, a_req, b_req
        store
            .create_request(
                Some(&folder_name),
                "C Req",
                HttpMethod::Get,
                "https://api.com/c",
            )
            .unwrap();
        store
            .create_request(
                Some(&folder_name),
                "A Req",
                HttpMethod::Get,
                "https://api.com/a",
            )
            .unwrap();
        store
            .create_request(
                Some(&folder_name),
                "B Req",
                HttpMethod::Get,
                "https://api.com/b",
            )
            .unwrap();

        // In _folder.yaml: order specifies ["b-req", "missing-req"]
        store
            .reorder_folder(
                Some(&folder_name),
                &["b-req".to_string(), "missing-req".to_string()],
            )
            .unwrap();

        let tree = store.scan_tree().unwrap();
        let auth_folder = tree.folders.iter().find(|f| f.dir_name == "auth").unwrap();

        // Expected order:
        // 1. "b-req" (from order)
        // 2. "missing-req" ignored silently
        // 3. remaining alphabetical: "a-req", then "c-req"
        let stems: Vec<_> = auth_folder
            .requests
            .iter()
            .map(|r| r.slug.as_str())
            .collect();
        assert_eq!(stems, vec!["b-req", "a-req", "c-req"]);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_secret_split() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_sec_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Test Sec").unwrap();

        let env_file = EnvironmentFile {
            variables: vec![
                EnvVarFile {
                    name: "public_var".to_string(),
                    value: Some("public_123".to_string()),
                    secret: false,
                    enabled: true,
                },
                EnvVarFile {
                    name: "secret_var".to_string(),
                    value: Some("super_secret_value".to_string()),
                    secret: true,
                    enabled: true,
                },
            ],
        };

        store.save_environment("prod", &env_file).unwrap();

        // 1. Inspect on-disk committed environment file
        let committed_raw =
            std::fs::read_to_string(store.environments_dir().join("prod.yaml")).unwrap();
        assert!(committed_raw.contains("public_123"));
        assert!(
            !committed_raw.contains("super_secret_value"),
            "Secret values must NOT be in environments/*.yaml"
        );

        // 2. Inspect on-disk .local/secrets/prod.yaml
        let secret_raw = std::fs::read_to_string(store.secrets_dir().join("prod.yaml")).unwrap();
        assert!(secret_raw.contains("super_secret_value"));

        // 3. Load environment back through store: secrets should be merged
        let loaded = store.load_environment("prod").unwrap();
        let sec_item = loaded
            .variables
            .iter()
            .find(|v| v.name == "secret_var")
            .unwrap();
        assert_eq!(sec_item.value.as_deref(), Some("super_secret_value"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_globals_persistence_and_secret_split() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_globals_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Test Globals").unwrap();

        // 1. Initial state: no globals.yaml -> loads default empty
        let initial_globals = store.load_global_environment().unwrap();
        assert!(initial_globals.variables.is_empty());

        // 2. Save globals with public and secret variables
        let globals = EnvironmentFile {
            variables: vec![
                EnvVarFile {
                    name: "api_url".to_string(),
                    value: Some("https://api.example.com".to_string()),
                    secret: false,
                    enabled: true,
                },
                EnvVarFile {
                    name: "global_token".to_string(),
                    value: Some("top_secret_token_999".to_string()),
                    secret: true,
                    enabled: true,
                },
            ],
        };
        store.save_global_environment(&globals).unwrap();

        // 3. Inspect committed file: public value present, secret value stripped
        let committed_path = store.environments_dir().join("globals.yaml");
        assert!(committed_path.exists());
        let committed_raw = std::fs::read_to_string(&committed_path).unwrap();
        assert!(committed_raw.contains("https://api.example.com"));
        assert!(
            !committed_raw.contains("top_secret_token_999"),
            "Global secrets must not appear in environments/globals.yaml"
        );

        // 4. Inspect .local/secrets/globals.yaml
        let secret_path = store.secrets_dir().join("globals.yaml");
        assert!(secret_path.exists());
        let secret_raw = std::fs::read_to_string(&secret_path).unwrap();
        assert!(secret_raw.contains("top_secret_token_999"));

        // 5. list_environments() must NOT list "globals"
        let env_list = store.list_environments().unwrap();
        assert!(!env_list.contains(&"globals".to_string()));

        // 6. Reload globals through store: secret value is merged back
        let loaded = store.load_global_environment().unwrap();
        let pub_v = loaded
            .variables
            .iter()
            .find(|v| v.name == "api_url")
            .unwrap();
        assert_eq!(pub_v.value.as_deref(), Some("https://api.example.com"));
        let sec_v = loaded
            .variables
            .iter()
            .find(|v| v.name == "global_token")
            .unwrap();
        assert_eq!(sec_v.value.as_deref(), Some("top_secret_token_999"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_request_lifecycle_and_orphan_loc_cleanup() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_life_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Test Life").unwrap();

        // 1. Create request
        let (slug, rel_path) = store
            .create_request(None, "Login", HttpMethod::Post, "http://api.com")
            .unwrap();
        assert_eq!(slug, "login");
        assert_eq!(rel_path, "login.yaml");

        // Save a response cache
        let cache = LocFile {
            sent_at: "2026-10-15T12:00:00Z".to_string(),
            request: LocRequestInfo {
                method: HttpMethod::Post,
                url: "http://api.com".to_string(),
                headers: vec![],
                body: None,
            },
            response: LocResponseInfo {
                status: 200,
                duration_ms: 15,
                size: 2,
                headers: vec![],
                body: Some("{}".to_string()),
                truncated: false,
                binary: false,
            },
        };
        store.save_response_cache(&rel_path, &cache).unwrap();
        assert!(store.collections_dir().join("login.loc").exists());

        // 2. Rename request
        let new_rel = store.rename_request(&rel_path, "Login User").unwrap();
        assert_eq!(new_rel, "login-user.yaml");
        assert!(store.collections_dir().join("login-user.yaml").exists());
        assert!(store.collections_dir().join("login-user.loc").exists());
        assert!(!store.collections_dir().join("login.yaml").exists());
        assert!(!store.collections_dir().join("login.loc").exists());

        // 2b. Same-slug rename must preserve exact stem, never appending -2
        let same_slug_rel = store.rename_request(&new_rel, "login user").unwrap();
        assert_eq!(same_slug_rel, "login-user.yaml");
        assert!(store.collections_dir().join("login-user.yaml").exists());
        assert!(store.collections_dir().join("login-user.loc").exists());

        // 2c. Rename when target .loc already exists (pre-existing or orphan)
        let ghost_target_loc = store.collections_dir().join("login-admin.loc");
        std::fs::write(&ghost_target_loc, "{\"stale\": true}").unwrap();
        assert!(ghost_target_loc.exists());
        let admin_rel = store.rename_request(&same_slug_rel, "Login Admin").unwrap();
        assert_eq!(admin_rel, "login-admin.yaml");
        assert!(store.collections_dir().join("login-admin.yaml").exists());
        assert!(store.collections_dir().join("login-admin.loc").exists());
        assert!(!store.collections_dir().join("login-user.yaml").exists());
        assert!(!store.collections_dir().join("login-user.loc").exists());
        // Verify target .loc contains the real cached response, not the stale ghost
        let admin_loc_content =
            std::fs::read_to_string(store.collections_dir().join("login-admin.loc")).unwrap();
        assert!(admin_loc_content.contains("\"status\": 200"));

        let new_rel = admin_rel;

        // 3. Move request into group
        let (_, group_rel) = store.create_group(None, "Auth").unwrap();
        let moved_rel = store.move_request(&new_rel, Some(&group_rel)).unwrap();
        assert_eq!(moved_rel, "auth/login-admin.yaml");
        assert!(store
            .collections_dir()
            .join("auth/login-admin.yaml")
            .exists());
        assert!(store
            .collections_dir()
            .join("auth/login-admin.loc")
            .exists());

        // 4. Create an orphan .loc in auth/
        let orphan_loc = store.collections_dir().join("auth/ghost.loc");
        std::fs::write(&orphan_loc, "{}").unwrap();
        assert!(orphan_loc.exists());

        // Scanning should clean up the orphan .loc lazily
        let _ = store.scan_tree().unwrap();
        assert!(
            !orphan_loc.exists(),
            "Orphan .loc must be deleted lazily on scan"
        );

        // 5. Delete group leaves nothing behind
        store.delete_group(&group_rel).unwrap();
        assert!(!store.collections_dir().join("auth").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_loc_cache_binary_and_large_truncation() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_cache_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Test Cache").unwrap();

        let (_, rel_path) = store
            .create_request(None, "Large", HttpMethod::Get, "http://api.com")
            .unwrap();

        // 1. Large body > 512KB
        let huge_string = "x".repeat(600 * 1024);
        let cache = LocFile {
            sent_at: "2026-10-15T12:00:00Z".to_string(),
            request: LocRequestInfo {
                method: HttpMethod::Get,
                url: "http://api.com".to_string(),
                headers: vec![],
                body: None,
            },
            response: LocResponseInfo {
                status: 200,
                duration_ms: 100,
                size: huge_string.len(),
                headers: vec![],
                body: Some(huge_string),
                truncated: false,
                binary: false,
            },
        };
        store.save_response_cache(&rel_path, &cache).unwrap();

        let loaded = store.load_response_cache(&rel_path).unwrap().unwrap();
        assert!(loaded.response.truncated, "Must be truncated");
        assert_eq!(loaded.response.body.unwrap().len(), 512 * 1024);

        // 2. Binary body stores metadata only
        let bin_cache = LocFile {
            sent_at: "2026-10-15T12:00:00Z".to_string(),
            request: LocRequestInfo {
                method: HttpMethod::Get,
                url: "http://api.com".to_string(),
                headers: vec![],
                body: None,
            },
            response: LocResponseInfo {
                status: 200,
                duration_ms: 50,
                size: 1024,
                headers: vec![],
                body: Some("raw_bytes".to_string()),
                truncated: false,
                binary: true,
            },
        };
        store.save_response_cache(&rel_path, &bin_cache).unwrap();
        let loaded_bin = store.load_response_cache(&rel_path).unwrap().unwrap();
        assert!(loaded_bin.response.binary);
        assert_eq!(loaded_bin.response.body, None, "Binary body should be null");

        // 3. Corrupt .loc ignored silently
        let loc_path = store.collections_dir().join("large.loc");
        std::fs::write(&loc_path, "{ broken json...").unwrap();
        assert_eq!(store.load_response_cache(&rel_path).unwrap(), None);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_burst_state_changes_and_pins() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_burst_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Burst Workspace").unwrap();

        // Simulate rapid burst of 100 state changes
        let mut state = store.load_state().unwrap();
        for i in 0..100 {
            state.active_request = Some(format!("req_{}.yaml", i));
            state.sidebar.width = 250.0 + (i as f32);
            state.recent.push(format!("req_{}.yaml", i));
        }
        // Save the resulting state (as done after debounce)
        store.save_state(&state).unwrap();

        let loaded = store.load_state().unwrap();
        assert_eq!(loaded.active_request.as_deref(), Some("req_99.yaml"));
        assert_eq!(loaded.sidebar.width, 349.0);
        assert_eq!(loaded.recent.len(), 100);

        // Test pins save and load
        let pins = vec!["auth/login.yaml".to_string(), "users/get.yaml".to_string()];
        store.save_pins(&pins).unwrap();
        let loaded_pins = store.load_pins().unwrap();
        assert_eq!(loaded_pins, pins);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_invalid_yaml_handling_and_corrupt_secret_backup() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_invalid_yaml_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Invalid Yaml Test").unwrap();

        // 1. Write an invalid request YAML
        let req_path = store.collections_dir().join("broken.yaml");
        std::fs::write(
            &req_path,
            "name: Broken\nmethod: NOT_A_METHOD\nurl: ://broken\n  invalid_indent",
        )
        .unwrap();

        let tree = store.scan_tree().unwrap();
        assert_eq!(tree.root_requests.len(), 1);
        let node = &tree.root_requests[0];
        assert_eq!(node.rel_path, "broken.yaml");
        assert!(
            node.parse_error.is_some(),
            "Node must contain parse error message"
        );

        // 2. Corrupt secret file: should rename to *.bak and return Err
        let secret_path = store.secrets_dir().join("local.yaml");
        std::fs::create_dir_all(store.secrets_dir()).unwrap();
        std::fs::write(&secret_path, "invalid: : yaml content").unwrap();

        let load_res = store.load_secrets("local");
        assert!(
            load_res.is_err(),
            "Must return error for corrupt secret file"
        );

        let bak_path = store.secrets_dir().join("local.yaml.bak");
        assert!(
            bak_path.exists(),
            "Corrupt secret file must be backed up to .bak"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_gitignore_management() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_gitignore_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Gitignore Test").unwrap();

        let gitignore = temp_dir.join(".gitignore");
        assert!(gitignore.exists());
        let content = std::fs::read_to_string(&gitignore).unwrap();
        assert!(content.contains(".local/"));
        assert!(content.contains("*.loc"));

        // Running ensure_gitignore again is idempotent
        let tracked = store.ensure_gitignore().unwrap();
        assert!(!tracked);
        let content2 = std::fs::read_to_string(&gitignore).unwrap();
        assert_eq!(content, content2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_suppress_path_worker_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!(
            "zest-suppress-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let store = FsWorkspaceStore::new(&temp_dir);
        let test_path = temp_dir.join("test.yaml");

        assert!(!store
            .suppressed_paths()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&test_path));
        store.suppress_path(&test_path);
        assert!(store
            .suppressed_paths()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&test_path));

        // Calling multiple times in a burst does not panic and tracks suppressed paths via single queue
        for i in 0..50 {
            let p = temp_dir.join(format!("burst_{}.yaml", i));
            store.suppress_path(&p);
            assert!(store
                .suppressed_paths()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(&p));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_unique_slug_skips_yml_twin() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_slug_yml_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // The scanner loads .yml files as requests too, so a .yml twin must
        // count as an occupied stem even though callers probe "yaml".
        std::fs::write(temp_dir.join("login.yml"), "name: Login\n").unwrap();
        assert_eq!(unique_slug(&temp_dir, "login", "yaml"), "login-2");

        std::fs::write(temp_dir.join("login-2.yml"), "name: Login 2\n").unwrap();
        assert_eq!(unique_slug(&temp_dir, "login", "yaml"), "login-3");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_request_self_exclusion_keeps_own_suffix() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_self_excl_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Self Exclude").unwrap();

        let (_, login_rel) = store
            .create_request(None, "Login", HttpMethod::Post, "http://a.com")
            .unwrap();
        let (_, login2_rel) = store
            .create_request(None, "Login 2", HttpMethod::Get, "http://a.com")
            .unwrap();
        assert_eq!(login_rel, "login.yaml");
        assert_eq!(login2_rel, "login-2.yaml");

        // Renaming the suffixed item to "Login" (slug taken by the sibling)
        // must keep its own stem login-2 instead of jumping to login-3.
        let renamed = store.rename_request(&login2_rel, "Login").unwrap();
        assert_eq!(renamed, "login-2.yaml");
        assert!(store.collections_dir().join("login-2.yaml").exists());
        assert!(!store.collections_dir().join("login-3.yaml").exists());
        let file = store.read_request("login-2.yaml").unwrap();
        assert_eq!(file.name, "Login");

        // A new third "Login" gets the next free suffix.
        let (_, login3_rel) = store
            .create_request(None, "Login", HttpMethod::Get, "http://a.com")
            .unwrap();
        assert_eq!(login3_rel, "login-3.yaml");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_request_updates_folder_order_lists() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_move_order_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Move Order").unwrap();

        let (_, src_folder) = store.create_group(None, "Source").unwrap();
        let (_, dst_folder) = store.create_group(None, "Dest").unwrap();
        let (_, login_rel) = store
            .create_request(
                Some(&src_folder),
                "Login",
                HttpMethod::Post,
                "http://api.com",
            )
            .unwrap();
        store
            .create_request(
                Some(&src_folder),
                "Other",
                HttpMethod::Get,
                "http://api.com",
            )
            .unwrap();
        store
            .reorder_folder(
                Some(&src_folder),
                &["login".to_string(), "other".to_string()],
            )
            .unwrap();

        let moved = store.move_request(&login_rel, Some(&dst_folder)).unwrap();
        assert_eq!(moved, format!("{}/login.yaml", dst_folder));

        let src_meta = store.load_folder_meta(&src_folder).unwrap();
        assert!(
            !src_meta.order.contains(&"login".to_string()),
            "moved stem must be dropped from the source order list, got {:?}",
            src_meta.order
        );
        assert_eq!(src_meta.order, vec!["other".to_string()]);

        let dst_meta = store.load_folder_meta(&dst_folder).unwrap();
        assert!(
            dst_meta.order.contains(&"login".to_string()),
            "moved stem must be appended to the destination order list"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_group_same_parent_keeps_name() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_move_group_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Move Group").unwrap();

        let (_, group_rel) = store.create_group(None, "Auth").unwrap();
        assert_eq!(group_rel, "auth");

        // A same-parent move is a no-op on disk: the directory must keep its
        // name instead of being suffixed to "auth-2".
        let moved = store.move_group(&group_rel, None).unwrap();
        assert_eq!(moved, "auth");
        assert!(store.collections_dir().join("auth").exists());
        assert!(!store.collections_dir().join("auth-2").exists());

        // Cross-parent move with a name clash resolves to a suffixed dir.
        let (_, parent_rel) = store.create_group(None, "Parent").unwrap();
        let (_, clash_rel) = store.create_group(Some(&parent_rel), "Auth").unwrap();
        assert_eq!(clash_rel, "parent/auth");
        let moved_clash = store.move_group(&group_rel, Some(&parent_rel)).unwrap();
        assert_eq!(moved_clash, "parent/auth-2");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_environment_collision_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_env_rename_{}", uuid::Uuid::new_v4().simple()));
        let store = FsWorkspaceStore::new(&temp_dir);
        store.init_workspace("Env Rename").unwrap();

        store
            .save_environment("dev", &EnvironmentFile::default())
            .unwrap();
        store
            .save_environment("prod", &EnvironmentFile::default())
            .unwrap();

        // Renaming onto an existing environment must fail instead of silently
        // replacing the sibling file (and its secrets).
        let res = store.rename_environment("dev", "prod");
        assert!(res.is_err(), "rename onto existing env must be rejected");
        assert!(store.environments_dir().join("dev.yaml").exists());
        assert!(store.environments_dir().join("prod.yaml").exists());

        // Same-name rename stays a no-op success.
        assert!(store.rename_environment("dev", "dev").is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_benchmark_yaml_and_clone() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_benchmark_yaml_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let speedt_path = temp_dir.join("speedt.yaml");

        // Generate synthetic ~16KB request file for benchmarking
        let mut headers = String::new();
        for i in 0..150 {
            headers.push_str(&format!(
                "  - key: X-Benchmark-Header-{}\n    value: benchmark-value-header-payload-{}\n    enabled: true\n",
                i, i
            ));
        }
        let sample_content = format!(
            "name: Benchmark Request\nmethod: POST\nurl: https://api.benchmark.example.com/v1/payload\nheaders:\n{}body:\n  type: json\n  content: '{{\"benchmark\": true}}'\n",
            headers
        );
        std::fs::write(&speedt_path, sample_content).unwrap();

        let content = std::fs::read_to_string(&speedt_path).unwrap();
        let t0 = std::time::Instant::now();
        let req_file: RequestFile = serde_yaml::from_str(&content).unwrap();
        let parse_time = t0.elapsed();

        let t1 = std::time::Instant::now();
        let serialized = to_yaml_string(&req_file).unwrap();
        let ser_time = t1.elapsed();

        let item = req_file.to_item("speedt.yaml", "ws", None);
        let t2 = std::time::Instant::now();
        for _ in 0..100 {
            let _ = item.clone();
        }
        let clone_time = t2.elapsed();

        assert!(!serialized.is_empty());
        assert_eq!(item.name, "Benchmark Request");
        let _ = (parse_time, ser_time, clone_time);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
