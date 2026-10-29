use crate::state::{RequestSessionState, WorkspaceState};
use gpui_kit::{App, Entity};

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
