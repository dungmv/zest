use super::drag_drop::{DragFolderItem, DragMultiItem, DragRequestItem};
use super::git_status::SidebarGitStatus;
use super::item_actions::{delete_multi_selected_items, delete_request_item};
use super::{SidebarRenderContext, SIDEBAR_ROW_HEIGHT};
use crate::app::actions::DismissModal;
use crate::components::{MethodBadge, ProtocolBadge, StatusCodeBadge};
use crate::services::GitService;
use crate::state::{
    CollectionState, GitState, PersistenceState, RequestSessionState, SidebarState,
};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, AppContext as _, Entity, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement, StatefulInteractiveElement as _, Styled,
};
use zest_core::models::{HttpMethod, Protocol};

#[derive(Clone, Debug)]
pub struct SidebarExampleItem {
    pub name: String,
    pub status: u16,
}

#[derive(Clone, Debug)]
pub struct SidebarReqItem {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub display_name: String,
    pub method: HttpMethod,
    pub protocol: Protocol,
    pub url: String,
    pub last_status_code: Option<u16>,
    pub examples: Vec<SidebarExampleItem>,
}

pub fn render_request_inner_pill(
    req: &SidebarReqItem,
    examples_expanded: Option<bool>,
    req_git_status: SidebarGitStatus,
    id_prefix: &str,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let t = theme();
    let is_req_active = ctx.active_req_id == Some(req.id.as_str());
    let is_viewing_example = is_req_active && ctx.active_example_name.is_some();
    let is_in_selected_set = ctx.selection.selected_requests.contains(&req.id);
    let is_selected = is_in_selected_set
        || (ctx.selection.selected_requests.is_empty()
            && ctx.selection.selected_folders.is_empty()
            && is_req_active
            && !is_viewing_example);
    let is_renaming = ctx.renaming.req_id == Some(req.id.as_str()) && id_prefix == "req-item";

    let r_id_click = req.id.clone();
    let r_id_right = req.id.clone();
    let ed_s = ctx.entities.ed.clone();
    let ed_right = ctx.entities.ed.clone();
    let ws_click = ctx.entities.ws.clone();
    let sb_click = ctx.entities.sidebar.clone();
    let sb_right = ctx.entities.sidebar.clone();
    let resp_click = ctx.entities.resp.clone();
    let initial_name_click = req.display_name.clone();

    let drag_req_payload = DragRequestItem {
        id: req.id.clone(),
        name: req.display_name.clone(),
        method: req.method,
    };
    let multi_drag_payload = (is_in_selected_set && ctx.selection.is_multi_select).then(|| {
        DragMultiItem::new(
            ctx.selection.selected_requests.iter().cloned().collect(),
            ctx.selection.selected_folders.iter().cloned().collect(),
        )
    });

    let ws_req_drop = ctx.entities.ws.clone();
    let ws_req_drop_folder = ctx.entities.ws.clone();
    let ws_req_drop_multi = ctx.entities.ws.clone();
    let target_req_id = req.id.clone();
    let target_req_folder_drop = req.folder_id.clone();
    let target_req_folder_multi = req.folder_id.clone();
    let is_pinned_row = id_prefix != "req-item";

    let req_name_color = match req_git_status {
        SidebarGitStatus::New => t.git_added,
        SidebarGitStatus::Modified => t.git_modified,
        SidebarGitStatus::Clean if is_selected => t.text_primary,
        SidebarGitStatus::Clean => t.sidebar_foreground,
    };

    div()
        .id(format!("{}-{}", id_prefix, req.id))
        .w_full()
        .h_full()
        .px(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(if is_selected { t.border } else { t.transparent })
        .bg(if is_selected {
            t.bg_selected
        } else if is_viewing_example {
            t.bg_card
        } else {
            t.transparent
        })
        .when(!is_selected, |row| row.hover(|s| s.bg(t.bg_hover)))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_between()
        .when(!is_renaming && !is_pinned_row, move |this| {
            let this = if let Some(multi_payload) = multi_drag_payload {
                this.on_drag(multi_payload, |item, _, _, cx| cx.new(|_| item.clone()))
            } else {
                this.on_drag(drag_req_payload, |item, _, _, cx| cx.new(|_| item.clone()))
            };
            this.drag_over::<DragRequestItem>(|style, _, _, _| {
                let t = theme();
                style.bg(t.bg_hover).border_color(t.border_focus)
            })
            .on_drop(move |drag: &DragRequestItem, _, cx| {
                ws_req_drop.update(cx, |state, cx| {
                    state.move_request(&drag.id, &target_req_id, cx);
                });
            })
            .drag_over::<DragFolderItem>(|style, _, _, _| {
                let t = theme();
                style.bg(t.bg_hover).border_color(t.border_focus)
            })
            .on_drop(move |drag: &DragFolderItem, _, cx| {
                let target_folder = target_req_folder_drop.clone();
                ws_req_drop_folder.update(cx, |state, cx| {
                    state.move_folder(&drag.id, target_folder, cx);
                });
            })
            .drag_over::<DragMultiItem>(|style, _, _, _| {
                let t = theme();
                style.bg(t.bg_hover).border_color(t.border_focus)
            })
            .on_drop(move |drag: &DragMultiItem, _, cx| {
                let target_folder = target_req_folder_multi.clone();
                ws_req_drop_multi.update(cx, |state, cx| {
                    for req_id in &drag.request_ids {
                        state.move_request_to_folder(req_id, target_folder.clone(), cx);
                    }
                    for fold_id in &drag.folder_ids {
                        state.move_folder(fold_id, target_folder.clone(), cx);
                    }
                });
            })
        })
        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
            let is_already_selected = sb_right.read(cx).is_request_selected(&r_id_right);
            let has_multi = sb_right.read(cx).is_multi_selected();
            if !(has_multi && is_already_selected) {
                sb_right.update(cx, |state, cx| {
                    state.select_single_request(&r_id_right, cx);
                });
                ed_right.update(cx, |state, cx| {
                    state.set_active_request_id(&r_id_right, cx);
                });
            }
        })
        .on_click(move |ev, window, cx| {
            if is_renaming {
                return;
            }
            let mods = window.modifiers();
            if mods.control || mods.platform {
                let current_active = ed_s.read(cx).active_request_id.clone();
                sb_click.update(cx, |state, cx| {
                    state.toggle_select_request(&r_id_click, current_active.as_deref(), cx);
                });
                return;
            }

            if ev.click_count() > 1 {
                let r_id = r_id_click.clone();
                let init_name = initial_name_click.clone();
                let ws_c = ws_click.clone();
                sb_click.update(cx, |state, cx| {
                    state.start_renaming_request(r_id, init_name, ws_c, window, cx);
                });
                ed_s.update(cx, |state, cx| {
                    state.set_active_request_id(&r_id_click, cx);
                });
            } else {
                let r_id = r_id_click.clone();
                sb_click.update(cx, |state, cx| {
                    state.select_single_request(&r_id, cx);
                    state.record_navigation(&r_id);
                });
                let already_active = ed_s.read(cx).active_request_id() == r_id_click;
                ed_s.update(cx, |state, cx| {
                    state.set_active_request_id(&r_id_click, cx);
                });
                if already_active {
                    let ws_store = ws_click.read(cx).store.clone();
                    resp_click.update(cx, |state, cx| {
                        state.load_for_request(&r_id, ws_store, cx);
                    });
                }
            }
        })
        .context_menu({
            let ws_menu = ctx.entities.ws.clone();
            let per_menu = ctx.entities.persistence.clone();
            let sb_menu = ctx.entities.sidebar.clone();
            let git_menu = ctx.entities.git.clone();
            let ed_menu = ctx.entities.ed.clone();
            let r_id_menu = req.id.clone();
            let init_name_menu = req.display_name.clone();
            let is_pinned = ctx.pins.contains(&r_id_menu);
            let is_dirty_git = req_git_status != SidebarGitStatus::Clean;

            move |menu, _window, cx| {
                build_request_context_menu(
                    menu,
                    &r_id_menu,
                    &init_name_menu,
                    is_pinned,
                    is_dirty_git,
                    &ws_menu,
                    &per_menu,
                    &sb_menu,
                    &git_menu,
                    &ed_menu,
                    cx,
                )
            }
        })
        .child(render_request_label(
            req,
            id_prefix,
            examples_expanded,
            is_renaming,
            req_name_color,
            ctx,
        ))
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(4.))
                .when(ctx.parse_errors.contains_key(&req.id), |this| {
                    this.child(div().text_xs().text_color(t.status_5xx).child("!"))
                })
                .when_some(req.last_status_code, |this, code| {
                    this.child(StatusCodeBadge::new(code))
                }),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn build_request_context_menu(
    menu: PopupMenu,
    req_id: &str,
    req_name: &str,
    is_pinned: bool,
    is_dirty_git: bool,
    ws: &Entity<CollectionState>,
    per: &Entity<PersistenceState>,
    sb: &Entity<SidebarState>,
    git: &Entity<GitState>,
    ed: &Entity<RequestSessionState>,
    cx: &App,
) -> PopupMenu {
    let is_in_multi = sb.read(cx).is_multi_selected() && sb.read(cx).is_request_selected(req_id);
    if is_in_multi {
        let ws_del = ws.clone();
        let ed_del = ed.clone();
        let sb_del = sb.clone();
        return menu.item(PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
            move |_, _, cx| {
                delete_multi_selected_items(&ws_del, &ed_del, &sb_del, cx);
            },
        ));
    }

    let ws_curl = ws.clone();
    let r_id_curl = req_id.to_string();

    let ws_p = ws.clone();
    let r_id_p = req_id.to_string();

    let ws_r = ws.clone();
    let sb_r = sb.clone();
    let r_id_r = req_id.to_string();
    let init_r = req_name.to_string();

    let ws_d = ws.clone();
    let ed_d = ed.clone();
    let r_id_d = req_id.to_string();

    let ws_del = ws.clone();
    let ed_del = ed.clone();
    let r_id_del = req_id.to_string();

    let mut m = menu
        .item(
            PopupMenuItem::new("Copy as cURL")
                .icon(IconName::Terminal)
                .on_click(move |_, _, cx| {
                    if let Some(r) = ws_curl.read(cx).find_request(&r_id_curl) {
                        let mut r_clone = r.clone();
                        let (resolved_h, resolved_a) =
                            ws_curl.read(cx).resolve_request_hierarchy(&r_clone);
                        r_clone.headers = resolved_h;
                        r_clone.auth = resolved_a;
                        let curl = zest_core::curl::request_to_curl(&r_clone, None);
                        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(curl));
                    }
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new(if is_pinned { "Unpin" } else { "Pin to top" })
                .icon(IconName::Pin)
                .on_click(move |_, _, cx| {
                    ws_p.update(cx, |state, cx| {
                        state.toggle_pin(&r_id_p, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Rename")
                .icon(IconName::Pencil)
                .on_click(move |_, window, cx| {
                    let ws_c = ws_r.clone();
                    sb_r.update(cx, |state, cx| {
                        state.start_renaming_request(
                            r_id_r.clone(),
                            init_r.clone(),
                            ws_c,
                            window,
                            cx,
                        );
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Duplicate")
                .icon(IconName::Copy)
                .on_click(move |_, _, cx| {
                    let session = ed_d.clone();
                    ws_d.update(cx, |state, cx| {
                        state.duplicate_request(&r_id_d, cx, move |request, cx| {
                            session.update(cx, |state, cx| {
                                state.set_active_request_id(&request.id, cx)
                            });
                        })
                    });
                }),
        );

    if is_dirty_git {
        let git_disc = git.clone();
        let ws_disc = ws.clone();
        let per_disc = per.clone();
        let r_id_disc = req_id.to_string();
        let r_name_disc = req_name.to_string();

        m = m.separator().item(
            PopupMenuItem::new("Discard Changes")
                .icon(IconName::Undo2)
                .on_click(move |_, _, cx| {
                    discard_request_changes(
                        &r_id_disc,
                        &r_name_disc,
                        &git_disc,
                        &ws_disc,
                        &per_disc,
                        cx,
                    );
                }),
        );
    }

    m.separator()
        .item(
            PopupMenuItem::new("Delete")
                .icon(IconName::Trash)
                .on_click(move |_, _, cx| {
                    delete_request_item(&ws_del, &ed_del, &r_id_del, cx);
                }),
        )
}

fn discard_request_changes(
    req_id: &str,
    req_name: &str,
    git: &Entity<GitState>,
    ws: &Entity<CollectionState>,
    per: &Entity<PersistenceState>,
    cx: &mut App,
) {
    let norm_req = req_id.replace('\\', "/");
    let coll_rel = format!("collections/{}", norm_req);
    let path_to_discard = {
        let g = git.read(cx);
        if let Some(git_st) = g.status() {
            let matches = |p: &str| -> bool {
                let p = p.replace('\\', "/");
                p == norm_req
                    || p == coll_rel
                    || p.ends_with(&format!("/{}", norm_req))
                    || p.ends_with(&format!("/{}", coll_rel))
            };
            git_st
                .untracked_files
                .iter()
                .find(|i| matches(&i.path))
                .or_else(|| git_st.unstaged_files.iter().find(|i| matches(&i.path)))
                .or_else(|| git_st.staged_files.iter().find(|i| matches(&i.path)))
                .map(|i| i.path.clone())
                .unwrap_or(coll_rel)
        } else {
            coll_rel
        }
    };

    // Never delete never-committed request files: for an untracked request
    // this only drops unsaved in-memory edits via mark_clean + reload_all.
    per.update(cx, |p, cx| {
        p.cancel_request(req_id, cx);
    });
    let r_id_task = req_id.to_string();
    let per_cb = per.clone();
    let ws_cb = ws.clone();
    let req_name_cb = req_name.to_string();
    let svc = GitService::new(git.clone());
    svc.discard_file(&path_to_discard, false, cx, move |cx| {
        per_cb.update(cx, |p, cx| {
            p.mark_clean(&r_id_task, cx);
            p.clear_request_conflict(&r_id_task, cx);
        });
        ws_cb.update(cx, |w, cx| w.request_rescan(cx));
        crate::notify::info(
            "Git Discard",
            format!("Discarded changes in '{}'", req_name_cb),
            cx,
        );
    });
}

fn render_request_label(
    req: &SidebarReqItem,
    id_prefix: &str,
    examples_expanded: Option<bool>,
    is_renaming: bool,
    req_name_color: Hsla,
    ctx: &SidebarRenderContext,
) -> gpui_kit::Div {
    let t = theme();
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .items_center()
        .gap(px(6.))
        .overflow_hidden()
        .child(if let Some(is_expanded) = examples_expanded {
            let sb_toggle = ctx.entities.sidebar.clone();
            let r_id = req.id.clone();
            div()
                .id(format!("req-toggle-{}-{}", id_prefix, r_id))
                .w(px(14.))
                .h(px(14.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.text_color(t.sidebar_foreground))
                .text_color(t.icon_muted)
                .child(
                    Icon::new(if is_expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .size(px(11.)),
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    sb_toggle.update(cx, |sb, cx| {
                        sb.toggle_request_expansion(&r_id, cx);
                    });
                })
                .into_any_element()
        } else {
            div()
                .w(px(14.))
                .h(px(14.))
                .flex_shrink_0()
                .into_any_element()
        })
        .child(if req.protocol.is_http() {
            MethodBadge::new(req.method).into_any_element()
        } else {
            ProtocolBadge::new(req.protocol).into_any_element()
        })
        .child(
            if let (true, Some(inp)) = (is_renaming, ctx.renaming.req_input) {
                let sb_esc = ctx.entities.sidebar.clone();
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h(px(22.))
                    .px(px(4.))
                    .rounded(px(4.))
                    .bg(t.bg_input)
                    .border_1()
                    .border_color(t.border_focus)
                    .flex()
                    .items_center()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_action(move |_: &DismissModal, _, cx| {
                        sb_esc.update(cx, |state, cx| {
                            state.cancel_renaming(cx);
                        });
                    })
                    .child(
                        gpui_kit::component::input::Input::new(inp)
                            .id(format!("rename-input-field-{}", req.id))
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .cleanable(false)
                            .xsmall()
                            .font_family(".SystemMonoFont")
                            .font_weight(gpui_kit::FontWeight::NORMAL),
                    )
                    .into_any_element()
            } else {
                div()
                    .text_xs()
                    .font_family(".SystemMonoFont")
                    .font_weight(gpui_kit::FontWeight::NORMAL)
                    .text_color(req_name_color)
                    .truncate()
                    .child(req.display_name.clone())
                    .into_any_element()
            },
        )
}

pub fn render_request_row(
    req: &SidebarReqItem,
    depth: usize,
    is_expanded: bool,
    req_git_status: SidebarGitStatus,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let examples_expanded = (!req.examples.is_empty()).then_some(is_expanded);
    let pill = render_request_inner_pill(req, examples_expanded, req_git_status, "req-item", ctx);
    div()
        .w_full()
        .h(px(SIDEBAR_ROW_HEIGHT))
        .pl(px(6.0 + depth as f32 * 14.0))
        .pr(px(6.))
        .py(px(1.))
        .flex()
        .items_center()
        .child(pill)
        .into_any_element()
}
