//! Typed commands/events emitted by state entities (`WorkspaceState`,
//! `CollectionState`, `RequestSessionState`, `ResponseState`, `EnvironmentState`)
//! so downstream states and UI editors receive change payloads in a single
//! synchronous dispatch instead of polling intermediate state fields on `cx.notify()`.

use super::{
    CollectionState, EnvironmentSnapshot, EnvironmentState, ModalState, RequestSessionState,
    ResponseState, WorkspaceState,
};
use gpui_kit::EventEmitter;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::models::StoredCookie;
use zest_core::storage::models::{LocalState, WorkspaceMeta};
use zest_core::storage::WorkspaceStore;

/// Events emitted by `WorkspaceState`.
#[derive(Clone)]
pub enum WorkspaceEvent {
    /// User initiated opening or creating a workspace; `CollectionState` should
    /// switch its store and run an async background scan.
    WorkspaceSwitchRequested {
        path: PathBuf,
        store: Arc<dyn WorkspaceStore>,
        folder_name: String,
    },
    /// Workspace-level metadata, headers, or auth were modified.
    WorkspaceMetaModified,
}

impl fmt::Debug for WorkspaceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceSwitchRequested {
                path, folder_name, ..
            } => f
                .debug_struct("WorkspaceSwitchRequested")
                .field("path", path)
                .field("folder_name", folder_name)
                .finish(),
            Self::WorkspaceMetaModified => write!(f, "WorkspaceMetaModified"),
        }
    }
}

impl PartialEq for WorkspaceEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::WorkspaceSwitchRequested {
                    path: p1,
                    folder_name: n1,
                    ..
                },
                Self::WorkspaceSwitchRequested {
                    path: p2,
                    folder_name: n2,
                    ..
                },
            ) => p1 == p2 && n1 == n2,
            (Self::WorkspaceMetaModified, Self::WorkspaceMetaModified) => true,
            _ => false,
        }
    }
}

impl Eq for WorkspaceEvent {}

impl EventEmitter<WorkspaceEvent> for WorkspaceState {}

/// Events emitted by `CollectionState`.
#[derive(Clone)]
pub enum CollectionEvent {
    /// A background workspace scan completed; carries workspace metadata and
    /// cookies so `WorkspaceState` hydrates from the same single disk pass.
    WorkspaceScanLoaded {
        workspace_meta: Option<WorkspaceMeta>,
        local_state: Box<LocalState>,
        environment: Box<EnvironmentSnapshot>,
        cookies: Vec<StoredCookie>,
        folder_name: String,
        store: Arc<dyn WorkspaceStore>,
        announce_opened: bool,
        workspace_path: PathBuf,
    },
    WorkspaceMetaReloaded {
        meta: Box<WorkspaceMeta>,
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
    ImportApplied(Vec<String>),
    RequestDeleted(String),
    RequestContentModified(String),
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
    /// Folder-level metadata, headers, auth, or order were modified.
    FolderMetaModified(String),
    /// Request pin list was modified.
    PinsModified,
}

impl fmt::Debug for CollectionEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceScanLoaded {
                folder_name,
                announce_opened,
                workspace_path,
                ..
            } => f
                .debug_struct("WorkspaceScanLoaded")
                .field("folder_name", folder_name)
                .field("announce_opened", announce_opened)
                .field("workspace_path", workspace_path)
                .finish(),
            Self::WorkspaceMetaReloaded { .. } => write!(f, "WorkspaceMetaReloaded"),
            Self::IdsRemapped(pairs) => f.debug_tuple("IdsRemapped").field(pairs).finish(),
            Self::RequestsReloaded { req_ids } => f
                .debug_struct("RequestsReloaded")
                .field("req_ids", req_ids)
                .finish(),
            Self::ImportApplied(names) => f.debug_tuple("ImportApplied").field(names).finish(),
            Self::RequestDeleted(id) => f.debug_tuple("RequestDeleted").field(id).finish(),
            Self::RequestContentModified(id) => {
                f.debug_tuple("RequestContentModified").field(id).finish()
            }
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
            Self::FolderMetaModified(folder_rel) => f
                .debug_tuple("FolderMetaModified")
                .field(folder_rel)
                .finish(),
            Self::PinsModified => write!(f, "PinsModified"),
        }
    }
}

impl PartialEq for CollectionEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::WorkspaceScanLoaded {
                    folder_name: f1,
                    announce_opened: a1,
                    workspace_path: p1,
                    ..
                },
                Self::WorkspaceScanLoaded {
                    folder_name: f2,
                    announce_opened: a2,
                    workspace_path: p2,
                    ..
                },
            ) => f1 == f2 && a1 == a2 && p1 == p2,
            (
                Self::WorkspaceMetaReloaded { meta: a, store: sa },
                Self::WorkspaceMetaReloaded { meta: b, store: sb },
            ) => a == b && Arc::ptr_eq(sa, sb),
            (Self::IdsRemapped(a), Self::IdsRemapped(b)) => a == b,
            (Self::ImportApplied(a), Self::ImportApplied(b)) => a == b,
            (Self::RequestDeleted(a), Self::RequestDeleted(b))
            | (Self::RequestContentModified(a), Self::RequestContentModified(b)) => a == b,
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
            (Self::FolderMetaModified(a), Self::FolderMetaModified(b)) => a == b,
            (Self::PinsModified, Self::PinsModified) => true,
            _ => false,
        }
    }
}

impl Eq for CollectionEvent {}

impl EventEmitter<CollectionEvent> for CollectionState {}

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
    /// Environment variables or metadata were modified.
    EnvironmentModified(String),
    /// Globals variables were modified.
    GlobalsModified,
    /// Environments were reloaded or externally/programmatically updated; UI inputs must re-sync.
    EnvironmentsReloaded,
    RenameRequested {
        id: String,
        name: String,
    },
    DeleteRequested(String),
    EnvironmentRenamed {
        old_id: String,
        new_id: String,
    },
}

impl EventEmitter<EnvironmentEvent> for EnvironmentState {}

/// Events emitted by `ModalState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalEvent {
    /// Environment modal opened or its selected environment changed.
    EnvModalSelectionChanged,
    /// Active light/dark theme mode changed (either by user preference or OS appearance).
    ThemeChanged,
}

impl EventEmitter<ModalEvent> for ModalState {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ResponseViewMode;
    use gpui_kit::{AppContext, TestAppContext};
    use std::cell::RefCell;
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

        let new_id = col
            .update(cx, |col, cx| col.rename_request(&req_id, "Accounts", cx))
            .expect("rename succeeds");

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
}
