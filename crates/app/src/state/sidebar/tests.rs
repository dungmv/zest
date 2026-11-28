use super::SidebarState;
use crate::state::CollectionState;
use std::sync::Arc;
use zest_core::models::Protocol;
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

#[test]
fn test_sidebar_state_navigation() {
    let mut sidebar = SidebarState::new();
    assert!(sidebar.navigation.history.is_empty());

    sidebar.record_navigation("req-1");
    sidebar.record_navigation("req-2");
    sidebar.record_navigation("req-3");
    assert_eq!(sidebar.navigation.history.len(), 3);

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
    cx.run_until_parked();

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
