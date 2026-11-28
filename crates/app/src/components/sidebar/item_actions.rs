use crate::state::{CollectionState, RequestSessionState, SidebarState};
use gpui_kit::{App, Entity, Window};
use zest_core::models::Protocol;

/// Creates a new request of `protocol`, activates it and starts inline rename.
pub fn create_and_rename_request(
    protocol: Protocol,
    folder_id: Option<String>,
    ws_entity: &Entity<CollectionState>,
    ed_entity: &Entity<RequestSessionState>,
    sb_entity: &Entity<SidebarState>,
    window: &mut Window,
    cx: &mut App,
) {
    let window = window.window_handle();
    let collection = ws_entity.clone();
    let session = ed_entity.clone();
    let sidebar = sb_entity.clone();
    ws_entity.update(cx, |state, cx| {
        state.create_request(folder_id, protocol, cx, move |request, cx| {
            session.update(cx, |state, cx| state.set_active_request_id(&request.id, cx));
            let _ = window.update(cx, |_, window, cx| {
                sidebar.update(cx, |state, cx| {
                    state.start_renaming_request(request.id, request.name, collection, window, cx);
                });
            });
        });
    });
}

pub fn create_and_rename_folder(
    parent: Option<String>,
    collection: &Entity<CollectionState>,
    sidebar: &Entity<SidebarState>,
    window: &mut Window,
    cx: &mut App,
) {
    let window = window.window_handle();
    let target = collection.clone();
    let sidebar = sidebar.clone();
    let expand = parent.clone();
    collection.update(cx, |state, cx| {
        state.create_folder(parent, "new-folder".into(), cx, move |id, cx| {
            let _ = window.update(cx, |_, window, cx| {
                sidebar.update(cx, |state, cx| {
                    if let Some(parent) = expand {
                        state.expand_folder(&parent, cx);
                    }
                    state.start_renaming_folder(id, "new-folder".into(), target, window, cx);
                });
            });
        });
    });
}

pub fn delete_multi_selected_items(
    ws_entity: &Entity<CollectionState>,
    ed_entity: &Entity<RequestSessionState>,
    sb_entity: &Entity<SidebarState>,
    cx: &mut App,
) {
    let (req_ids, folder_ids) = sb_entity.update(cx, |sb, cx| sb.take_selection(cx));

    let active_id = ed_entity.read(cx).active_request_id().to_string();

    let session = ed_entity.clone();
    ws_entity.update(cx, |ws, cx| {
        for f_id in &folder_ids {
            ws.delete_folder(f_id, cx);
        }
        for r_id in &req_ids {
            ws.delete_request(r_id, cx, |_, _, _| {});
        }
        ws.after_disk_edits(cx, move |ws, cx| {
            if session.read(cx).active_request_id() == active_id
                && ws.find_request(&active_id).is_none()
            {
                select_after_delete(
                    ws,
                    ws.requests().first().map(|request| request.id.clone()),
                    session,
                    cx,
                );
            }
        });
    });
}

pub fn delete_request_item(
    ws_entity: &Entity<CollectionState>,
    ed_entity: &Entity<RequestSessionState>,
    req_id: &str,
    cx: &mut App,
) {
    let session = ed_entity.clone();
    let id = req_id.to_string();
    ws_entity.update(cx, |ws, cx| {
        ws.delete_request(req_id, cx, move |ws, next, cx| {
            if session.read(cx).active_request_id() == id {
                select_after_delete(ws, next, session, cx);
            }
        });
    });
}

fn select_after_delete(
    collection: &mut CollectionState,
    next: Option<String>,
    session: Entity<RequestSessionState>,
    cx: &mut gpui_kit::Context<CollectionState>,
) {
    if let Some(id) = next {
        session.update(cx, |state, cx| state.set_active_request_id(&id, cx));
    } else {
        let previous = session.read(cx).active_request_id().to_owned();
        collection.create_request(None, Protocol::Http, cx, move |new, cx| {
            session.update(cx, |state, cx| {
                if state.active_request_id() == previous {
                    state.set_active_request_id(&new.id, cx);
                }
            });
        });
    }
}
