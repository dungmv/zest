//! Event dispatcher for `WorkspaceEvent`s emitted by `WorkspaceDataState`.
//!
//! Handles synchronization across downstream states (response cache, session pointer,
//! persistence dirty map, toasts) in response to workspace CRUD events.

use crate::app::AppView;
use crate::state::WorkspaceEvent;
use gpui::Context;

pub fn handle_workspace_event(
    this: &mut AppView,
    event: &WorkspaceEvent,
    cx: &mut Context<AppView>,
) {
    match event {
        WorkspaceEvent::IdsRemapped(pairs) => {
            if pairs.is_empty() {
                return;
            }

            // 1. Rename requests in response state / cache
            this.state.response.update(cx, |resp, _cx| {
                for (old_id, new_id) in pairs {
                    resp.rename_request(old_id, new_id);
                }
            });

            // 2. Follow active request pointer if active request was moved/renamed
            let new_active = {
                let ed_ref = this.state.session.read(cx);
                pairs.iter().find_map(|(old, new)| {
                    (ed_ref.active_request_id.as_deref() == Some(old.as_str())).then(|| new.clone())
                })
            };
            if let Some(new_id) = new_active {
                this.state.session.update(cx, |ed, cx| {
                    ed.active_request_id = Some(new_id);
                    cx.notify();
                });
            }

            // 3. Remap persistence state unsaved requests / active request
            this.state.persistence.update(cx, |per, cx| {
                per.remap_request_ids(pairs, cx);
            });
        }
        WorkspaceEvent::ExampleRenamed {
            old_name, new_name, ..
        } => {
            this.state.response.update(cx, |resp, cx| {
                resp.rename_example(old_name, new_name, cx);
            });
        }
        WorkspaceEvent::CrudFailed { title, message } => {
            this.state.toast.update(cx, |t, cx| {
                t.push_error(*title, message.clone(), cx);
            });
        }
    }
}
