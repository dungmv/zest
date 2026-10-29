pub mod drag_drop;
pub mod example_node;
pub mod flat_tree;
pub mod folder_node;
pub mod footer;
pub mod git_status;
pub mod item_actions;
pub mod request_node;

pub use drag_drop::*;
pub use example_node::*;
pub use flat_tree::*;
pub use folder_node::*;
pub use footer::*;
pub use git_status::*;
pub use item_actions::*;
pub use request_node::*;

pub const SIDEBAR_ROW_HEIGHT: f32 = 28.0;

use crate::state::{
    GitState, ModalState, PersistenceState, RequestSessionState, ResponseState, SidebarState,
    ToastState, WorkspaceDataState,
};
use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, uniform_list, AnyElement, App, AppContext as _, Entity, InteractiveElement as _,
    IntoElement, ListSizingBehavior, ParentElement, RenderOnce, StatefulInteractiveElement as _,
    Styled, Window,
};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Icon;
use gpui_kit::assets::IconName;
use std::collections::HashMap;

pub struct SidebarRenderContext<'a> {
    pub active_req_id: Option<&'a str>,
    pub active_example_name: Option<&'a str>,
    pub renaming_req_id: Option<&'a str>,
    pub renaming_folder_id: Option<&'a str>,
    pub renaming_example: Option<&'a (String, String)>,
    pub rename_input: Option<&'a Entity<InputState>>,
    pub rename_folder_input: Option<&'a Entity<InputState>>,
    pub rename_example_input: Option<&'a Entity<InputState>>,
    pub pins: &'a [String],
    pub parse_errors: &'a HashMap<String, String>,
    pub running_request_id: Option<&'a str>,
    pub ws: &'a Entity<WorkspaceDataState>,
    pub persistence: &'a Entity<PersistenceState>,
    pub sidebar: &'a Entity<SidebarState>,
    pub modal: &'a Entity<ModalState>,
    pub git: &'a Entity<GitState>,
    pub ed: &'a Entity<RequestSessionState>,
    pub resp: &'a Entity<ResponseState>,
    pub toast: &'a Entity<ToastState>,
}

fn render_flat_sidebar_row(row: &FlatSidebarRow, ctx: &SidebarRenderContext) -> AnyElement {
    let t = theme();
    match row {
        FlatSidebarRow::PinnedHeader => div()
            .w_full()
            .h(px(SIDEBAR_ROW_HEIGHT))
            .px(px(12.))
            .py(px(1.))
            .flex()
            .items_center()
            .text_xs()
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .text_color(t.text_muted)
            .child("📌 PINNED")
            .into_any_element(),

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
            let ws_p_drop = ctx.ws.clone();
            let target_idx = *pin_index;
            let pill = render_request_inner_pill(req, true, *git_status, "pin-req-item", ctx);

            div()
                .id(format!("pinned-row-{}-{}", target_idx, p_id))
                .w_full()
                .h(px(SIDEBAR_ROW_HEIGHT))
                .px(px(6.))
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
            let ws_root = ctx.ws.clone();
            let ws_root_folder = ctx.ws.clone();
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
                .into_any_element()
        }
    }
}

#[derive(IntoElement)]
pub struct SidebarTree {
    workspace_state: Entity<WorkspaceDataState>,
    persistence_state: Entity<PersistenceState>,
    sidebar_state: Entity<SidebarState>,
    modal_state: Entity<ModalState>,
    git_state: Entity<GitState>,
    editor_state: Entity<RequestSessionState>,
    response_state: Entity<ResponseState>,
    toast_state: Entity<ToastState>,
}

impl SidebarTree {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        workspace_state: Entity<WorkspaceDataState>,
        persistence_state: Entity<PersistenceState>,
        sidebar_state: Entity<SidebarState>,
        modal_state: Entity<ModalState>,
        git_state: Entity<GitState>,
        editor_state: Entity<RequestSessionState>,
        response_state: Entity<ResponseState>,
        toast_state: Entity<ToastState>,
    ) -> Self {
        Self {
            workspace_state,
            persistence_state,
            sidebar_state,
            modal_state,
            git_state,
            editor_state,
            response_state,
            toast_state,
        }
    }
}

impl RenderOnce for SidebarTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let (filter_query_raw, has_nothing, flat_rows, scroll_handle, footer_el) = {
            let ws = self.workspace_state.read(cx);
            let per = self.persistence_state.read(cx);
            let sb = self.sidebar_state.read(cx);
            let git = self.git_state.read(cx);

            let has_nothing = ws.folders.is_empty() && ws.requests.is_empty();
            let flat_rows = sb.cached_flat_rows(ws, per, git);
            let scroll_handle = sb.scroll_handle.clone();
            let filter_query_raw = sb.filter_query.clone();
            let git_branch = git.branch.as_deref().unwrap_or("main");
            let footer_el = render_sidebar_footer(
                git_branch,
                git.status.as_ref(),
                git.is_syncing,
                git.sync_message.as_deref(),
                &self.git_state,
                &self.toast_state,
            );

            (
                filter_query_raw,
                has_nothing,
                flat_rows,
                scroll_handle,
                footer_el,
            )
        };

        let ws_clone = self.workspace_state.clone();
        let per_clone = self.persistence_state.clone();
        let sb_clone = self.sidebar_state.clone();
        let modal_clone = self.modal_state.clone();
        let git_clone = self.git_state.clone();
        let ed_clone = self.editor_state.clone();
        let resp_clone = self.response_state.clone();
        let toast_clone = self.toast_state.clone();

        let sb_search = sb_clone.clone();
        let search_holder = window.use_keyed_state(
            gpui_kit::SharedString::from("sidebar-search-input-holder"),
            cx,
            |window, cx| {
                let search_input = cx.new(|cx| {
                    gpui_kit::component::input::InputState::new(window, cx).placeholder("Search...")
                });
                let s_in = search_input.clone();
                let sub = cx.subscribe_in(&search_input, window, move |_, _, event, _, cx| {
                    if let gpui_kit::component::input::InputEvent::Change = event {
                        let q = s_in.read(cx).value().to_string();
                        sb_search.update(cx, |state, cx| {
                            state.set_filter_query(q, cx);
                        });
                    }
                });
                (search_input, sub)
            },
        );

        let search_input = search_holder.read(cx).0.clone();
        if search_input.read(cx).value().as_str() != filter_query_raw {
            search_input.update(cx, |inp, cx| {
                inp.set_value(filter_query_raw.clone(), window, cx);
            });
        }

        let row_count = flat_rows.len();

        div()
            .w_full()
            .h_full()
            .bg(t.bg_sidebar)
            .border_r_1()
            .border_color(t.border)
            .flex()
            .flex_col()
            .justify_between()
            // Top section: search bar + overflow menu
            .child(
                div()
                    .p(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .h(px(28.))
                            .px(px(6.))
                            .rounded(px(6.))
                            .bg(t.bg_input)
                            .border_1()
                            .border_color(t.border_subtle)
                            .flex()
                            .items_center()
                            .child(
                                div().flex_1().child(
                                    gpui_kit::component::input::Input::new(&search_input)
                                        .id("sidebar-search-input")
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .cleanable(true),
                                ),
                            ),
                    )
                    // Overflow menu (+) - create request or folder
                    .child({
                        let ws_menu = ws_clone.clone();
                        let sb_menu = sb_clone.clone();
                        let ed_menu = ed_clone.clone();
                        Button::new("btn-sidebar-new")
                            .ghost()
                            .compact()
                            .w(px(28.))
                            .h(px(28.))
                            .child(
                                Icon::new(IconName::Plus)
                                    .size(px(15.))
                                    .text_color(t.text_secondary),
                            )
                            .dropdown_menu_with_anchor(
                                gpui_kit::Anchor::TopRight,
                                move |menu, _window, _cx| {
                                    let ws_req = ws_menu.clone();
                                    let ed_req = ed_menu.clone();
                                    let ws_folder = ws_menu.clone();
                                    let sb_folder = sb_menu.clone();

                                    menu.item(
                                        PopupMenuItem::new("New Request")
                                            .icon(Icon::new(IconName::FileText))
                                            .on_click(move |_, _, cx| {
                                                let req = ws_req.update(cx, |state, cx| {
                                                    state.create_request(None, cx)
                                                });
                                                ed_req.update(cx, |state, cx| {
                                                    state.set_active_request_id(&req.id, cx);
                                                });
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("New Folder")
                                            .icon(Icon::new(IconName::Folder))
                                            .on_click(move |_, window, cx| {
                                                let ws_c = ws_folder.clone();
                                                let created = ws_folder.update(cx, |state, cx| {
                                                    state.create_folder(
                                                        None,
                                                        "new-folder".to_string(),
                                                        cx,
                                                    )
                                                });
                                                if let Some(new_id) = created {
                                                    sb_folder.update(cx, |state, cx| {
                                                        state.start_renaming_folder(
                                                            new_id,
                                                            "new-folder".to_string(),
                                                            ws_c,
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                }
                                            }),
                                    )
                                },
                            )
                    }),
            )
            // Middle section: Virtualized Folders & Requests tree
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .relative()
                    .overflow_hidden()
                    .py(px(4.))
                    .when(has_nothing, |this| {
                        let ws_new = ws_clone.clone();
                        let ed_new = ed_clone.clone();
                        this.child(
                            div()
                                .py(px(32.))
                                .px(px(12.))
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .child("No saved requests yet"),
                                )
                                .child(
                                    div()
                                        .id("btn-sidebar-empty-new-req")
                                        .px(px(10.))
                                        .py(px(5.))
                                        .rounded(px(5.))
                                        .bg(t.bg_card)
                                        .border_1()
                                        .border_color(t.border)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(t.text_secondary)
                                        .on_click(move |_, _, cx| {
                                            let req = ws_new.update(cx, |state, cx| {
                                                state.create_request(None, cx)
                                            });
                                            ed_new.update(cx, |state, cx| {
                                                state.set_active_request_id(&req.id, cx);
                                            });
                                        })
                                        .child("+ New Request"),
                                ),
                        )
                    })
                    .when(!has_nothing, move |this| {
                        this.child(
                            uniform_list(
                                "sidebar-tree-uniform-list",
                                row_count,
                                move |visible_range, _window, cx| {
                                    let ws_ref = ws_clone.read(cx);
                                    let sb_ref = sb_clone.read(cx);
                                    let ed_ref = ed_clone.read(cx);
                                    let resp_ref = resp_clone.read(cx);

                                    let ctx = SidebarRenderContext {
                                        active_req_id: ed_ref.active_request_id.as_deref(),
                                        active_example_name: resp_ref
                                            .active_example_name
                                            .as_deref(),
                                        renaming_req_id: sb_ref.renaming_request_id(),
                                        renaming_folder_id: sb_ref.renaming_folder_id(),
                                        renaming_example: sb_ref.renaming_example(),
                                        rename_input: sb_ref.rename_input(),
                                        rename_folder_input: sb_ref.rename_folder_input(),
                                        rename_example_input: sb_ref.rename_example_input(),
                                        pins: &ws_ref.pins,
                                        parse_errors: &ws_ref.parse_errors,
                                        running_request_id: ed_ref.running_request_id.as_deref(),
                                        ws: &ws_clone,
                                        persistence: &per_clone,
                                        sidebar: &sb_clone,
                                        modal: &modal_clone,
                                        git: &git_clone,
                                        ed: &ed_clone,
                                        resp: &resp_clone,
                                        toast: &toast_clone,
                                    };

                                    let mut elements = Vec::with_capacity(visible_range.len());
                                    for idx in visible_range {
                                        if let Some(row) = flat_rows.get(idx) {
                                            elements.push(render_flat_sidebar_row(row, &ctx));
                                        }
                                    }
                                    elements
                                },
                            )
                            .size_full()
                            .with_sizing_behavior(ListSizingBehavior::Auto)
                            .track_scroll(&scroll_handle),
                        )
                        .vertical_scrollbar(&scroll_handle)
                    }),
            )
            // Footer at bottom of sidebar: Branch info on left, Pull button on right
            .child(footer_el)
    }
}
