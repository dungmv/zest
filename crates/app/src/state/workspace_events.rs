//! Typed commands/events emitted by state entities (`WorkspaceDataState`,
//! `RequestSessionState`, `ResponseState`, `EnvironmentState`) so downstream
//! states and UI editors receive change payloads in a single synchronous dispatch
//! instead of polling intermediate state fields on `cx.notify()`.

use super::{EnvironmentState, RequestSessionState, ResponseState, WorkspaceDataState};
use gpui::EventEmitter;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::storage::WorkspaceStore;

/// Events emitted by `WorkspaceDataState`.
#[derive(Clone)]
pub enum WorkspaceEvent {
    /// A workspace directory was opened or initialized.
    WorkspaceOpened {
        path: PathBuf,
        workspace_name: String,
        store: Arc<dyn WorkspaceStore>,
    },
    /// Request/folder ids changed on disk (request rename, folder rename/move,
    /// cross-folder request move). `(old_id, new_id)` pairs.
    IdsRemapped(Vec<(String, String)>),
    /// Request items were reloaded from disk (e.g. external fs watcher, conflict reload, import).
    RequestsReloaded {
        /// `None` means the entire tree was reloaded; `Some(ids)` lists specific request IDs.
        req_ids: Option<Vec<String>>,
    },
    /// A saved example of `req_id` was renamed.
    ExampleRenamed {
        req_id: String,
        old_name: String,
        new_name: String,
    },
    /// A user-initiated CRUD operation was rejected; surfaced as an error toast.
    CrudFailed {
        title: &'static str,
        message: String,
    },
}

impl fmt::Debug for WorkspaceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceOpened {
                path,
                workspace_name,
                ..
            } => f
                .debug_struct("WorkspaceOpened")
                .field("path", path)
                .field("workspace_name", workspace_name)
                .finish(),
            Self::IdsRemapped(pairs) => f.debug_tuple("IdsRemapped").field(pairs).finish(),
            Self::RequestsReloaded { req_ids } => f
                .debug_struct("RequestsReloaded")
                .field("req_ids", req_ids)
                .finish(),
            Self::ExampleRenamed {
                req_id,
                old_name,
                new_name,
            } => f
                .debug_struct("ExampleRenamed")
                .field("req_id", req_id)
                .field("old_name", old_name)
                .field("new_name", new_name)
                .finish(),
            Self::CrudFailed { title, message } => f
                .debug_struct("CrudFailed")
                .field("title", title)
                .field("message", message)
                .finish(),
        }
    }
}

impl PartialEq for WorkspaceEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::WorkspaceOpened {
                    path: p1,
                    workspace_name: n1,
                    ..
                },
                Self::WorkspaceOpened {
                    path: p2,
                    workspace_name: n2,
                    ..
                },
            ) => p1 == p2 && n1 == n2,
            (Self::IdsRemapped(a), Self::IdsRemapped(b)) => a == b,
            (Self::RequestsReloaded { req_ids: a }, Self::RequestsReloaded { req_ids: b }) => {
                a == b
            }
            (
                Self::ExampleRenamed {
                    req_id: r1,
                    old_name: o1,
                    new_name: n1,
                },
                Self::ExampleRenamed {
                    req_id: r2,
                    old_name: o2,
                    new_name: n2,
                },
            ) => r1 == r2 && o1 == o2 && n1 == n2,
            (
                Self::CrudFailed {
                    title: t1,
                    message: m1,
                },
                Self::CrudFailed {
                    title: t2,
                    message: m2,
                },
            ) => t1 == t2 && m1 == m2,
            _ => false,
        }
    }
}

impl Eq for WorkspaceEvent {}

impl EventEmitter<WorkspaceEvent> for WorkspaceDataState {}

/// Outcome of a folder move, returned by `move_folder_internal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderMoveOutcome {
    /// Nothing changed (unknown source, same-parent drop with no target).
    Unchanged,
    /// Same-parent drop: in-memory sibling reorder only.
    Reordered,
    /// Moved on disk; `(old_id, new_id)` pairs of the folder and descendants.
    Moved(Vec<(String, String)>),
}

/// Scope of composer text inputs to synchronize from the active request model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerSyncScope {
    /// Only reload body editors (JSON, XML, Raw).
    BodyOnly,
    /// Reload URL, Auth, Pre-request/Test scripts, and Body editors.
    All,
}

/// Events emitted by `RequestSessionState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// User switched the active request pointer.
    ActiveRequestChanged { req_id: String },
    /// Active request was mutated and requires debounced auto-save.
    /// Optionally synchronizes composer input editors if the mutation happened outside the input.
    RequestModified {
        req_id: String,
        sync_inputs: Option<ComposerSyncScope>,
    },
    /// Explicitly synchronize composer inputs from the in-memory request model without marking dirty.
    SyncComposerInputs { scope: ComposerSyncScope },
}

impl EventEmitter<SessionEvent> for RequestSessionState {}

/// Events emitted by `ResponseState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseEvent {
    /// Displayed response content or view mode changed; inspector editors must update.
    ContentChanged {
        /// `true` when the response/example record itself changed; `false` when only view mode changed.
        sync_sent_preview: bool,
    },
}

impl EventEmitter<ResponseEvent> for ResponseState {}

/// Events emitted by `EnvironmentState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentEvent {
    /// Active environment selection changed.
    ActiveEnvironmentChanged(Option<String>),
}

impl EventEmitter<EnvironmentEvent> for EnvironmentState {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ResponseViewMode;
    use gpui::{AppContext, TestAppContext};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
    use zest_core::models::HttpResponseRecord;
    use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

    struct Listener;

    fn test_workspace(tag: &str) -> (PathBuf, Arc<FsWorkspaceStore>, WorkspaceDataState) {
        let dir = std::env::temp_dir().join(format!(
            "zest-events-{}-{}",
            tag,
            uuid::Uuid::new_v4().simple()
        ));
        let store = Arc::new(FsWorkspaceStore::new(&dir));
        store.init_workspace("test-ws").expect("init workspace");
        let ws = WorkspaceDataState::new_for_path(dir.clone(), store.clone());
        (dir, store, ws)
    }

    #[gpui::test]
    fn test_rename_request_emits_single_ids_remapped_event(cx: &mut TestAppContext) {
        let (dir, _store, mut ws_raw) = test_workspace("rename-req");
        let req = ws_raw.create_request_internal(None);
        let req_id = req.id.clone();
        let ws = cx.new(|_| ws_raw);

        let recorded = Rc::new(RefCell::new(Vec::new()));
        let recorded_cb = recorded.clone();
        let _listener = cx.new(|cx| {
            cx.subscribe(&ws, move |_this, _ws, ev: &WorkspaceEvent, _cx| {
                recorded_cb.borrow_mut().push(ev.clone());
            })
            .detach();
            Listener
        });

        let new_id = ws
            .update(cx, |ws, cx| ws.rename_request(&req_id, "Accounts", cx))
            .expect("rename succeeds");

        let events = recorded.borrow();
        assert_eq!(
            events.as_slice(),
            &[WorkspaceEvent::IdsRemapped(vec![(req_id, new_id)])]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn test_move_folder_cycle_emits_crud_failed_event(cx: &mut TestAppContext) {
        let (dir, _store, mut ws_raw) = test_workspace("move-cycle");
        let parent_id = ws_raw
            .create_folder_internal(None, "Parent".to_string())
            .expect("create parent");
        let child_id = ws_raw
            .create_folder_internal(Some(parent_id.clone()), "Child".to_string())
            .expect("create child");
        let ws = cx.new(|_| ws_raw);

        let recorded = Rc::new(RefCell::new(Vec::new()));
        let recorded_cb = recorded.clone();
        let _listener = cx.new(|cx| {
            cx.subscribe(&ws, move |_this, _ws, ev: &WorkspaceEvent, _cx| {
                recorded_cb.borrow_mut().push(ev.clone());
            })
            .detach();
            Listener
        });

        ws.update(cx, |ws, cx| {
            ws.move_folder(&parent_id, Some(child_id), cx);
        });

        let events = recorded.borrow();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            WorkspaceEvent::CrudFailed {
                title: "Cannot rename or move",
                ..
            }
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
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

    #[gpui::test]
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

    #[gpui::test]
    fn test_environment_active_change_emits_event(cx: &mut TestAppContext) {
        let (dir, store, _ws_raw) = test_workspace("env-ev");
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
            env.set_active_environment(Some("Staging".to_string()), cx);
            // Setting same active environment does not re-emit
            env.set_active_environment(Some("Staging".to_string()), cx);
            env.rename_environment("Staging", "Production".to_string(), cx);
        });

        let events = recorded.borrow();
        assert_eq!(
            events.as_slice(),
            &[
                EnvironmentEvent::ActiveEnvironmentChanged(Some("Staging".to_string())),
                EnvironmentEvent::ActiveEnvironmentChanged(Some("Production".to_string())),
            ]
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
