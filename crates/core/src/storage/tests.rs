use super::slug::unique_slug;
use super::*;
use crate::models::{HttpMethod, Protocol};
use crate::storage::serialize::to_yaml_string;
#[test]
fn test_byte_identical_roundtrip_request() {
    let req = RequestFile {
        name: "Login".to_string(),
        method: HttpMethod::Post,
        url: "{{base_url}}/api/auth/login".to_string(),
        protocol: Protocol::Http,
        grpc: None,
        graphql: None,
        ws_message_format: Default::default(),
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

    let legacy_ws = "name: WS\ndefault_environment: local\n";
    let res_legacy = serde_yaml::from_str::<WorkspaceMeta>(legacy_ws);
    assert!(
        res_legacy.is_err(),
        "WorkspaceMeta must reject deprecated default_environment"
    );

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
fn test_protocol_defaults_and_roundtrip() {
    // Legacy YAML without a protocol key must load as HTTP and serialize
    // byte-identically without introducing the key.
    let legacy = "name: Old\nmethod: GET\nurl: https://api.com\n";
    let req: RequestFile = serde_yaml::from_str(legacy).unwrap();
    assert_eq!(req.protocol, Protocol::Http);
    assert!(req.grpc.is_none());
    let yaml = to_yaml_string(&req).unwrap();
    assert!(!yaml.contains("protocol:"), "HTTP must omit protocol key");

    // WebSocket round-trip
    let ws_yaml = "name: Echo\nmethod: GET\nurl: wss://echo.example\nprotocol: websocket\n";
    let ws_req: RequestFile = serde_yaml::from_str(ws_yaml).unwrap();
    assert_eq!(ws_req.protocol, Protocol::WebSocket);
    let ws_out = to_yaml_string(&ws_req).unwrap();
    assert!(ws_out.contains("protocol: websocket"));

    // gRPC round-trip with payload
    let grpc_yaml = concat!(
        "name: Greet\nmethod: POST\nurl: grpc://localhost:50051\nprotocol: grpc\n",
        "grpc:\n",
        "  use_reflection: true\n",
        "  service: helloworld.Greeter\n",
        "  method: SayHello\n",
        "  message_json: '{\"name\": \"Zest\"}'\n",
    );
    let grpc_req: RequestFile = serde_yaml::from_str(grpc_yaml).unwrap();
    assert_eq!(grpc_req.protocol, Protocol::Grpc);
    let payload = grpc_req.grpc.as_ref().expect("grpc payload must parse");
    assert_eq!(payload.service, "helloworld.Greeter");
    assert_eq!(payload.method, "SayHello");
    assert!(payload.use_reflection);
    assert!(payload.proto_path.is_none());
    let grpc_out = to_yaml_string(&grpc_req).unwrap();
    assert!(grpc_out.contains("protocol: grpc"));
    assert!(grpc_out.contains("service: helloworld.Greeter"));

    // gRPC rejects unknown payload fields
    let bad = concat!(
        "name: Greet\nmethod: POST\nurl: grpc://localhost\nprotocol: grpc\n",
        "grpc:\n  unknown: 1\n",
    );
    assert!(serde_yaml::from_str::<RequestFile>(bad).is_err());
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
            Protocol::Http,
        )
        .unwrap();
    store
        .create_request(
            Some(&folder_name),
            "A Req",
            HttpMethod::Get,
            "https://api.com/a",
            Protocol::Http,
        )
        .unwrap();
    store
        .create_request(
            Some(&folder_name),
            "B Req",
            HttpMethod::Get,
            "https://api.com/b",
            Protocol::Http,
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
    let temp_dir = std::env::temp_dir().join(format!("test_sec_{}", uuid::Uuid::new_v4().simple()));
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

#[path = "response_cache_tests.rs"]
mod response_cache_tests;

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
fn test_store_records_disk_fingerprints_on_writes_and_deletes() {
    let temp_dir = std::env::temp_dir().join(format!(
        "zest-fingerprint-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let store = FsWorkspaceStore::new(&temp_dir);
    store.init_workspace("FingerprintWS").unwrap();

    let env_path = temp_dir.join("environments").join("local.yaml");
    let fp = store.disk_fingerprints();
    assert!(fp.matches_disk(&env_path));

    let (_, rel_req) = store
        .create_request(
            None,
            "Ping",
            HttpMethod::Get,
            "https://example.com",
            Protocol::Http,
        )
        .unwrap();
    let req_path = temp_dir.join("collections").join(&rel_req);
    assert!(fp.matches_disk(&req_path));

    store.delete_request(&rel_req).unwrap();
    assert!(fp.matches_disk(&req_path));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[path = "request_paths_tests.rs"]
mod request_paths_tests;

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

#[test]
fn test_graphql_payload_yaml_roundtrip() {
    use crate::models::{Protocol, RequestItem};

    let mut item = RequestItem::new_with_protocol(
        "ws1",
        "Countries",
        HttpMethod::Post,
        "https://countries.trevorblades.com/graphql",
        Protocol::GraphQL,
    );
    item.graphql_mut().query = "query { country(code: \"VN\") { name } }".to_string();
    item.graphql_mut().variables = "{\"filter\": \"asia\"}".to_string();
    item.graphql_mut().operation_name = Some("Countries".to_string());

    let file = RequestFile::from_item(&item);
    let yaml = to_yaml_string(&file).unwrap();
    assert!(
        yaml.contains("protocol: graphql"),
        "protocol key serialized: {yaml}"
    );
    assert!(yaml.contains("graphql:"));

    let parsed: RequestFile = serde_yaml::from_str(&yaml).unwrap();
    let back = parsed.to_item("speedt.yaml", "ws1", None);
    assert_eq!(back.protocol, Protocol::GraphQL);
    assert_eq!(back.graphql, item.graphql);
}

#[test]
fn test_http_yaml_omits_graphql_key() {
    use crate::models::RequestItem;

    let item = RequestItem::new("ws1", "Plain", HttpMethod::Get, "https://x.dev");
    let file = RequestFile::from_item(&item);
    let yaml = to_yaml_string(&file).unwrap();
    assert!(!yaml.contains("graphql:"), "omitted for HTTP: {yaml}");
    assert!(!yaml.contains("protocol:"), "omitted for HTTP: {yaml}");
}
