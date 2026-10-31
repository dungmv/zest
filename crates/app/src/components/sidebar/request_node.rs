use super::drag_drop::{DragFolderItem, DragMultiItem, DragRequestItem};
use super::git_status::SidebarGitStatus;
use super::item_actions::{delete_multi_selected_items, delete_request_item};
use super::{SidebarRenderContext, SIDEBAR_ROW_HEIGHT};
use crate::app::actions::DismissModal;
use crate::components::{MethodBadge, StatusCodeBadge};
use crate::services::GitService;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, AppContext as _, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement as _, Styled,
};
use zest_core::models::HttpMethod;
use zest_core::storage::models::RequestFile;
use zest_core::storage::serialize::to_yaml_string;

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
    pub url: String,
    pub last_status_code: Option<u16>,
    pub examples: Vec<SidebarExampleItem>,
}

pub fn render_request_inner_pill(
    req: &SidebarReqItem,
    is_req_collapsed: bool,
    req_git_status: SidebarGitStatus,
    id_prefix: &str,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let t = theme();
    let is_req_active = ctx.active_req_id == Some(req.id.as_str());
    let is_viewing_example = is_req_active && ctx.active_example_name.is_some();
    let is_in_selected_set = ctx.selected_requests.contains(&req.id);
    let is_selected = is_in_selected_set
        || (ctx.selected_requests.is_empty()
            && ctx.selected_folders.is_empty()
            && is_req_active
            && !is_viewing_example);
    let is_renaming = ctx.renaming_req_id == Some(req.id.as_str()) && id_prefix == "req-item";
    let has_examples = !req.examples.is_empty() && id_prefix == "req-item";

    let r_id_click = req.id.clone();
    let r_id_right = req.id.clone();
    let r_id_menu = req.id.clone();
    let ed_s = ctx.ed.clone();
    let ed_right = ctx.ed.clone();
    let ws_click = ctx.ws.clone();
    let sb_click = ctx.sidebar.clone();
    let sb_right = ctx.sidebar.clone();
    let resp_click = ctx.resp.clone();

    let initial_name = req.display_name.clone();
    let initial_name_click = initial_name.clone();
    let initial_name_menu = initial_name.clone();

    let drag_req_payload = DragRequestItem {
        id: req.id.clone(),
        name: initial_name.clone(),
        method: req.method,
    };
    let multi_drag_payload = if is_in_selected_set && ctx.is_multi_select {
        Some(DragMultiItem::new(
            ctx.selected_requests.iter().cloned().collect(),
            ctx.selected_folders.iter().cloned().collect(),
        ))
    } else {
        None
    };

    let ws_req_drop = ctx.ws.clone();
    let ws_req_drop_folder = ctx.ws.clone();
    let ws_req_drop_multi = ctx.ws.clone();
    let target_req_id = req.id.clone();
    let target_req_folder_drop = req.folder_id.clone();
    let target_req_folder_multi = req.folder_id.clone();
    let is_pinned_row = id_prefix != "req-item";

    let req_name_color = match req_git_status {
        SidebarGitStatus::New => t.git_added,
        SidebarGitStatus::Modified => t.git_modified,
        SidebarGitStatus::Clean => t.text_primary,
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
        .hover(|s| s.bg(t.bg_hover))
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
            if has_multi && is_already_selected {
                // Keep multi-selection intact
            } else {
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
            let is_ctrl_or_cmd = mods.control || mods.platform;
            if is_ctrl_or_cmd {
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
                ed_s.update(cx, |state, cx| {
                    state.set_active_request_id(&r_id_click, cx);
                });
                let ws_store = ws_click.read(cx).store.clone();
                resp_click.update(cx, |state, cx| {
                    state.load_for_request(&r_id, ws_store.as_ref(), cx);
                });
            }
        })
        .context_menu({
            let ws_menu = ctx.ws.clone();
            let per_menu = ctx.persistence.clone();
            let sb_menu = ctx.sidebar.clone();
            let git_menu = ctx.git.clone();
            let r_id_menu = r_id_menu.clone();
            let init_name_menu = initial_name_menu.clone();
            let ed_menu = ctx.ed.clone();
            let toast_menu = ctx.toast.clone();
            let is_pinned = ctx.pins.contains(&r_id_menu);
            let is_dirty_git = req_git_status != SidebarGitStatus::Clean;

            move |menu, _window, cx| {
                let is_in_multi = sb_menu.read(cx).is_multi_selected()
                    && sb_menu.read(cx).is_request_selected(&r_id_menu);

                if is_in_multi {
                    let ws_del = ws_menu.clone();
                    let ed_del = ed_menu.clone();
                    let sb_del = sb_menu.clone();
                    return menu.item(PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
                        move |_, _, cx| {
                            delete_multi_selected_items(&ws_del, &ed_del, &sb_del, cx);
                        },
                    ));
                }
                let ws_r = ws_menu.clone();
                let sb_r = sb_menu.clone();
                let r_id_r = r_id_menu.clone();
                let init_r = init_name_menu.clone();

                let ws_d = ws_menu.clone();
                let ed_d = ed_menu.clone();
                let r_id_d = r_id_menu.clone();

                let ws_del = ws_menu.clone();
                let ed_del = ed_menu.clone();
                let r_id_del = r_id_menu.clone();

                let ws_p = ws_menu.clone();
                let r_id_p = r_id_menu.clone();

                let ws_yaml = ws_menu.clone();
                let r_id_yaml = r_id_menu.clone();
                let ws_curl = ws_menu.clone();
                let r_id_curl = r_id_menu.clone();

                let mut m = menu
                    .item(
                        PopupMenuItem::new("Copy as YAML")
                            .icon(IconName::FileCode)
                            .on_click(move |_, _, cx| {
                                if let Some(r) = ws_yaml.read(cx).find_request(&r_id_yaml) {
                                    let rf = RequestFile::from_item(r);
                                    if let Ok(yaml) = to_yaml_string(&rf) {
                                        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                            yaml,
                                        ));
                                    }
                                }
                            }),
                    )
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
                                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                        curl,
                                    ));
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
                                if let Some(dup) = ws_d
                                    .update(cx, |state, cx| state.duplicate_request(&r_id_d, cx))
                                {
                                    ed_d.update(cx, |state, cx| {
                                        state.set_active_request_id(&dup.id, cx);
                                    });
                                }
                            }),
                    );

                if is_dirty_git {
                    let git_disc = git_menu.clone();
                    let svc_disc = GitService::new(git_menu.clone(), toast_menu.clone());
                    let ws_disc = ws_menu.clone();
                    let per_disc = per_menu.clone();
                    let ed_disc = ed_menu.clone();
                    let r_id_disc = r_id_menu.clone();
                    let toast_disc = toast_menu.clone();
                    let r_name_disc = init_name_menu.clone();

                    m = m.separator().item(
                        PopupMenuItem::new("Discard Changes")
                            .icon(IconName::Undo2)
                            .on_click(move |_, _, cx| {
                                let req_name = r_name_disc.clone();
                                let norm_req = r_id_disc.replace('\\', "/");
                                let coll_rel = format!("collections/{}", norm_req);
                                let path_to_discard = {
                                    let g = git_disc.read(cx);
                                    if let Some(git_st) = &g.status {
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
                                            .or_else(|| {
                                                git_st
                                                    .unstaged_files
                                                    .iter()
                                                    .find(|i| matches(&i.path))
                                            })
                                            .or_else(|| {
                                                git_st
                                                    .staged_files
                                                    .iter()
                                                    .find(|i| matches(&i.path))
                                            })
                                            .map(|i| i.path.clone())
                                            .unwrap_or(coll_rel)
                                    } else {
                                        coll_rel
                                    }
                                };

                                // Never delete never-committed request files: for an
                                // untracked request this only drops unsaved in-memory
                                // edits via mark_clean + reload_all in on_success below.
                                per_disc.update(cx, |per, cx| {
                                    per.cancel_request(&r_id_disc, cx);
                                });
                                let r_id_task = r_id_disc.clone();
                                let per_cb = per_disc.clone();
                                let ws_cb = ws_disc.clone();
                                let ed_cb = ed_disc.clone();
                                let toast_cb = toast_disc.clone();
                                let req_name_cb = req_name.clone();
                                svc_disc.discard_file(&path_to_discard, false, cx, move |cx| {
                                    per_cb.update(cx, |per, cx| {
                                        per.mark_clean(&r_id_task, cx);
                                        if per.conflicts.remove(&r_id_task).is_some() {
                                            cx.notify();
                                        }
                                    });
                                    ws_cb.update(cx, |ws, cx| {
                                        ws.reload_all();
                                        let active_id =
                                            ed_cb.read(cx).active_request_id().to_string();
                                        if active_id == r_id_task {
                                            if ws.find_request(&r_id_task).is_some() {
                                                ed_cb.update(cx, |ed, cx| {
                                                    ed.sync_composer_inputs(
                                                        crate::state::ComposerSyncScope::All,
                                                        cx,
                                                    );
                                                });
                                            } else {
                                                ed_cb.update(cx, |ed, cx| {
                                                    if let Some(first_req) = ws.requests.first() {
                                                        ed.set_active_request_id(&first_req.id, cx);
                                                    } else {
                                                        ed.set_active_request_id("scratch", cx);
                                                    }
                                                });
                                            }
                                        }
                                        cx.notify();
                                    });
                                    toast_cb.update(cx, |ts, cx| {
                                        ts.push_info(
                                            "Git Discard",
                                            format!("Discarded changes in '{}'", req_name_cb),
                                            cx,
                                        );
                                    });
                                });
                            }),
                    );
                }

                m.separator()
                    .item(PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
                        move |_, _, cx| {
                            delete_request_item(&ws_del, &ed_del, &r_id_del, cx);
                        },
                    ))
            }
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .items_center()
                .gap(px(6.))
                .overflow_hidden()
                .child(if has_examples {
                    let sb_toggle = ctx.sidebar.clone();
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
                        .hover(|s| s.text_color(t.text_primary))
                        .text_color(t.text_muted)
                        .child(
                            Icon::new(if is_req_collapsed {
                                IconName::ChevronRight
                            } else {
                                IconName::ChevronDown
                            })
                            .size(px(11.)),
                        )
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            sb_toggle.update(cx, |sb, cx| {
                                sb.toggle_request_collapsed(&r_id, cx);
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
                .child(MethodBadge::new(req.method))
                .child(if let (true, Some(inp)) = (is_renaming, ctx.rename_input) {
                    let sb_esc = ctx.sidebar.clone();
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
                }),
        )
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

pub fn render_request_row(
    req: &SidebarReqItem,
    depth: usize,
    is_req_collapsed: bool,
    req_git_status: SidebarGitStatus,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let pill = render_request_inner_pill(req, is_req_collapsed, req_git_status, "req-item", ctx);
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
