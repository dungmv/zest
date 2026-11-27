use super::*;

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
        .create_request(
            None,
            "Login",
            HttpMethod::Post,
            "http://a.com",
            Protocol::Http,
        )
        .unwrap();
    let (_, login2_rel) = store
        .create_request(
            None,
            "Login 2",
            HttpMethod::Get,
            "http://a.com",
            Protocol::Http,
        )
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
        .create_request(
            None,
            "Login",
            HttpMethod::Get,
            "http://a.com",
            Protocol::Http,
        )
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
            Protocol::Http,
        )
        .unwrap();
    store
        .create_request(
            Some(&src_folder),
            "Other",
            HttpMethod::Get,
            "http://api.com",
            Protocol::Http,
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
