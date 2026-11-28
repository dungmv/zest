pub mod drag_drop;
pub mod example_node;
pub mod flat_tree;
pub mod folder_node;
pub mod footer;
pub mod git_status;
pub mod item_actions;
pub mod request_node;
pub mod row_dispatch;

pub use flat_tree::*;
pub use footer::*;
pub use row_dispatch::{
    SidebarEntities, SidebarRenameCtx, SidebarRenderContext, SidebarSelectionCtx,
};

pub const SIDEBAR_ROW_HEIGHT: f32 = 28.0;

use crate::state::{
    CollectionState, GitState, ModalState, PersistenceState, RequestSessionState, ResponseState,
    SidebarState,
};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, uniform_list, App, AppContext as _, Entity, InteractiveElement as _, IntoElement,
    ListSizingBehavior, ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use row_dispatch::render_flat_sidebar_row;
use std::{cell::RefCell, rc::Rc};
use zest_core::models::Protocol;

/// Built directly via struct literal at the call site — the eight entity
/// handles are the element's entire state, so a constructor would only
/// re-emit them.
#[derive(IntoElement)]
pub struct SidebarTree {
    pub cache: Rc<RefCell<SidebarFlatCache>>,
    pub workspace_state: Entity<CollectionState>,
    pub persistence_state: Entity<PersistenceState>,
    pub sidebar_state: Entity<SidebarState>,
    pub modal_state: Entity<ModalState>,
    pub git_state: Entity<GitState>,
    pub editor_state: Entity<RequestSessionState>,
    pub response_state: Entity<ResponseState>,
}

impl RenderOnce for SidebarTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let (filter_query_raw, has_nothing, flat_rows, scroll_handle, footer_el) = {
            let ws = self.workspace_state.read(cx);
            let per = self.persistence_state.read(cx);
            let sb = self.sidebar_state.read(cx);
            let git = self.git_state.read(cx);

            let has_nothing = ws.folders().is_empty() && ws.requests().is_empty();
            let flat_rows = self.cache.borrow_mut().rows(sb, ws, per, git);
            let scroll_handle = sb.scroll_handle.clone();
            let filter_query_raw = sb.filter_query().to_owned();
            let git_branch = git.branch().unwrap_or("main");
            let footer_el = render_sidebar_footer(
                git_branch,
                git.status(),
                git.operation.is_syncing(),
                git.operation.sync_message(),
                &self.git_state,
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
                                    let sb_req = sb_menu.clone();
                                    let ws_folder = ws_menu.clone();
                                    let sb_folder = sb_menu.clone();
                                    let ws_ws = ws_menu.clone();
                                    let ed_ws = ed_menu.clone();
                                    let sb_ws = sb_menu.clone();
                                    let ws_grpc = ws_menu.clone();
                                    let ed_grpc = ed_menu.clone();
                                    let sb_grpc = sb_menu.clone();
                                    let ws_gql = ws_menu.clone();
                                    let ed_gql = ed_menu.clone();
                                    let sb_gql = sb_menu.clone();
                                    let ws_sse = ws_menu.clone();
                                    let ed_sse = ed_menu.clone();
                                    let sb_sse = sb_menu.clone();

                                    menu.item(
                                        PopupMenuItem::new("New HTTP")
                                            .icon(Icon::new(IconName::Globe))
                                            .on_click(move |_, window, cx| {
                                                super::item_actions::create_and_rename_request(
                                                    Protocol::Http,
                                                    None,
                                                    &ws_req,
                                                    &ed_req,
                                                    &sb_req,
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("New WebSocket")
                                            .icon(Icon::new(IconName::ArrowLeftRight))
                                            .on_click(move |_, window, cx| {
                                                super::item_actions::create_and_rename_request(
                                                    Protocol::WebSocket,
                                                    None,
                                                    &ws_ws,
                                                    &ed_ws,
                                                    &sb_ws,
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("New gRPC")
                                            .icon(Icon::new(IconName::Network))
                                            .on_click(move |_, window, cx| {
                                                super::item_actions::create_and_rename_request(
                                                    Protocol::Grpc,
                                                    None,
                                                    &ws_grpc,
                                                    &ed_grpc,
                                                    &sb_grpc,
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("New GraphQL")
                                            .icon(Icon::new(IconName::ChartNetwork))
                                            .on_click(move |_, window, cx| {
                                                super::item_actions::create_and_rename_request(
                                                    Protocol::GraphQL,
                                                    None,
                                                    &ws_gql,
                                                    &ed_gql,
                                                    &sb_gql,
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("New SSE")
                                            .icon(Icon::new(IconName::Zap))
                                            .on_click(move |_, window, cx| {
                                                super::item_actions::create_and_rename_request(
                                                    Protocol::Sse,
                                                    None,
                                                    &ws_sse,
                                                    &ed_sse,
                                                    &sb_sse,
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("New Folder")
                                            .icon(Icon::new(IconName::Folder))
                                            .on_click(move |_, window, cx| {
                                                super::item_actions::create_and_rename_folder(
                                                    None, &ws_folder, &sb_folder, window, cx,
                                                );
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
                        let sb_new = sb_clone.clone();
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
                                        .on_click(move |_, window, cx| {
                                            super::item_actions::create_and_rename_request(
                                                Protocol::Http,
                                                None,
                                                &ws_new,
                                                &ed_new,
                                                &sb_new,
                                                window,
                                                cx,
                                            );
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
                                        pins: ws_ref.pins(),
                                        parse_errors: ws_ref.parse_errors(),
                                        renaming: SidebarRenameCtx {
                                            req_id: sb_ref.renaming_request_id(),
                                            folder_id: sb_ref.renaming_folder_id(),
                                            example: sb_ref.renaming_example(),
                                            req_input: sb_ref.rename_input(),
                                            folder_input: sb_ref.rename_folder_input(),
                                            example_input: sb_ref.rename_example_input(),
                                        },
                                        entities: SidebarEntities {
                                            ws: &ws_clone,
                                            persistence: &per_clone,
                                            sidebar: &sb_clone,
                                            modal: &modal_clone,
                                            git: &git_clone,
                                            ed: &ed_clone,
                                            resp: &resp_clone,
                                        },
                                        selection: SidebarSelectionCtx {
                                            is_multi_select: sb_ref.is_multi_selected(),
                                            selected_requests: sb_ref.selected_requests(),
                                            selected_folders: sb_ref.selected_folders(),
                                        },
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
