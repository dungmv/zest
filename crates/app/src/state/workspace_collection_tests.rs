use super::*;

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

    let mut ws_state = CollectionState::new_for_path(temp_dir.clone(), store);
    ws_state.workspace_scope = WorkspaceScopeSnapshot {
        name: ws.name,
        headers: ws.headers,
        auth: ws.auth,
    };
    ws_state.folders = vec![folder_users, folder_admin];

    // Case 1: Request in subfolder (users/admin) with Inherit auth
    let mut req1 = RequestItem::new("ws", "Admin Req", HttpMethod::Get, "https://api.com/admin");
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
    let mut req2 = RequestItem::new("ws", "Users Req", HttpMethod::Get, "https://api.com/users");
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
    let mut req3 = RequestItem::new("ws", "Root Req", HttpMethod::Get, "https://api.com/root");
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
    let mut req4 = RequestItem::new(
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

    let mut ws_state = CollectionState::new_for_path(temp_dir.clone(), store);
    ws_state.workspace_scope = WorkspaceScopeSnapshot {
        name: ws.name,
        headers: ws.headers,
        auth: ws.auth,
    };
    ws_state.folders = vec![folder];

    let mut req = RequestItem::new("ws", "R", HttpMethod::Get, "https://api.com");
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
    ws_state.workspace_scope.auth = Auth::Inherit;
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

    let mut ws_state = CollectionState::new_for_path(temp_dir.clone(), store.clone());

    // 1. Create Request
    let req1 = ws_state.create_request_internal(None, Protocol::Http);
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
    ws_state.delete_folder_internal(&new_folder_id).unwrap();
    assert_eq!(ws_state.folders.len(), 0);
    assert_eq!(ws_state.requests.len(), 1);
    assert_eq!(ws_state.requests[0].id, new_id);

    // 8. Delete Request
    ws_state.delete_request_internal(&new_id).unwrap();
    assert_eq!(ws_state.requests.len(), 0);

    // Verify disk matches: scan_tree returns empty workspace
    let disk_tree = store.scan_tree().unwrap();
    assert_eq!(disk_tree.root_requests.len(), 0);
    assert_eq!(disk_tree.folders.len(), 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn failed_request_crud_preserves_tree_and_pins() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(FsWorkspaceStore::new(dir.path()));
    store.init_workspace("CRUD errors").unwrap();
    let (_, folder) = store.create_group(None, "Folder").unwrap();
    let (_, id) = store
        .create_request(
            Some(&folder),
            "Request",
            HttpMethod::Get,
            "",
            Protocol::Http,
        )
        .unwrap();
    let mut collection = CollectionState::new_for_path(dir.path().to_path_buf(), store.clone());
    collection.pins.push(id.clone());

    let request_path = store.collections_dir().join(&id);
    std::fs::remove_file(&request_path).unwrap();
    std::fs::create_dir(&request_path).unwrap();
    assert!(collection.delete_request_internal(&id).is_err());

    std::fs::remove_dir_all(store.collections_dir().join(&folder)).unwrap();
    std::fs::write(
        store.collections_dir().join(&folder),
        b"file instead of directory",
    )
    .unwrap();

    assert!(collection.duplicate_request_internal(&id).is_err());
    assert!(collection.find_request(&id).is_some());
    assert_eq!(collection.requests.len(), 1);
    assert_eq!(collection.pins, [id]);
}
