use super::*;
use super::*;
use crate::models::{Environment, EnvironmentVariable};

#[test]
fn test_environment_file_conversion_and_secrets() {
    let mut env = Environment::new("ws1", "Development");
    env.id = "env-dev".to_string();
    env.variables.push(EnvironmentVariable {
        key: "BASE_URL".to_string(),
        initial_value: "https://api.example.com".to_string(),
        current_value: "http://localhost:8000".to_string(),
        secret: false,
    });
    env.variables.push(EnvironmentVariable {
        key: "TIMEOUT".to_string(),
        initial_value: "30".to_string(),
        current_value: "".to_string(),
        secret: false,
    });
    env.variables.push(EnvironmentVariable {
        key: "API_KEY".to_string(),
        initial_value: "prod-secret".to_string(),
        current_value: "local-dev-secret".to_string(),
        secret: true,
    });

    let (env_file, secrets) = EnvironmentFile::from_environment(&env);

    // Env file only holds initial_value for non-secrets, None for secrets
    assert_eq!(env_file.variables.len(), 3);
    assert_eq!(env_file.variables[0].name, "BASE_URL");
    assert_eq!(
        env_file.variables[0].value.as_deref(),
        Some("https://api.example.com")
    );
    assert!(!env_file.variables[0].secret);

    assert_eq!(env_file.variables[1].name, "TIMEOUT");
    assert_eq!(env_file.variables[1].value.as_deref(), Some("30"));
    assert!(!env_file.variables[1].secret);

    assert_eq!(env_file.variables[2].name, "API_KEY");
    assert_eq!(env_file.variables[2].value, None);
    assert!(env_file.variables[2].secret);

    // Secrets hold current values and secret values
    assert_eq!(
        secrets.get("BASE_URL").map(|s| s.as_str()),
        Some("http://localhost:8000")
    );
    assert!(!secrets.contains_key("TIMEOUT")); // empty current value not stored
    assert_eq!(
        secrets.get("API_KEY").map(|s| s.as_str()),
        Some("local-dev-secret")
    );

    // Converting back from EnvironmentFile + secrets
    let restored = env_file.to_environment("env-dev", "ws1", "Development", &secrets);
    assert_eq!(restored.variables.len(), 3);

    let v0 = &restored.variables[0];
    assert_eq!(v0.key, "BASE_URL");
    assert_eq!(v0.initial_value, "https://api.example.com");
    assert_eq!(v0.current_value, "http://localhost:8000");
    assert_eq!(v0.resolved_value(), "http://localhost:8000");

    let v1 = &restored.variables[1];
    assert_eq!(v1.key, "TIMEOUT");
    assert_eq!(v1.initial_value, "30");
    assert_eq!(v1.current_value, "");
    assert_eq!(v1.resolved_value(), "30");

    let v2 = &restored.variables[2];
    assert_eq!(v2.key, "API_KEY");
    assert_eq!(v2.current_value, "local-dev-secret");
    assert_eq!(v2.resolved_value(), "local-dev-secret");
    assert!(v2.secret);
}

#[test]
fn test_loc_file_to_record_status_text() {
    let loc = LocFile {
        sent_at: "2026-10-21T10:00:00Z".to_string(),
        request: LocRequestInfo {
            method: crate::models::HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: vec![],
            body: None,
        },
        response: LocResponseInfo {
            status: 200,
            duration_ms: 50,
            size: 10,
            headers: vec![],
            body: Some("{}".to_string()),
            truncated: false,
            binary: false,
        },
    };
    let rec = loc.to_record("req-1");
    assert_eq!(rec.status_code, 200);
    assert_eq!(rec.status_text, "OK");

    let mut loc_404 = loc;
    loc_404.response.status = 404;
    let rec_404 = loc_404.to_record("req-2");
    assert_eq!(rec_404.status_code, 404);
    assert_eq!(rec_404.status_text, "Not Found");
}

#[test]
fn test_workspace_tree_counting() {
    let req1 = RequestNode {
        slug: "r1".into(),
        rel_path: "r1.yaml".into(),
        file: RequestFile::default(),
        last_status_code: None,
        parse_error: None,
    };
    let req2 = RequestNode {
        slug: "r2".into(),
        rel_path: "sub/r2.yaml".into(),
        file: RequestFile::default(),
        last_status_code: None,
        parse_error: None,
    };
    let subfolder = FolderNode {
        dir_name: "nested".into(),
        rel_path: "sub/nested".into(),
        file: FolderFile::default(),
        requests: vec![req2],
        subfolders: vec![],
    };
    let folder = FolderNode {
        dir_name: "sub".into(),
        rel_path: "sub".into(),
        file: FolderFile::default(),
        requests: vec![],
        subfolders: vec![subfolder],
    };
    let tree = WorkspaceTree {
        root_requests: vec![req1],
        folders: vec![folder],
    };

    assert_eq!(tree.count_requests(), 2);
    assert_eq!(tree.count_folders(), 2);
}
