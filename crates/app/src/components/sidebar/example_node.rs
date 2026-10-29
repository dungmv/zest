use super::request_node::SidebarExampleItem;
use super::{SidebarRenderContext, SIDEBAR_ROW_HEIGHT};
use crate::app::actions::DismissModal;
use crate::components::StatusCodeBadge;
use crate::theme::theme;
use gpui_kit::{
    div, px, AnyElement, AppContext as _, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement as _, Styled,
};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::Sizable as _;

pub fn render_example_row(
    parent_id: &str,
    idx: usize,
    ex: &SidebarExampleItem,
    depth: usize,
    total_ex: usize,
    ctx: &SidebarRenderContext,
) -> AnyElement {
    let t = theme();
    let is_req_active = ctx.active_req_id == Some(parent_id);
    let is_this_ex_active = is_req_active && ctx.active_example_name == Some(ex.name.as_str());
    let is_renaming_this_ex = ctx
        .renaming_example
        .map(|(r_id, ex_name)| r_id == parent_id && ex_name == &ex.name)
        .unwrap_or(false);
    let ed_ex_click = ctx.ed.clone();
    let resp_ex_click = ctx.resp.clone();
    let ws_ex_click = ctx.ws.clone();
    let sb_ex_click = ctx.sidebar.clone();
    let r_id_for_click = parent_id.to_string();
    let ex_name_for_click = ex.name.clone();

    let inner = div()
        .id(format!("req-{}-ex-{}", parent_id, idx))
        .w_full()
        .h_full()
        .px(px(6.))
        .rounded(px(5.))
        .border_1()
        .border_color(if is_this_ex_active {
            t.border
        } else {
            t.transparent
        })
        .bg(if is_this_ex_active {
            t.bg_selected
        } else {
            t.transparent
        })
        .hover(|s| s.bg(t.bg_hover))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_between()
        .on_click(move |ev, window, cx| {
            if is_renaming_this_ex {
                return;
            }
            let r_id = r_id_for_click.clone();
            let ex_name = ex_name_for_click.clone();
            if ev.click_count() > 1 {
                let ws_c = ws_ex_click.clone();
                sb_ex_click.update(cx, |state, cx| {
                    state.start_renaming_example(r_id.clone(), ex_name.clone(), ws_c, window, cx);
                });
                ed_ex_click.update(cx, |state, cx| {
                    state.set_active_request_id(&r_id, cx);
                });
                if let Some(rf) = ws_ex_click.read(cx).get_request_file(&r_id) {
                    if let Some(ex_file) = rf.examples.iter().find(|e| e.name == ex_name) {
                        resp_ex_click.update(cx, |state, cx| {
                            state.view_example(ex_file, cx);
                        });
                    }
                }
            } else {
                sb_ex_click.update(cx, |state, _cx| {
                    state.record_navigation(&r_id);
                });
                ed_ex_click.update(cx, |state, cx| {
                    state.set_active_request_id(&r_id, cx);
                });
                if let Some(rf) = ws_ex_click.read(cx).get_request_file(&r_id) {
                    if let Some(ex_file) = rf.examples.iter().find(|e| e.name == ex_name) {
                        resp_ex_click.update(cx, |state, cx| {
                            state.view_example(ex_file, cx);
                        });
                    }
                }
            }
        })
        .context_menu({
            let ws_ctx = ctx.ws.clone();
            let sb_ctx = ctx.sidebar.clone();
            let resp_ctx = ctx.resp.clone();
            let r_id_ctx = parent_id.to_string();
            let ex_name_ctx = ex.name.clone();

            move |menu, _window, _cx| {
                let ws_ren = ws_ctx.clone();
                let sb_ren = sb_ctx.clone();
                let r_id_ren = r_id_ctx.clone();
                let ex_ren_name = ex_name_ctx.clone();

                let ws_dup = ws_ctx.clone();
                let r_id_dup = r_id_ctx.clone();
                let ex_dup_name = ex_name_ctx.clone();

                let ws_del = ws_ctx.clone();
                let resp_del = resp_ctx.clone();
                let r_id_del = r_id_ctx.clone();
                let ex_del_name = ex_name_ctx.clone();

                let mut m = menu
                    .item(gpui_kit::component::menu::PopupMenuItem::new("Rename").on_click(
                        move |_, window, cx| {
                            let ws_c = ws_ren.clone();
                            sb_ren.update(cx, |state, cx| {
                                state.start_renaming_example(
                                    r_id_ren.clone(),
                                    ex_ren_name.clone(),
                                    ws_c,
                                    window,
                                    cx,
                                );
                            });
                        },
                    ))
                    .item(
                        gpui_kit::component::menu::PopupMenuItem::new("Duplicate").on_click(
                            move |_, _, cx| {
                                ws_dup.update(cx, |ws, cx| {
                                    if let Some(mut rf) = ws.get_request_file(&r_id_dup) {
                                        if let Some(orig_ex) = rf
                                            .examples
                                            .iter()
                                            .find(|e| e.name == ex_dup_name)
                                            .cloned()
                                        {
                                            let base_name = orig_ex.name.clone();
                                            let mut name = format!("{}-copy", base_name);
                                            let mut i = 2;
                                            while rf.examples.iter().any(|e| e.name == name) {
                                                name = format!("{}-copy-{}", base_name, i);
                                                i += 1;
                                            }
                                            let mut new_ex = orig_ex;
                                            new_ex.name = name;
                                            rf.examples.push(new_ex);
                                            ws.save_request_file(&r_id_dup, &rf, cx);
                                        }
                                    }
                                });
                            },
                        ),
                    );

                if idx > 0 {
                    let ws_up = ws_ctx.clone();
                    let r_id_up = r_id_ctx.clone();
                    m = m.item(
                        gpui_kit::component::menu::PopupMenuItem::new("Move Up").on_click(
                            move |_, _, cx| {
                                ws_up.update(cx, |ws, cx| {
                                    if let Some(mut rf) = ws.get_request_file(&r_id_up) {
                                        if idx < rf.examples.len() {
                                            rf.examples.swap(idx, idx - 1);
                                            ws.save_request_file(&r_id_up, &rf, cx);
                                        }
                                    }
                                });
                            },
                        ),
                    );
                }

                if idx + 1 < total_ex {
                    let ws_down = ws_ctx.clone();
                    let r_id_down = r_id_ctx.clone();
                    m = m.item(
                        gpui_kit::component::menu::PopupMenuItem::new("Move Down").on_click(
                            move |_, _, cx| {
                                ws_down.update(cx, |ws, cx| {
                                    if let Some(mut rf) = ws.get_request_file(&r_id_down) {
                                        if idx + 1 < rf.examples.len() {
                                            rf.examples.swap(idx, idx + 1);
                                            ws.save_request_file(&r_id_down, &rf, cx);
                                        }
                                    }
                                });
                            },
                        ),
                    );
                }

                m.separator()
                    .item(gpui_kit::component::menu::PopupMenuItem::new("Delete").on_click(
                        move |_, _, cx| {
                            ws_del.update(cx, |ws, cx| {
                                if let Some(mut rf) = ws.get_request_file(&r_id_del) {
                                    rf.examples.retain(|e| e.name != ex_del_name);
                                    ws.save_request_file(&r_id_del, &rf, cx);
                                }
                                if is_this_ex_active {
                                    let store = ws.store.clone();
                                    resp_del.update(cx, |s, cx| {
                                        s.load_for_request(&r_id_del, store.as_ref(), cx);
                                    });
                                }
                            });
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
                .child(div().text_xs().text_color(t.text_muted).child("↳"))
                .child(
                    if let (true, Some(inp)) = (is_renaming_this_ex, ctx.rename_example_input) {
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
                                    state.cancel_renaming_example(cx);
                                });
                            })
                            .child(
                                gpui_kit::component::input::Input::new(inp)
                                    .id(format!("rename-ex-input-{}-{}", parent_id, idx))
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .cleanable(false)
                                    .xsmall()
                                    .font_family(".SystemMonoFont"),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .text_xs()
                            .font_family(".SystemMonoFont")
                            .text_color(if is_this_ex_active {
                                t.text_primary
                            } else {
                                t.text_secondary
                            })
                            .truncate()
                            .child(ex.name.clone())
                            .into_any_element()
                    },
                ),
        )
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(StatusCodeBadge::new(ex.status)),
        );

    div()
        .w_full()
        .h(px(SIDEBAR_ROW_HEIGHT))
        .pl(px(6.0 + depth as f32 * 14.0))
        .pr(px(8.))
        .py(px(1.))
        .flex()
        .items_center()
        .child(inner)
        .into_any_element()
}
