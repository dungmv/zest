use super::drag_drop::{DragFolderItem, DragRequestItem};
use super::git_status::SidebarGitStatus;
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
    let git_green: gpui_kit::Hsla = gpui_kit::rgb(0x4ade80).into();
    let git_yellow: gpui_kit::Hsla = gpui_kit::rgb(0xfacc15).into();

    let folder_color = match folder_git_status {
        SidebarGitStatus::New => git_green,
        SidebarGitStatus::Modified => git_yellow,
        SidebarGitStatus::Clean => t.text_primary,
    };

    let f_id = folder_id.to_string();
    let is_renaming_folder = ctx.renaming_folder_id == Some(folder_id);
    let f_name_str = folder_name.to_string();

    let drag_folder_payload = DragFolderItem {
        id: f_id.clone(),
        name: f_name_str.clone(),
    };
    let ws_drop_req = ctx.ws.clone();
    let f_id_target_req = f_id.clone();
    let ws_drop_folder = ctx.ws.clone();
    let f_id_target_folder = f_id.clone();

    let header = div()
        .id(format!("folder-header-{}", f_id))
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_between()
        .px(px(6.))
        .rounded(px(4.))
        .text_color(t.text_secondary)
        .text_xs()
        .cursor_pointer()
        .hover(|s| s.bg(t.bg_hover))
        .when(!is_renaming_folder, move |this| {
            this.on_drag(drag_folder_payload, |item, _, _, cx| {
                cx.new(|_| item.clone())
            })
        })
        .drag_over::<DragRequestItem>(|style, _, _, _| {
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
        .on_click({
            let sb_f_click = ctx.sidebar.clone();
            let ws_f_click = ctx.ws.clone();
            let f_id_click = f_id.clone();
            let f_name_click = f_name_str.clone();
            move |ev, window, cx| {
                if is_renaming_folder {
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

            move |menu, _window, _cx| {
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
                    gpui_kit::component::menu::PopupMenuItem::new("New Request").on_click(
                        move |_, _, cx| {
                            let req = ws_n.update(cx, |state, cx| {
                                state.create_request(Some(f_id_n.clone()), cx)
                            });
                            ed_n.update(cx, |state, cx| {
                                state.set_active_request_id(&req.id, cx);
                            });
                        },
                    ),
                )
                .item(
                    gpui_kit::component::menu::PopupMenuItem::new("New Folder").on_click(
                        move |_, window, cx| {
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
                        },
                    ),
                )
                .item(
                    gpui_kit::component::menu::PopupMenuItem::new("Rename").on_click(
                        move |_, window, cx| {
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
                        },
                    ),
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
                    gpui_kit::component::menu::PopupMenuItem::new("Delete").on_click(
                        move |_, _, cx| {
                            ws_d.update(cx, |state, cx| {
                                state.delete_folder(&f_id_d, cx);
                            });
                        },
                    ),
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
