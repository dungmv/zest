use crate::services::openapi_export::{
    build_openapi_export_for, openapi_export_stats, openapi_export_stats_for,
};
use crate::state::CollectionState;
use std::sync::Arc;
use zest_core::models::{Auth, Folder, HttpMethod, Protocol, RequestItem};
use zest_core::storage::FsWorkspaceStore;

/// Builds a minimal collection state (no GPUI context): a `users` folder containing an
/// `auth` subfolder, requests spread across the tree, and one WebSocket
/// request that the exporter must skip.
fn state_with_tree() -> CollectionState {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut state = CollectionState::new(
        Arc::new(FsWorkspaceStore::new(tmp.path())),
        tmp.path().to_path_buf(),
        "Workspace".to_string(),
    );
    state.folders = vec![
        Folder {
            id: "f-users".into(),
            workspace_id: "ws".into(),
            name: "users".into(),
            parent_id: None,
            sort_order: 0,
            headers: Vec::new(),
            auth: Auth::Inherit,
        },
        Folder {
            id: "f-auth".into(),
            workspace_id: "ws".into(),
            name: "auth".into(),
            parent_id: Some("f-users".into()),
            sort_order: 0,
            headers: Vec::new(),
            auth: Auth::Inherit,
        },
    ];
    state.requests = vec![
        {
            let mut r = RequestItem::new("ws", "List users", HttpMethod::Get, "/users");
            r.folder_id = Some("f-users".into());
            r
        },
        {
            let mut r = RequestItem::new("ws", "Login", HttpMethod::Post, "/auth/login");
            r.folder_id = Some("f-auth".into());
            r
        },
        RequestItem::new("ws", "Root ping", HttpMethod::Get, "/ping"),
        {
            let mut r = RequestItem::new_with_protocol(
                "ws",
                "Live feed",
                HttpMethod::Get,
                "/feed",
                Protocol::WebSocket,
            );
            r.folder_id = Some("f-auth".into());
            r
        },
    ];
    // Keep the tempdir alive for the whole test run by leaking it: the
    // store never touches disk during export (pure in-memory projection).
    std::mem::forget(tmp);
    state
}

#[test]
fn folder_export_includes_subtree_with_relative_tags() {
    let state = state_with_tree();
    let export = build_openapi_export_for(&state, Some("f-users"), "Workspace", None);
    let paths = export.document["paths"].as_object().expect("paths");
    // Subtree requests only: the workspace-root request is excluded.
    assert_eq!(paths.len(), 2);
    assert!(paths.contains_key("/users"));
    assert!(paths.contains_key("/auth/login"));
    // Tags are relative to the export root: direct child has none, deeper
    // requests carry the subfolder chain only.
    assert!(export.document["paths"]["/users"]["get"]["tags"].is_null());
    assert_eq!(
        export.document["paths"]["/auth/login"]["post"]["tags"],
        serde_json::json!(["auth"])
    );
    assert_eq!(export.document["info"]["title"], "users");
    // The WebSocket request is skipped with a warning, not exported.
    assert!(export.warnings.iter().any(|w| w.contains("Live feed")));
}

#[test]
fn folder_export_stats_count_subtree_only() {
    let state = state_with_tree();
    // (exportable, folders, skipped) inside the `users` subtree.
    assert_eq!(openapi_export_stats_for(&state, Some("f-users")), (2, 1, 1));
    // Whole-workspace stats are unchanged by folder scoping.
    assert_eq!(openapi_export_stats(&state), (3, 2, 1));
}
