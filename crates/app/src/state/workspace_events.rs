//! Typed events emitted by `WorkspaceDataState` so downstream states receive
//! change payloads in a single dispatch instead of polling `last_*` fields.

use super::WorkspaceDataState;
use gpui::EventEmitter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    /// Request/folder ids changed on disk (request rename, folder rename/move,
    /// cross-folder request move). `(old_id, new_id)` pairs.
    IdsRemapped(Vec<(String, String)>),
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
    use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

    fn setup_ws(tag: &str) -> (std::path::PathBuf, Arc<FsWorkspaceStore>) {
        let temp_dir = std::env::temp_dir().join(format!(
            "zest-events-{}-{}",
            tag,
            uuid::Uuid::new_v4().simple()
        ));
        let store = Arc::new(FsWorkspaceStore::new(&temp_dir));
        store.init_workspace("Events WS").unwrap();
        (temp_dir, store)
    }

    #[gpui::test]
    fn test_rename_request_emits_single_ids_remapped_event(cx: &mut TestAppContext) {
        let (temp_dir, store) = setup_ws("rename-event");

        let events = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        let ws_entity =
            cx.new(|_cx| WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone()));

        let _sub = cx.update(|cx| {
            cx.subscribe(&ws_entity, move |_entity, event: &WorkspaceEvent, _cx| {
                events_clone.borrow_mut().push(event.clone());
            })
        });

        // 1. Create a request
        let req = ws_entity.update(cx, |ws, cx| ws.create_request(None, cx));
        assert!(events.borrow().is_empty());

        // 2. Rename request: should emit exactly one IdsRemapped event
        let new_id = ws_entity
            .update(cx, |ws, cx| ws.rename_request(&req.id, "Login", cx))
            .expect("rename succeeds");
        assert_eq!(new_id, "login.yaml");

        assert_eq!(events.borrow().len(), 1);
        assert_eq!(
            events.borrow()[0],
            WorkspaceEvent::IdsRemapped(vec![(req.id.clone(), "login.yaml".to_string())])
        );

        // 3. Subsequent unrelated notifications (e.g. bump tree version or toggle pin) do NOT re-emit
        ws_entity.update(cx, |ws, cx| {
            ws.toggle_pin("login.yaml", cx);
        });
        assert_eq!(
            events.borrow().len(),
            1,
            "no spurious events re-emitted on notify"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[gpui::test]
    fn test_move_folder_cycle_emits_crud_failed_event(cx: &mut TestAppContext) {
        let (temp_dir, store) = setup_ws("move-failed");

        let events = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        let ws_entity =
            cx.new(|_cx| WorkspaceDataState::new_for_path(temp_dir.clone(), store.clone()));

        let _sub = cx.update(|cx| {
            cx.subscribe(&ws_entity, move |_entity, event: &WorkspaceEvent, _cx| {
                events_clone.borrow_mut().push(event.clone());
            })
        });

        ws_entity.update(cx, |ws, cx| {
            ws.create_folder(None, "Parent".to_string(), cx);
            ws.create_folder(Some("parent".to_string()), "Child".to_string(), cx);
        });

        events.borrow_mut().clear();

        // Moving parent into its child must fail and emit CrudFailed immediately
        ws_entity.update(cx, |ws, cx| {
            ws.move_folder("parent", Some("parent/child".to_string()), cx);
        });

        assert_eq!(events.borrow().len(), 1);
        match &events.borrow()[0] {
            WorkspaceEvent::CrudFailed { title, message } => {
                assert_eq!(*title, "Cannot rename or move");
                assert!(message.contains("into itself"));
            }
            other => panic!("expected CrudFailed, got: {:?}", other),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
