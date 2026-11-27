use super::drag_drop::{DragFolderItem, DragMultiItem, DragPinnedItem, DragRequestItem};
use super::example_node::render_example_row;
use super::flat_tree::FlatSidebarRow;
use super::folder_node::render_folder_row;
use super::request_node::{render_request_inner_pill, render_request_row};
use super::SIDEBAR_ROW_HEIGHT;
use crate::state::{
    GitState, ModalState, PersistenceState, RequestSessionState, ResponseState, SidebarState,
    WorkspaceDataState,
};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::component::Icon;
use gpui_kit::{
    div, px, AnyElement, AppContext as _, Entity, InteractiveElement as _, IntoElement,
    ParentElement, StatefulInteractiveElement as _, Styled,
};
use std::collections::{HashMap, HashSet};

/// Inline rename targets and their active input entities.
pub struct SidebarRenameCtx<'a> {
    pub req_id: Option<&'a str>,
    pub folder_id: Option<&'a str>,
    pub example: Option<&'a (String, String)>,
    pub req_input: Option<&'a Entity<InputState>>,
    pub folder_input: Option<&'a Entity<InputState>>,
    pub example_input: Option<&'a Entity<InputState>>,
}

/// Entity handles needed by sidebar row interaction callbacks.
pub struct SidebarEntities<'a> {
    pub ws: &'a Entity<WorkspaceDataState>,
    pub persistence: &'a Entity<PersistenceState>,
    pub sidebar: &'a Entity<SidebarState>,
    pub modal: &'a Entity<ModalState>,
    pub git: &'a Entity<GitState>,
    pub ed: &'a Entity<RequestSessionState>,
    pub resp: &'a Entity<ResponseState>,
}

/// Multi-selection snapshot for the current render frame.
pub struct SidebarSelectionCtx<'a> {
    pub is_multi_select: bool,
    pub selected_requests: &'a HashSet<String>,
    pub selected_folders: &'a HashSet<String>,
}

pub struct SidebarRenderContext<'a> {
    pub active_req_id: Option<&'a str>,
    pub active_example_name: Option<&'a str>,
    pub pins: &'a [String],
    pub parse_errors: &'a HashMap<String, String>,
    pub renaming: SidebarRenameCtx<'a>,
    pub entities: SidebarEntities<'a>,
    pub selection: SidebarSelectionCtx<'a>,
}

pub(super) fn render_flat_sidebar_row(
    row: &FlatSidebarRow,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let t = theme();
    match row {
        FlatSidebarRow::PinnedHeader { is_collapsed } => {
            let sb_toggle = ctx.entities.sidebar.clone();
            let is_collapsed = *is_collapsed;
            let ws_drop = ctx.entities.ws.clone();
            let sb_drop = ctx.entities.sidebar.clone();

            div()
                .id("pinned-folder-row")
                .w_full()
                .h(px(SIDEBAR_ROW_HEIGHT))
                .pl(px(6.))
                .pr(px(6.))
                .py(px(1.))
                .flex()
                .items_center()
                .child(
                    div()
                        .id("pinned-folder-header")
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .px(px(6.))
                        .rounded(px(4.))
                        .text_color(t.text_secondary)
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(t.bg_hover))
                        .on_click(move |_, _, cx| {
                            sb_toggle.update(cx, |state, cx| {
                                state.toggle_pinned_collapsed(cx);
                            });
                        })
                        .drag_over::<DragRequestItem>(|style, _, _, _| {
                            let t = theme();
                            style.bg(t.bg_hover).border_1().border_color(t.border_focus)
                        })
                        .on_drop(move |drag: &DragRequestItem, _, cx| {
                            let req_id = drag.id.clone();
                            ws_drop.update(cx, |state, cx| {
                                if !state.is_pinned(&req_id) {
                                    state.toggle_pin(&req_id, cx);
                                }
                            });
                            sb_drop.update(cx, |state, _cx| {
                                state.pinned_collapsed = false;
                            });
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .w(px(14.))
                                        .h(px(14.))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(t.text_muted)
                                        .child(
                                            Icon::new(if is_collapsed {
                                                IconName::ChevronRight
                                            } else {
                                                IconName::ChevronDown
                                            })
                                            .size(px(11.)),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(".SystemMonoFont")
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .truncate()
                                        .child("Pinned"),
                                ),
                        ),
                )
                .into_any_element()
        }

        FlatSidebarRow::PinnedRequest {
            req,
            pin_index,
            git_status,
        } => {
            let p_id = req.id.clone();
            let drag_pin = DragPinnedItem {
                id: p_id.clone(),
                index: *pin_index,
            };
            let ws_p_drop = ctx.entities.ws.clone();
            let target_idx = *pin_index;
            let pill = render_request_inner_pill(req, true, *git_status, "pin-req-item", ctx);

            div()
                .id(format!("pinned-row-{}-{}", target_idx, p_id))
                .w_full()
                .h(px(SIDEBAR_ROW_HEIGHT))
                .pl(px(20.))
                .pr(px(6.))
                .py(px(1.))
                .flex()
                .items_center()
                .on_drag(drag_pin, |item, _, _, cx| cx.new(|_| item.clone()))
                .drag_over::<DragPinnedItem>(|style, _, _, _| {
                    let t = theme();
                    style
                        .bg(t.bg_hover)
                        .border_t_2()
                        .border_color(t.border_focus)
                })
                .on_drop(move |drag: &DragPinnedItem, _, cx| {
                    let from_idx = drag.index;
                    let to_idx = target_idx;
                    ws_p_drop.update(cx, |state, cx| {
                        if from_idx < state.pins.len() && to_idx < state.pins.len() {
                            let item = state.pins.remove(from_idx);
                            state.pins.insert(to_idx, item);
                            let _ = state.store.save_pins(&state.pins);
                            cx.emit(crate::state::WorkspaceEvent::PinsModified);
                            state.bump_tree_version();
                            cx.notify();
                        }
                    });
                })
                .child(pill)
                .into_any_element()
        }

        FlatSidebarRow::Folder {
            folder_id,
            name,
            depth,
            is_collapsed,
            git_status,
        } => render_folder_row(folder_id, name, *depth, *is_collapsed, *git_status, ctx),

        FlatSidebarRow::Request {
            req,
            depth,
            is_collapsed,
            git_status,
        } => render_request_row(req, *depth, *is_collapsed, *git_status, ctx),

        FlatSidebarRow::Example {
            parent_id,
            idx,
            total_ex,
            ex,
            depth,
        } => render_example_row(parent_id, *idx, ex, *depth, *total_ex, ctx),

        FlatSidebarRow::RootDropzone => {
            let ws_root = ctx.entities.ws.clone();
            let ws_root_folder = ctx.entities.ws.clone();
            let ws_root_multi = ctx.entities.ws.clone();
            div()
                .id("sidebar-root-dropzone")
                .w_full()
                .h(px(SIDEBAR_ROW_HEIGHT))
                .px(px(6.))
                .py(px(1.))
                .rounded(px(4.))
                .drag_over::<DragRequestItem>(|style, _, _, _| {
                    let t = theme();
                    style.bg(t.bg_hover).border_1().border_color(t.border_focus)
                })
                .on_drop(move |drag: &DragRequestItem, _, cx| {
                    ws_root.update(cx, |state, cx| {
                        state.move_request_to_folder(&drag.id, None, cx);
                    });
                })
                .drag_over::<DragFolderItem>(|style, _, _, _| {
                    let t = theme();
                    style.bg(t.bg_hover).border_1().border_color(t.border_focus)
                })
                .on_drop(move |drag: &DragFolderItem, _, cx| {
                    ws_root_folder.update(cx, |state, cx| {
                        state.move_folder(&drag.id, None, cx);
                    });
                })
                .drag_over::<DragMultiItem>(|style, _, _, _| {
                    let t = theme();
                    style.bg(t.bg_hover).border_1().border_color(t.border_focus)
                })
                .on_drop(move |drag: &DragMultiItem, _, cx| {
                    ws_root_multi.update(cx, |state, cx| {
                        for req_id in &drag.request_ids {
                            state.move_request_to_folder(req_id, None, cx);
                        }
                        for fold_id in &drag.folder_ids {
                            state.move_folder(fold_id, None, cx);
                        }
                    });
                })
                .into_any_element()
        }
    }
}
