mod row;
mod state;

use crate::state::{EnvironmentState, ModalState, RequestSessionState, WorkspaceDataState};
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    div, px, AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Styled, Window,
};
use state::{MpRowSnapshot, MpTableState};
use zest_core::models::MultipartItem;

/// Borrows the states the multipart editor reads and mutates while rendering.
pub struct MultipartContext<'a> {
    pub req_id_str: &'a str,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub workspace_state: Option<&'a Entity<WorkspaceDataState>>,
    pub env_state: Option<&'a Entity<EnvironmentState>>,
    pub modal_state: Option<&'a Entity<ModalState>>,
}

pub fn render_multipart_editor(
    ctx: MultipartContext<'_>,
    items: &[MultipartItem],
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let items = items.to_vec();
    let req_id_str = ctx.req_id_str.to_string();

    div()
        .size_full()
        .overflow_y_scrollbar()
        .p(px(8.))
        .flex()
        .flex_col()
        .gap(px(6.))
        // Multipart Rows
        .child({
            let table_state = window.use_keyed_state(
                gpui_kit::SharedString::from(format!("mp-table-inputs-{}", req_id_str)),
                cx,
                |_window, _cx| MpTableState { rows: Vec::new() },
            );

            table_state.update(cx, |st, cx| {
                st.reconcile(&items, &ctx, window, cx);
            });

            let row_inputs: Vec<MpRowSnapshot> = table_state.read(cx).snapshots();

            let row_ctx = row::RowCtx {
                table_state: &table_state,
                editor_state: ctx.editor_state,
                workspace_state: ctx.workspace_state,
                env_state: ctx.env_state,
                modal_state: ctx.modal_state,
                req_id: ctx.req_id_str,
            };

            div()
                .w_full()
                .flex()
                .flex_col()
                .children(items.into_iter().enumerate().map(|(idx, item)| {
                    let snapshot = row_inputs.get(idx).cloned().unwrap_or_else(|| {
                        let dummy = cx.new(|cx| InputState::new(window, cx));
                        MpRowSnapshot {
                            key_input: dummy.clone(),
                            val_input: dummy,
                            is_file: item.is_file,
                            file_path: item.file_path.clone(),
                            enabled: item.enabled,
                        }
                    });
                    row::render_multipart_row(&row_ctx, idx, snapshot, window, cx)
                }))
        })
        .into_any_element()
}
