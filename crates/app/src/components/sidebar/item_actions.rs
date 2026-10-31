use crate::state::{RequestSessionState, SidebarState, WorkspaceState};
use gpui_kit::{App, Entity};

pub fn delete_multi_selected_items(
    ws_entity: &Entity<WorkspaceState>,
    ed_entity: &Entity<RequestSessionState>,
    sb_entity: &Entity<SidebarState>,
    cx: &mut App,
) {
    let (req_ids, folder_ids) = sb_entity.update(cx, |sb, cx| {
        let reqs: Vec<String> = sb.selected_requests.drain().collect();
        let folders: Vec<String> = sb.selected_folders.drain().collect();
        sb.ui_version = sb.ui_version.wrapping_add(1);
        cx.notify();
        (reqs, folders)
    });

    let active_id = ed_entity.read(cx).active_request_id().to_string();

    ws_entity.update(cx, |ws, cx| {
        for f_id in &folder_ids {
            ws.delete_folder(f_id, cx);
        }
        for r_id in &req_ids {
            ws.delete_request(r_id, cx);
        }
    });

    let active_still_exists = ws_entity.read(cx).find_request(&active_id).is_some();
    if !active_still_exists {
        ws_entity.update(cx, |ws, cx| {
            if let Some(first_req) = ws.requests.first() {
                let first_id = first_req.id.clone();
                ed_entity.update(cx, |ed, cx| {
                    ed.set_active_request_id(&first_id, cx);
                });
            } else {
                let new_req = ws.create_request(None, cx);
                ed_entity.update(cx, |ed, cx| {
                    ed.set_active_request_id(&new_req.id, cx);
                });
            }
        });
    }
}

pub fn delete_request_item(
    ws_entity: &Entity<WorkspaceState>,
    ed_entity: &Entity<RequestSessionState>,
    req_id: &str,
    cx: &mut App,
) {
    let is_active = ed_entity.read(cx).active_request_id() == req_id;

    let next_req_id = ws_entity.update(cx, |ws, cx| {
        let deleted_idx = ws.requests.iter().position(|r| r.id == req_id);
        ws.delete_request(req_id, cx);
        if let Some(idx) = deleted_idx {
            if !ws.requests.is_empty() {
                let new_idx = idx.min(ws.requests.len() - 1);
                Some(ws.requests[new_idx].id.clone())
            } else {
                None
            }
        } else {
            ws.requests.first().map(|r| r.id.clone())
        }
    });

    if is_active {
        if let Some(id) = next_req_id {
            ed_entity.update(cx, |ed, cx| {
                ed.set_active_request_id(&id, cx);
            });
        } else {
            let new_req = ws_entity.update(cx, |ws, cx| ws.create_request(None, cx));
            ed_entity.update(cx, |ed, cx| {
                ed.set_active_request_id(&new_req.id, cx);
            });
        }
    }
}
