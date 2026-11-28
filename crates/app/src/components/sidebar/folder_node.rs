use super::drag_drop::{DragFolderItem, DragMultiItem, DragRequestItem};
use super::git_status::SidebarGitStatus;
use super::item_actions::delete_multi_selected_items;
use super::{SidebarRenderContext, SIDEBAR_ROW_HEIGHT};
use crate::app::actions::DismissModal;
use crate::state::{
    CollectionState, ModalKind, ModalPayload, ModalState, RequestSessionState, SidebarState,
    WorkspaceSettingTarget,
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
use zest_core::models::Protocol;

pub fn render_folder_row(
    folder_id: &str,
    folder_name: &str,
    depth: usize,
    is_collapsed: bool,
    folder_git_status: SidebarGitStatus,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let t = theme();
    let f_id = folder_id.to_string();
    let is_renaming_folder = ctx.renaming.folder_id == Some(folder_id);
    let f_name_str = folder_name.to_string();
    let is_folder_selected = ctx.selection.selected_folders.contains(folder_id);
    let is_in_multi_set = is_folder_selected && ctx.selection.is_multi_select;
    let folder_name_color = match folder_git_status {
        SidebarGitStatus::New => t.git_added,
        SidebarGitStatus::Modified => t.git_modified,
        SidebarGitStatus::Clean if is_folder_selected => t.text_primary,
        SidebarGitStatus::Clean => t.sidebar_foreground,
    };

    let drag_folder_payload = DragFolderItem {
        id: f_id.clone(),
        name: f_name_str.clone(),
    };
    let multi_drag_payload = is_in_multi_set.then(|| {
        DragMultiItem::new(
            ctx.selection.selected_requests.iter().cloned().collect(),
            ctx.selection.selected_folders.iter().cloned().collect(),
        )
    });

    let ws_drop_req = ctx.entities.ws.clone();
    let f_id_target_req = f_id.clone();
    let ws_drop_folder = ctx.entities.ws.clone();
    let f_id_target_folder = f_id.clone();
    let ws_drop_multi = ctx.entities.ws.clone();
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
        .when(!is_folder_selected, |row| row.hover(|s| s.bg(t.bg_hover)))
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
            let sb_right = ctx.entities.sidebar.clone();
            let f_id_right = f_id.clone();
            move |_, _, cx| {
                let is_already_selected = sb_right.read(cx).is_folder_selected(&f_id_right);
                let has_multi = sb_right.read(cx).is_multi_selected();
                if !(has_multi && is_already_selected) {
                    sb_right.update(cx, |state, cx| {
                        state.select_single_folder(&f_id_right, cx);
                    });
                }
            }
        })
        .on_click({
            let sb_f_click = ctx.entities.sidebar.clone();
            let ws_f_click = ctx.entities.ws.clone();
            let ed_f_click = ctx.entities.ed.clone();
            let f_id_click = f_id.clone();
            let f_name_click = f_name_str.clone();
            move |ev, window, cx| {
                if is_renaming_folder {
                    return;
                }
                let mods = window.modifiers();
                if mods.control || mods.platform {
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
            let ws = ctx.entities.ws.clone();
            let ed = ctx.entities.ed.clone();
            let sb = ctx.entities.sidebar.clone();
            let modal = ctx.entities.modal.clone();
            let f_id_menu = f_id.clone();
            let f_name_menu = f_name_str.clone();
            move |menu, _window, cx| {
                build_folder_context_menu(menu, &f_id_menu, &f_name_menu, &ws, &ed, &sb, &modal, cx)
            }
        })
        .child(render_folder_label(
            folder_id,
            &f_name_str,
            is_collapsed,
            is_renaming_folder,
            folder_name_color,
            ctx,
        ));

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

#[allow(clippy::too_many_arguments)]
fn build_folder_context_menu(
    menu: PopupMenu,
    folder_id: &str,
    folder_name: &str,
    ws: &Entity<CollectionState>,
    ed: &Entity<RequestSessionState>,
    sb: &Entity<SidebarState>,
    modal: &Entity<ModalState>,
    cx: &App,
) -> PopupMenu {
    let is_in_multi = sb.read(cx).is_multi_selected() && sb.read(cx).is_folder_selected(folder_id);
    if is_in_multi {
        let ws_d = ws.clone();
        let ed_d = ed.clone();
        let sb_d = sb.clone();
        return menu.item(PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
            move |_, _, cx| {
                delete_multi_selected_items(&ws_d, &ed_d, &sb_d, cx);
            },
        ));
    }

    let add_protocol_item =
        |m: PopupMenu, label: &'static str, icon: IconName, proto: Protocol| {
            let ws_p = ws.clone();
            let ed_p = ed.clone();
            let sb_p = sb.clone();
            let f_id_p = folder_id.to_string();
            m.item(PopupMenuItem::new(label).icon(Icon::new(icon)).on_click(
                move |_, window, cx| {
                    sb_p.update(cx, |state, cx| {
                        state.expand_folder(&f_id_p, cx);
                    });
                    super::item_actions::create_and_rename_request(
                        proto,
                        Some(f_id_p.clone()),
                        &ws_p,
                        &ed_p,
                        &sb_p,
                        window,
                        cx,
                    );
                },
            ))
        };

    let m = add_protocol_item(menu, "New HTTP", IconName::Globe, Protocol::Http);
    let m = add_protocol_item(
        m,
        "New WebSocket",
        IconName::ArrowLeftRight,
        Protocol::WebSocket,
    );
    let m = add_protocol_item(m, "New gRPC", IconName::Network, Protocol::Grpc);
    let m = add_protocol_item(m, "New GraphQL", IconName::ChartNetwork, Protocol::GraphQL);
    let m = add_protocol_item(m, "New SSE", IconName::Zap, Protocol::Sse);

    let ws_ns = ws.clone();
    let sb_ns = sb.clone();
    let f_id_ns = folder_id.to_string();

    let ws_r = ws.clone();
    let sb_r = sb.clone();
    let f_id_r = folder_id.to_string();
    let f_name_r = folder_name.to_string();

    let modal_set = modal.clone();
    let f_id_set = folder_id.to_string();

    let modal_exp = modal.clone();
    let f_id_exp = folder_id.to_string();
    let f_name_exp = folder_name.to_string();

    let ws_d = ws.clone();
    let f_id_d = folder_id.to_string();

    m.item(
        PopupMenuItem::new("New Folder")
            .icon(IconName::FolderPlus)
            .on_click(move |_, window, cx| {
                super::item_actions::create_and_rename_folder(
                    Some(f_id_ns.clone()),
                    &ws_ns,
                    &sb_ns,
                    window,
                    cx,
                );
            }),
    )
    .item(
        PopupMenuItem::new("Rename")
            .icon(IconName::Pencil)
            .on_click(move |_, window, cx| {
                let ws_c = ws_r.clone();
                sb_r.update(cx, |state, cx| {
                    state.start_renaming_folder(f_id_r.clone(), f_name_r.clone(), ws_c, window, cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new("Folder Settings...")
            .icon(IconName::Settings)
            .on_click(move |_, _, cx| {
                modal_set.update(cx, |state, cx| {
                    state.open(
                        ModalKind::WorkspaceSetting,
                        Some(ModalPayload::WorkspaceSetting(
                            WorkspaceSettingTarget::Folder(f_id_set.clone()),
                        )),
                        cx,
                    );
                });
            }),
    )
    .item(
        PopupMenuItem::new("Export OpenAPI...")
            .icon(IconName::Upload)
            .on_click(move |_, _, cx| {
                modal_exp.update(cx, |state, cx| {
                    state.open(
                        ModalKind::Export,
                        Some(ModalPayload::Export {
                            folder: Some((f_id_exp.clone(), f_name_exp.clone())),
                        }),
                        cx,
                    );
                });
            }),
    )
    .separator()
    .item(
        PopupMenuItem::new("Delete")
            .icon(IconName::Trash)
            .on_click(move |_, _, cx| {
                ws_d.update(cx, |state, cx| {
                    state.delete_folder(&f_id_d, cx);
                });
            }),
    )
}

fn render_folder_label(
    folder_id: &str,
    folder_name: &str,
    is_collapsed: bool,
    is_renaming_folder: bool,
    folder_name_color: Hsla,
    ctx: &SidebarRenderContext,
) -> gpui_kit::Div {
    let t = theme();
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
                .text_color(t.icon_muted)
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
            .text_color(t.icon_muted),
        )
        .child(
            if let (true, Some(inp)) = (is_renaming_folder, ctx.renaming.folder_input) {
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
                    .text_color(folder_name_color)
                    .truncate()
                    .child(folder_name.to_string())
                    .into_any_element()
            },
        )
}
