use super::drag_drop::{DragFolderItem, DragMultiItem, DragRequestItem};
use super::git_status::SidebarGitStatus;
use super::item_actions::delete_multi_selected_items;
use super::{SidebarRenderContext, SIDEBAR_ROW_HEIGHT};
use crate::app::actions::DismissModal;
use crate::state::ScopeSettingsTarget;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, AppContext as _, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement as _, Styled,
};

pub fn render_folder_row(
    folder_id: &str,
    folder_name: &str,
    depth: usize,
    is_collapsed: bool,
    folder_git_status: SidebarGitStatus,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let t = theme();
    let folder_color = match folder_git_status {
        SidebarGitStatus::New => t.git_added,
        SidebarGitStatus::Modified => t.git_modified,
        SidebarGitStatus::Clean => t.text_primary,
    };

    let f_id = folder_id.to_string();
    let is_renaming_folder = ctx.renaming_folder_id == Some(folder_id);
    let f_name_str = folder_name.to_string();
    let is_folder_selected = ctx.selected_folders.contains(folder_id);
    let is_in_multi_set = is_folder_selected && ctx.is_multi_select;

    let drag_folder_payload = DragFolderItem {
        id: f_id.clone(),
        name: f_name_str.clone(),
    };
    let multi_drag_payload = if is_in_multi_set {
        Some(DragMultiItem::new(
            ctx.selected_requests.iter().cloned().collect(),
            ctx.selected_folders.iter().cloned().collect(),
        ))
    } else {
        None
    };

    let ws_drop_req = ctx.ws.clone();
    let f_id_target_req = f_id.clone();
    let ws_drop_folder = ctx.ws.clone();
    let f_id_target_folder = f_id.clone();
    let ws_drop_multi = ctx.ws.clone();
    let f_id_target_multi = f_id.clone();

    let header = div()
        .id(format!("folder-header-{}", f_id))
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_between()
        .px(px(6.))
        .rounded(px(4.))
        .border_1()
        .border_color(if is_folder_selected {
            t.border
        } else {
            t.transparent
        })
        .bg(if is_folder_selected {
            t.bg_selected
        } else {
            t.transparent
        })
        .text_color(t.text_secondary)
        .text_xs()
        .cursor_pointer()
        .hover(|s| s.bg(t.bg_hover))
        .when(!is_renaming_folder, move |this| {
            let this = if let Some(multi_payload) = multi_drag_payload {
                this.on_drag(multi_payload, |item, _, _, cx| cx.new(|_| item.clone()))
            } else {
                this.on_drag(drag_folder_payload, |item, _, _, cx| {
                    cx.new(|_| item.clone())
                })
            };
            this.drag_over::<DragRequestItem>(|style, _, _, _| {
                let t = theme();
                style.bg(t.bg_hover).border_1().border_color(t.border_focus)
            })
            .on_drop(move |drag: &DragRequestItem, _, cx| {
                ws_drop_req.update(cx, |state, cx| {
                    state.move_request_to_folder(&drag.id, Some(f_id_target_req.clone()), cx);
                });
            })
            .drag_over::<DragFolderItem>(|style, _, _, _| {
                let t = theme();
                style
                    .bg(t.bg_hover)
                    .border_t_2()
                    .border_color(t.border_focus)
            })
            .on_drop(move |drag: &DragFolderItem, _, cx| {
                ws_drop_folder.update(cx, |state, cx| {
                    state.move_folder(&drag.id, Some(f_id_target_folder.clone()), cx);
                });
            })
            .drag_over::<DragMultiItem>(|style, _, _, _| {
                let t = theme();
                style.bg(t.bg_hover).border_1().border_color(t.border_focus)
            })
            .on_drop(move |drag: &DragMultiItem, _, cx| {
                let target_f_id = f_id_target_multi.clone();
                ws_drop_multi.update(cx, |state, cx| {
                    for req_id in &drag.request_ids {
                        state.move_request_to_folder(req_id, Some(target_f_id.clone()), cx);
                    }
                    for fold_id in &drag.folder_ids {
                        if fold_id != &target_f_id {
                            state.move_folder(fold_id, Some(target_f_id.clone()), cx);
                        }
                    }
                });
            })
        })
        .on_mouse_down(MouseButton::Right, {
            let sb_right = ctx.sidebar.clone();
            let f_id_right = f_id.clone();
            move |_, _, cx| {
                let is_already_selected = sb_right.read(cx).is_folder_selected(&f_id_right);
                let has_multi = sb_right.read(cx).is_multi_selected();
                if has_multi && is_already_selected {
                    // Keep multi-selection intact
                } else {
                    sb_right.update(cx, |state, cx| {
                        state.select_single_folder(&f_id_right, cx);
                    });
                }
            }
        })
        .on_click({
            let sb_f_click = ctx.sidebar.clone();
            let ws_f_click = ctx.ws.clone();
            let ed_f_click = ctx.ed.clone();
            let f_id_click = f_id.clone();
            let f_name_click = f_name_str.clone();
            move |ev, window, cx| {
                if is_renaming_folder {
                    return;
                }
                let mods = window.modifiers();
                let is_ctrl_or_cmd = mods.control || mods.platform;
                if is_ctrl_or_cmd {
                    let current_active = ed_f_click.read(cx).active_request_id.clone();
                    sb_f_click.update(cx, |state, cx| {
                        state.toggle_select_folder(&f_id_click, current_active.as_deref(), cx);
                    });
                    return;
                }

                if ev.click_count() > 1 {
                    let ws_c = ws_f_click.clone();
                    sb_f_click.update(cx, |state, cx| {
                        state.start_renaming_folder(
                            f_id_click.clone(),
                            f_name_click.clone(),
                            ws_c,
                            window,
                            cx,
                        );
                    });
                } else {
                    sb_f_click.update(cx, |state, cx| {
                        state.clear_selection(cx);
                        state.toggle_folder_collapsed(&f_id_click, cx);
                    });
                }
            }
        })
        .context_menu({
            let ws_new = ctx.ws.clone();
            let ed_new = ctx.ed.clone();
            let f_id_new = f_id.clone();

            let ws_new_sub = ctx.ws.clone();
            let sb_new_sub = ctx.sidebar.clone();
            let f_id_new_sub = f_id.clone();

            let ws_ren = ctx.ws.clone();
            let sb_ren = ctx.sidebar.clone();
            let f_id_ren = f_id.clone();
            let f_name_ren = f_name_str.clone();

            let ws_del = ctx.ws.clone();
            let f_id_del = f_id.clone();

            let modal_settings = ctx.modal.clone();
            let f_id_settings = f_id.clone();

            let sb_menu = ctx.sidebar.clone();
            let ed_menu = ctx.ed.clone();
            let f_id_menu = f_id.clone();

            move |menu, _window, cx| {
                let is_in_multi = sb_menu.read(cx).is_multi_selected()
                    && sb_menu.read(cx).is_folder_selected(&f_id_menu);

                if is_in_multi {
                    let ws_d = ws_del.clone();
                    let ed_d = ed_menu.clone();
                    let sb_d = sb_menu.clone();
                    return menu.item(
                        gpui_kit::component::menu::PopupMenuItem::new("Delete")
                            .icon(IconName::Trash)
                            .on_click(move |_, _, cx| {
                                delete_multi_selected_items(&ws_d, &ed_d, &sb_d, cx);
                            }),
                    );
                }
                let ws_n = ws_new.clone();
                let ed_n = ed_new.clone();
                let f_id_n = f_id_new.clone();

                let ws_ns = ws_new_sub.clone();
                let sb_ns = sb_new_sub.clone();
                let f_id_ns = f_id_new_sub.clone();

                let ws_r = ws_ren.clone();
                let sb_r = sb_ren.clone();
                let f_id_r = f_id_ren.clone();
                let f_name_r = f_name_ren.clone();

                let modal_set = modal_settings.clone();
                let f_id_set = f_id_settings.clone();

                let ws_d = ws_del.clone();
                let f_id_d = f_id_del.clone();

                menu.item(
                    gpui_kit::component::menu::PopupMenuItem::new("New Request")
                        .icon(IconName::Plus)
                        .on_click(move |_, _, cx| {
                            let req = ws_n.update(cx, |state, cx| {
                                state.create_request(Some(f_id_n.clone()), cx)
                            });
                            ed_n.update(cx, |state, cx| {
                                state.set_active_request_id(&req.id, cx);
                            });
                        }),
                )
                .item(
                    gpui_kit::component::menu::PopupMenuItem::new("New Folder")
                        .icon(IconName::FolderPlus)
                        .on_click(move |_, window, cx| {
                            let ws_c = ws_ns.clone();
                            let created = ws_ns.update(cx, |state, cx| {
                                state.create_folder(
                                    Some(f_id_ns.clone()),
                                    "new-folder".to_string(),
                                    cx,
                                )
                            });
                            if let Some(new_id) = created {
                                sb_ns.update(cx, |state, cx| {
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
                .item(
                    gpui_kit::component::menu::PopupMenuItem::new("Rename")
                        .icon(IconName::Pencil)
                        .on_click(move |_, window, cx| {
                            let ws_c = ws_r.clone();
                            sb_r.update(cx, |state, cx| {
                                state.start_renaming_folder(
                                    f_id_r.clone(),
                                    f_name_r.clone(),
                                    ws_c,
                                    window,
                                    cx,
                                );
                            });
                        }),
                )
                .item(
                    gpui_kit::component::menu::PopupMenuItem::new("Folder Settings...")
                        .icon(IconName::Settings)
                        .on_click(move |_, _, cx| {
                            modal_set.update(cx, |state, cx| {
                                state.open_scope_settings(
                                    ScopeSettingsTarget::Folder(f_id_set.clone()),
                                    cx,
                                );
                            });
                        }),
                )
                .separator()
                .item(
                    gpui_kit::component::menu::PopupMenuItem::new("Delete")
                        .icon(IconName::Trash)
                        .on_click(move |_, _, cx| {
                            ws_d.update(cx, |state, cx| {
                                state.delete_folder(&f_id_d, cx);
                            });
                        }),
                )
            }
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
                    Icon::new(if is_collapsed {
                        IconName::FolderClosed
                    } else {
                        IconName::FolderOpen
                    })
                    .size(px(14.))
                    .text_color(folder_color),
                )
                .child(
                    if let (true, Some(inp)) = (is_renaming_folder, ctx.rename_folder_input) {
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
                                    state.cancel_renaming_folder(cx);
                                });
                            })
                            .child(
                                gpui_kit::component::input::Input::new(inp)
                                    .id(format!("rename-folder-field-{}", folder_id))
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
                            .text_color(folder_color)
                            .truncate()
                            .child(f_name_str)
                            .into_any_element()
                    },
                ),
        );

    div()
        .w_full()
        .h(px(SIDEBAR_ROW_HEIGHT))
        .pl(px(6.0 + depth as f32 * 14.0))
        .pr(px(6.))
        .py(px(1.))
        .flex()
        .items_center()
        .child(header)
        .into_any_element()
}
