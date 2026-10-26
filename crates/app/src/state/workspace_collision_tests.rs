//! Tests for duplicate-name handling on rename/move (duplicate display names
//! are allowed; the on-disk slug gets an automatic suffix) and for the folder
//! drag-drop persistence (move_group wiring + id remapping).

#[cfg(test)]
mod tests {
    use crate::state::{FolderMoveOutcome, WorkspaceDataState};
    use std::path::PathBuf;
    use std::sync::Arc;
    use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

    fn setup_ws(tag: &str) -> (PathBuf, Arc<FsWorkspaceStore>, WorkspaceDataState) {
        let temp_dir = std::env::temp_dir().join(format!(
            "zest-collision-{}-{}",
            tag,
            uuid::Uuid::new_v4().simple()
        ));
        let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
        store.init_workspace("Collision WS").unwrap();
        let ws = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
        (temp_dir, store, ws)
    }

    #[test]
    fn test_rename_request_duplicate_slug_suffixes() {
        let (temp_dir, store, mut ws) = setup_ws("rename-req");
        let a = ws.create_request_internal(None);
        let a_id = ws
            .rename_request_internal(&a.id, "Login")
            .expect("first rename");
        assert_eq!(a_id, "login.yaml");

        // Duplicate display names are accepted: the file slug is suffixed
        // while the display name stays exactly as typed.
        let b = ws.create_request_internal(None);
        assert_eq!(
            ws.rename_request_internal(&b.id, "Login"),
            Some("login-2.yaml".to_string())
        );
        let b_item = ws.requests.iter().find(|r| r.id == "login-2.yaml").unwrap();
        assert_eq!(b_item.name, "Login");
        assert!(store.collections_dir().join("login.yaml").exists());
        assert!(store.collections_dir().join("login-2.yaml").exists());

        // Renaming the suffixed item to a name whose slug is taken by the
        // sibling keeps its own stem (self-exclusion) instead of jumping to
        // login-3 — the display name still updates in place.
        assert_eq!(
            ws.rename_request_internal("login-2.yaml", "login"),
            Some("login-2.yaml".to_string())
        );
        assert_eq!(
            ws.requests
                .iter()
                .find(|r| r.id == "login-2.yaml")
                .unwrap()
                .name,
            "login"
        );
        assert!(!store.collections_dir().join("login-3.yaml").exists());

        // A non-conflicting rename still works and never trips an error.
        assert_eq!(
            ws.rename_request_internal("login-2.yaml", "Logout"),
            Some("logout.yaml".to_string())
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_folder_duplicate_dir_suffixes() {
        let (temp_dir, store, mut ws) = setup_ws("rename-folder");
        ws.create_folder_internal(None, "Auth".to_string()).unwrap();
        let other = ws
            .create_folder_internal(None, "Other".to_string())
            .unwrap();

        let rename_res = ws.rename_folder_internal(&other, "Auth");
        assert_eq!(rename_res.as_ref().map(|(p, _)| p.as_str()), Some("auth-2"));
        let renamed = ws.folders.iter().find(|f| f.id == "auth-2").unwrap();
        assert_eq!(renamed.name, "Auth");
        assert!(store.collections_dir().join("auth-2/_folder.yaml").exists());

        // Non-conflicting rename keeps working.
        let rename_res2 = ws.rename_folder_internal("auth-2", "Other Stuff");
        assert_eq!(
            rename_res2.as_ref().map(|(p, _)| p.as_str()),
            Some("other-stuff")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_request_duplicate_suffixes_and_own_folder_noop() {
        let (temp_dir, store, mut ws) = setup_ws("move-req");
        let fa = ws.create_folder_internal(None, "A".to_string()).unwrap();
        let fb = ws.create_folder_internal(None, "B".to_string()).unwrap();

        let ra = ws.create_request_internal(Some(fa.clone()));
        let ra_id = ws
            .rename_request_internal(&ra.id, "Login")
            .expect("rename in A");
        let rb = ws.create_request_internal(Some(fb.clone()));
        let rb_id = ws
            .rename_request_internal(&rb.id, "Login")
            .expect("rename in B");

        // Moving onto a same-named sibling is allowed: the file is suffixed in
        // the destination while the display name stays as typed.
        assert_eq!(
            ws.move_request_to_folder_internal(&ra_id, Some(fb.clone())),
            Some("b/login-2.yaml".to_string())
        );
        assert!(!store.collections_dir().join("a/login.yaml").exists());
        assert!(store.collections_dir().join("b/login.yaml").exists());
        assert!(store.collections_dir().join("b/login-2.yaml").exists());
        let moved = ws
            .requests
            .iter()
            .find(|r| r.id == "b/login-2.yaml")
            .unwrap();
        assert_eq!(moved.name, "Login");

        // Dropping onto the request's own folder is a no-op, never a further
        // suffixed duplicate file.
        assert!(ws
            .move_request_to_folder_internal("b/login-2.yaml", Some(fb.clone()))
            .is_none());
        assert!(!store.collections_dir().join("b/login-3.yaml").exists());

        // A clean move to root still succeeds and records the id pair.
        assert_eq!(
            ws.move_request_to_folder_internal(&rb_id, None),
            Some("login.yaml".to_string())
        );
        assert!(store.collections_dir().join("login.yaml").exists());
        assert!(!store.collections_dir().join("b/login.yaml").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_folder_persists_and_remaps_ids() {
        let (temp_dir, store, mut ws) = setup_ws("move-folder");
        ws.create_folder_internal(None, "Api".to_string()).unwrap();
        ws.create_folder_internal(None, "Users".to_string())
            .unwrap();
        ws.create_folder_internal(Some("users".to_string()), "Admin".to_string())
            .unwrap();

        let req = ws.create_request_internal(Some("users/admin".to_string()));
        ws.rename_request_internal(&req.id, "Login")
            .expect("rename");
        let login_id = "users/admin/login.yaml";

        // Pin the request so the remapping of pins is exercised too.
        ws.pins = vec![login_id.to_string()];

        let move_res = ws.move_folder_internal("users", Some("api".to_string()));
        let moved_pairs = match move_res {
            Ok(FolderMoveOutcome::Moved(pairs)) => pairs,
            other => panic!("expected Moved, got: {:?}", other),
        };

        // Disk layout actually moved.
        assert!(store
            .collections_dir()
            .join("api/users/_folder.yaml")
            .exists());
        assert!(store
            .collections_dir()
            .join("api/users/admin/login.yaml")
            .exists());
        assert!(!store.collections_dir().join("users").exists());

        // In-memory ids, parents and pins follow the new path.
        let moved = ws
            .folders
            .iter()
            .find(|f| f.id == "api/users")
            .expect("moved folder id remapped");
        assert_eq!(moved.parent_id.as_deref(), Some("api"));
        assert!(ws
            .folders
            .iter()
            .any(|f| { f.id == "api/users/admin" && f.parent_id.as_deref() == Some("api/users") }));
        let req = ws
            .requests
            .iter()
            .find(|r| r.id == login_id.replace("users/", "api/users/"))
            .expect("request id remapped");
        assert_eq!(req.folder_id.as_deref(), Some("api/users/admin"));
        assert_eq!(ws.pins, vec!["api/users/admin/login.yaml".to_string()]);

        // (old, new) pairs are recorded for downstream state propagation.
        assert!(moved_pairs.contains(&("users".to_string(), "api/users".to_string())));
        assert!(moved_pairs.contains(&(
            "users/admin/login.yaml".to_string(),
            "api/users/admin/login.yaml".to_string()
        )));

        // A fresh scan of disk agrees with the in-memory state.
        let reloaded = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
        assert!(reloaded
            .folders
            .iter()
            .any(|f| { f.id == "api/users" && f.parent_id.as_deref() == Some("api") }));
        assert!(reloaded
            .requests
            .iter()
            .any(|r| r.id == "api/users/admin/login.yaml"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_folder_into_descendant_blocked() {
        let (temp_dir, store, mut ws) = setup_ws("move-cycle");
        ws.create_folder_internal(None, "Parent".to_string())
            .unwrap();
        ws.create_folder_internal(Some("parent".to_string()), "Child".to_string())
            .unwrap();

        let err = ws
            .move_folder_internal("parent", Some("parent/child".to_string()))
            .expect_err("error must be reported");
        assert!(err.contains("into itself"), "got: {}", err);
        assert!(store
            .collections_dir()
            .join("parent/child/_folder.yaml")
            .exists());
        assert!(!store.collections_dir().join("parent/child/parent").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_folder_duplicate_dir_suffixes() {
        let (temp_dir, store, mut ws) = setup_ws("move-folder-dup");
        ws.create_folder_internal(None, "A".to_string()).unwrap();
        ws.create_folder_internal(None, "B".to_string()).unwrap();
        ws.create_folder_internal(Some("a".to_string()), "Shared".to_string())
            .unwrap();
        ws.create_folder_internal(Some("b".to_string()), "Shared".to_string())
            .unwrap();

        let move_res = ws.move_folder_internal("a/shared", Some("b".to_string()));
        let moved_pairs = match move_res {
            Ok(FolderMoveOutcome::Moved(pairs)) => pairs,
            other => panic!("expected Moved, got: {:?}", other),
        };

        // The moved directory is suffixed in the destination; both display
        // names stay "Shared".
        assert!(!store.collections_dir().join("a").join("shared").exists());
        assert!(store
            .collections_dir()
            .join("b/shared/_folder.yaml")
            .exists());
        assert!(store
            .collections_dir()
            .join("b/shared-2/_folder.yaml")
            .exists());
        let moved = ws.folders.iter().find(|f| f.id == "b/shared-2").unwrap();
        assert_eq!(moved.name, "Shared");
        assert_eq!(moved.parent_id.as_deref(), Some("b"));
        assert_eq!(
            moved_pairs,
            vec![("a/shared".to_string(), "b/shared-2".to_string())]
        );

        // A fresh scan of disk agrees with the in-memory state.
        let reloaded = WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone());
        assert!(reloaded.folders.iter().any(|f| {
            f.id == "b/shared-2" && f.parent_id.as_deref() == Some("b") && f.name == "Shared"
        }));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_flush_before_rename_preserves_pending_edits() {
        let (temp_dir, store, mut ws) = setup_ws("flush-rename");
        let a = ws.create_request_internal(None);
        ws.rename_request_internal(&a.id, "Login").expect("rename");

        // Simulate an edit still waiting inside the 500 ms save debounce.
        ws.mutate_request("login.yaml", |r| {
            r.url = "https://edited.example.com".to_string();
        });

        let new_id = ws
            .rename_request_internal("login.yaml", "Login User")
            .expect("rename succeeds");
        assert_eq!(new_id, "login-user.yaml");

        // The renamed file must carry the pending edit, not the stale disk copy.
        let file = store.read_request(&new_id).expect("renamed file readable");
        assert_eq!(file.url, "https://edited.example.com");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
