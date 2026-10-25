pub mod drag_drop;
pub mod example_node;
pub mod folder_node;
pub mod footer;
pub mod git_status;
pub mod item_actions;
pub mod request_node;

pub use drag_drop::*;
pub use example_node::*;
pub use folder_node::*;
pub use footer::*;
pub use git_status::*;
pub use item_actions::*;
pub use request_node::*;

use crate::state::{
    GitState, ModalState, PersistenceState, RequestSessionState, ResponseState, SidebarState,
    ToastState, WorkspaceDataState,
};
use crate::theme::theme;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::InputState;
use gpui_component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::Icon;
use gpui_kit::assets::IconName;
use std::collections::{HashMap, HashSet};
use zest_core::git::GitStatusResult;
use zest_core::models::Folder;

pub struct SidebarRenderContext<'a> {
    pub folders: &'a [Folder],
    pub requests: &'a [SidebarReqItem],
    pub query: &'a str,
    pub active_req_id: Option<&'a String>,
    pub active_example_name: Option<&'a str>,
    pub renaming_req_id: Option<&'a String>,
    pub renaming_folder_id: Option<&'a str>,
    pub renaming_example: Option<&'a (String, String)>,
    pub rename_input: Option<&'a Entity<InputState>>,
    pub rename_folder_input: Option<&'a Entity<InputState>>,
    pub rename_example_input: Option<&'a Entity<InputState>>,
    pub collapsed_folders: &'a HashSet<String>,
    pub collapsed_requests: &'a HashSet<String>,
    pub pins: &'a [String],
    pub parse_errors: &'a HashMap<String, String>,
    pub unsaved_requests: &'a HashSet<String>,
    pub running_request_id: Option<&'a str>,
    pub git_status: Option<&'a GitStatusResult>,
    pub ws: &'a Entity<WorkspaceDataState>,
    pub persistence: &'a Entity<PersistenceState>,
    pub sidebar: &'a Entity<SidebarState>,
    pub modal: &'a Entity<ModalState>,
    pub git: &'a Entity<GitState>,
    pub ed: &'a Entity<RequestSessionState>,
    pub resp: &'a Entity<ResponseState>,
    pub toast: &'a Entity<ToastState>,
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
        let (
            git_branch,
            git_status,
            git_is_syncing,
            query,
            filter_query_raw,
            folders,
            requests,
            renaming_req_id,
            rename_input,
            renaming_folder_id,
            rename_folder_input,
            renaming_example,
            rename_example_input,
            collapsed_folders,
            collapsed_requests,
            pins,
            parse_errors,
            unsaved_requests,
            running_request_id,
        ) = {
            let ws = self.workspace_state.read(cx);
            let per = self.persistence_state.read(cx);
            let sb = self.sidebar_state.read(cx);
            let git = self.git_state.read(cx);
            let req_items: Vec<SidebarReqItem> = ws
                .requests
                .iter()
                .map(|r| {
                    let initial_name = {
                        let eff = crate::state::effective_request_name(r);
                        if !r.name.trim().is_empty()
                            && r.name != "New Request"
                            && r.name != "Untitled Request"
                        {
                            r.name.clone()
                        } else {
                            eff
                        }
                    };
                    let examples = r
                        .examples
                        .iter()
                        .map(|e| SidebarExampleItem {
                            name: e.name.clone(),
                            status: e.status,
                        })
                        .collect();

                    SidebarReqItem {
                        id: r.id.clone(),
                        folder_id: r.folder_id.clone(),
                        name: r.name.clone(),
                        display_name: initial_name,
                        method: r.method,
                        url: r.url.clone(),
                        last_status_code: r.last_status_code,
                        examples,
                    }
                })
                .collect();

            (
                git.branch.clone().unwrap_or_else(|| "main".to_string()),
                git.status.clone(),
                git.is_syncing,
                sb.filter_query.to_lowercase(),
                sb.filter_query.clone(),
                ws.folders.clone(),
                req_items,
                sb.renaming_request_id().map(ToString::to_string),
                sb.rename_input().cloned(),
                sb.renaming_folder_id().map(ToString::to_string),
                sb.rename_folder_input().cloned(),
                sb.renaming_example().cloned(),
                sb.rename_example_input().cloned(),
                sb.collapsed_folders.clone(),
                sb.collapsed_requests.clone(),
                ws.pins.clone(),
                ws.parse_errors.clone(),
                per.unsaved_requests.clone(),
                self.editor_state.read(cx).running_request_id.clone(),
            )
        };
        let is_syncing = git_is_syncing;

        let active_req_id = self.editor_state.read(cx).active_request_id.clone();
        let active_example_name = self.response_state.read(cx).active_example_name.clone();
        let has_nothing = folders.is_empty() && requests.is_empty();

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
            gpui::SharedString::from("sidebar-search-input-holder"),
            cx,
            |window, cx| {
                let search_input = cx.new(|cx| {
                    gpui_component::input::InputState::new(window, cx).placeholder("Search...")
                });
                let s_in = search_input.clone();
                let sub = cx.subscribe_in(&search_input, window, move |_, _, event, _, cx| {
                    if let gpui_component::input::InputEvent::Change = event {
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

        let ctx = SidebarRenderContext {
            folders: &folders,
            requests: &requests,
            query: &query,
            active_req_id: active_req_id.as_ref(),
            active_example_name: active_example_name.as_deref(),
            renaming_req_id: renaming_req_id.as_ref(),
            renaming_folder_id: renaming_folder_id.as_deref(),
            renaming_example: renaming_example.as_ref(),
            rename_input: rename_input.as_ref(),
            rename_folder_input: rename_folder_input.as_ref(),
            rename_example_input: rename_example_input.as_ref(),
            collapsed_folders: &collapsed_folders,
            collapsed_requests: &collapsed_requests,
            pins: &pins,
            parse_errors: &parse_errors,
            unsaved_requests: &unsaved_requests,
            running_request_id: running_request_id.as_deref(),
            git_status: git_status.as_ref(),
            ws: &ws_clone,
            persistence: &per_clone,
            sidebar: &sb_clone,
            modal: &modal_clone,
            git: &git_clone,
            ed: &ed_clone,
            resp: &resp_clone,
            toast: &toast_clone,
        };

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
                                    gpui_component::input::Input::new(&search_input)
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
                                gpui::Anchor::TopRight,
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
            // Middle section: Folders & Requests tree
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .px(px(6.))
                    .py(px(4.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
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
                                        .font_weight(gpui::FontWeight::MEDIUM)
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
                    .when(!pins.is_empty(), |this| {
                        let ws_pin_reorder = ws_clone.clone();
                        let pinned_requests: Vec<_> = pins
                            .iter()
                            .filter_map(|pin_id| requests.iter().find(|r| &r.id == pin_id).cloned())
                            .collect();

                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .mb(px(4.))
                                .child(
                                    div()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.text_muted)
                                        .child("📌 PINNED"),
                                )
                                .children(pinned_requests.into_iter().enumerate().map(
                                    |(idx, req)| {
                                        let p_id = req.id.clone();
                                        let drag_pin = DragPinnedItem {
                                            id: p_id.clone(),
                                            index: idx,
                                        };
                                        let ws_p_drop = ws_pin_reorder.clone();
                                        let target_idx = idx;

                                        div()
                                            .id(format!("pinned-row-{}", p_id))
                                            .on_drag(drag_pin, |item, _, _, cx| {
                                                cx.new(|_| item.clone())
                                            })
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
                                                    if from_idx < state.pins.len()
                                                        && to_idx < state.pins.len()
                                                    {
                                                        let item = state.pins.remove(from_idx);
                                                        state.pins.insert(to_idx, item);
                                                        let _ = state.store.save_pins(&state.pins);
                                                        cx.notify();
                                                    }
                                                });
                                            })
                                            .child(render_request_node(&req, false, &ctx))
                                    },
                                )),
                        )
                    })
                    .children({
                        let root_folders: Vec<_> = folders
                            .iter()
                            .filter(|f| {
                                f.parent_id.is_none()
                                    || !folders.iter().any(|p| Some(&p.id) == f.parent_id.as_ref())
                            })
                            .collect();
                        root_folders
                            .into_iter()
                            .filter_map(|folder| render_folder_node(folder, &ctx))
                    })
                    // Top-level requests without folder
                    .children(
                        requests
                            .iter()
                            .filter(|r| {
                                (r.folder_id.is_none()
                                    || !folders.iter().any(|f| Some(&f.id) == r.folder_id.as_ref()))
                                    && (query.is_empty()
                                        || r.name.to_lowercase().contains(&query)
                                        || r.url.to_lowercase().contains(&query)
                                        || r.examples
                                            .iter()
                                            .any(|e| e.name.to_lowercase().contains(&query)))
                            })
                            .map(|req| render_request_node(req, false, &ctx)),
                    )
                    // Drop zone at bottom to drag a request or folder back to root level
                    .child({
                        let ws_root = ws_clone.clone();
                        let ws_root_folder = ws_clone.clone();
                        div()
                            .id("sidebar-root-dropzone")
                            .min_h(px(24.))
                            .flex_1()
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
                    }),
            )
            // Footer at bottom of sidebar: Branch info on left, Pull button on right
            .child(render_sidebar_footer(
                &git_branch,
                git_status.as_ref(),
                is_syncing,
                &git_clone,
                &toast_clone,
            ))
    }
}
