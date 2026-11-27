use super::*;

#[test]
fn test_request_lifecycle_and_orphan_loc_cleanup() {
    let temp_dir =
        std::env::temp_dir().join(format!("test_life_{}", uuid::Uuid::new_v4().simple()));
    let store = FsWorkspaceStore::new(&temp_dir);
    store.init_workspace("Test Life").unwrap();

    // 1. Create request
    let (slug, rel_path) = store
        .create_request(
            None,
            "Login",
            HttpMethod::Post,
            "http://api.com",
            Protocol::Http,
        )
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
        .create_request(
            None,
            "Large",
            HttpMethod::Get,
            "http://api.com",
            Protocol::Http,
        )
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
fn response_cache_truncates_at_a_valid_utf8_boundary() {
    const LIMIT: usize = 512 * 1024;
    let dir = tempfile::tempdir().unwrap();
    let store = FsWorkspaceStore::new(dir.path());
    store.init_workspace("UTF-8 cache").unwrap();
    let (_, id) = store
        .create_request(None, "Response", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    let original = format!("{}é", "x".repeat(LIMIT - 1));
    let cache = LocFile {
        sent_at: String::new(),
        request: LocRequestInfo {
            method: HttpMethod::Get,
            url: String::new(),
            headers: vec![],
            body: None,
        },
        response: LocResponseInfo {
            status: 200,
            duration_ms: 0,
            size: original.len(),
            headers: vec![],
            body: Some(original),
            truncated: false,
            binary: false,
        },
    };
    for cache in [LocFile::from_record(&cache.to_record(&id)), cache] {
        store.save_response_cache(&id, &cache).unwrap();
        let saved = store.load_response_cache(&id).unwrap().unwrap().response;
        assert!(saved.truncated);
        assert_eq!(saved.body.unwrap(), "x".repeat(LIMIT - 1));
    }
}
