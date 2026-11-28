use crate::state::ResponseViewMode;
use crate::state::{
    CollectionEvent, CollectionState, ComposerSyncScope, EnvironmentEvent, EnvironmentState,
    RequestSessionState, ResponseEvent, ResponseState, SessionEvent,
};
use gpui_kit::{AppContext, TestAppContext};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use zest_core::models::{HttpResponseRecord, Protocol};
use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

struct Listener;

fn test_collection(tag: &str) -> (PathBuf, Arc<FsWorkspaceStore>, CollectionState) {
    let dir = std::env::temp_dir().join(format!(
        "zest-events-{}-{}",
        tag,
        uuid::Uuid::new_v4().simple()
    ));
    let store = Arc::new(FsWorkspaceStore::new(&dir));
    store.init_workspace("test-ws").expect("init workspace");
    let col = CollectionState::new_for_path(dir.clone(), store.clone());
    (dir, store, col)
}

#[gpui_kit::test]
fn test_rename_request_emits_single_ids_remapped_event(cx: &mut TestAppContext) {
    let (dir, _store, mut col_raw) = test_collection("rename-req");
    let req = col_raw.create_request_internal(None, Protocol::Http);
    let req_id = req.id.clone();
    let col = cx.new(|_| col_raw);

    let recorded = Rc::new(RefCell::new(Vec::new()));
    let recorded_cb = recorded.clone();
    let _listener = cx.new(|cx| {
        cx.subscribe(&col, move |_this, _col, ev: &CollectionEvent, _cx| {
            recorded_cb.borrow_mut().push(ev.clone());
        })
        .detach();
        Listener
    });

    col.update(cx, |col, cx| col.rename_request(&req_id, "Accounts", cx));
    cx.run_until_parked();
    let new_id = col.read_with(cx, |col, _| col.requests()[0].id.clone());

    let events = recorded.borrow();
    assert_eq!(
        events.as_slice(),
        &[CollectionEvent::IdsRemapped(vec![(req_id, new_id)])]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui_kit::test]
fn test_move_folder_cycle_emits_crud_failed_event(cx: &mut TestAppContext) {
    let (dir, _store, mut col_raw) = test_collection("move-cycle");
    let parent_id = col_raw
        .create_folder_internal(None, "Parent".to_string())
        .expect("create parent");
    let child_id = col_raw
        .create_folder_internal(Some(parent_id.clone()), "Child".to_string())
        .expect("create child");
    let col = cx.new(|_| col_raw);

    let recorded = Rc::new(RefCell::new(Vec::new()));
    let recorded_cb = recorded.clone();
    let _listener = cx.new(|cx| {
        cx.subscribe(&col, move |_this, _col, ev: &CollectionEvent, _cx| {
            recorded_cb.borrow_mut().push(ev.clone());
        })
        .detach();
        Listener
    });

    col.update(cx, |col, cx| {
        col.move_folder(&parent_id, Some(child_id), cx);
    });

    let events = recorded.borrow();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        CollectionEvent::CrudFailed {
            title: "Cannot rename or move",
            ..
        }
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui_kit::test]
fn test_session_events_on_switch_and_repeated_modify(cx: &mut TestAppContext) {
    let session = cx.new(|_| RequestSessionState::new());
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let recorded_cb = recorded.clone();

    let _listener = cx.new(|cx| {
        cx.subscribe(&session, move |_this, _ed, ev: &SessionEvent, _cx| {
            recorded_cb.borrow_mut().push(ev.clone());
        })
        .detach();
        Listener
    });

    session.update(cx, |ed, cx| {
        ed.set_active_request_id("req-1", cx);
        // Switching to the same ID is a no-op
        ed.set_active_request_id("req-1", cx);
        // Repeated mark_dirty calls MUST each emit RequestModified (no dropped dirty events)
        ed.mark_dirty(cx);
        ed.mark_dirty(cx);
        ed.notify_body_changed(cx);
        ed.sync_composer_inputs(ComposerSyncScope::All, cx);
    });

    let events = recorded.borrow();
    assert_eq!(
        events.as_slice(),
        &[
            SessionEvent::ActiveRequestChanged {
                req_id: "req-1".to_string(),
            },
            SessionEvent::RequestModified {
                req_id: "req-1".to_string(),
                sync_inputs: None,
            },
            SessionEvent::RequestModified {
                req_id: "req-1".to_string(),
                sync_inputs: None,
            },
            SessionEvent::RequestModified {
                req_id: "req-1".to_string(),
                sync_inputs: Some(ComposerSyncScope::BodyOnly),
            },
            SessionEvent::SyncComposerInputs {
                scope: ComposerSyncScope::All,
            },
        ]
    );
}

#[gpui_kit::test]
fn test_response_content_changed_emits_only_on_content_or_view_mode(cx: &mut TestAppContext) {
    let resp_state = cx.new(|_| ResponseState::new());
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let recorded_cb = recorded.clone();

    let _listener = cx.new(|cx| {
        cx.subscribe(&resp_state, move |_this, _r, ev: &ResponseEvent, _cx| {
            recorded_cb.borrow_mut().push(*ev);
        })
        .detach();
        Listener
    });

    resp_state.update(cx, |r, cx| {
        // UI-only tab switch does not emit ContentChanged
        r.set_active_tab(crate::state::ResponseTabKind::Headers, cx);
        // View mode change emits ContentChanged { sync_sent_preview: false }
        r.set_view_mode(ResponseViewMode::Raw, cx);
        // Setting same view mode is a no-op
        r.set_view_mode(ResponseViewMode::Raw, cx);
        // Setting a new response record emits ContentChanged { sync_sent_preview: true }
        let dummy = Arc::new(HttpResponseRecord {
            id: "resp-1".to_string(),
            request_id: "req-1".to_string(),
            status_code: 200,
            status_text: "OK".to_string(),
            body: b"{}".to_vec(),
            ..Default::default()
        });
        r.set_response(Some(dummy), cx);
    });

    let events = recorded.borrow();
    assert_eq!(
        events.as_slice(),
        &[
            ResponseEvent::ContentChanged {
                sync_sent_preview: false
            },
            ResponseEvent::ContentChanged {
                sync_sent_preview: true
            },
        ]
    );
}

#[gpui_kit::test]
fn test_environment_active_change_emits_event(cx: &mut TestAppContext) {
    let (dir, store, _col_raw) = test_collection("env-ev");
    let env_state = cx.new(|_| EnvironmentState::new(store, "test-ws".to_string(), None));
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let recorded_cb = recorded.clone();

    let _listener = cx.new(|cx| {
        cx.subscribe(&env_state, move |_this, _e, ev: &EnvironmentEvent, _cx| {
            recorded_cb.borrow_mut().push(ev.clone());
        })
        .detach();
        Listener
    });

    env_state.update(cx, |env, cx| {
        env.create_environment(Some("Staging".to_string()), cx);
    });
    cx.run_until_parked();
    env_state.update(cx, |env, cx| {
        env.set_active_environment(Some("Staging".to_string()), cx);
        // Setting same active environment does not re-emit
        env.set_active_environment(Some("Staging".to_string()), cx);
        env.rename_environment("Staging", "Production".to_string(), cx);
    });

    let events = recorded.borrow();
    assert_eq!(
        events.as_slice(),
        &[
            EnvironmentEvent::EnvironmentsReloaded,
            EnvironmentEvent::ActiveEnvironmentChanged(Some("Staging".to_string())),
            EnvironmentEvent::RenameRequested {
                id: "Staging".to_string(),
                name: "Production".to_string(),
            },
        ]
    );
    let _ = std::fs::remove_dir_all(dir);
}
