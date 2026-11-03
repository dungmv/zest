use crate::app::actions::DismissModal;
use crate::state::{EnvironmentState, ModalState, SidebarState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, Div, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement as _, Styled,
};
use zest_core::models::Environment;

pub(super) struct EnvListProps {
    pub env_state: Entity<EnvironmentState>,
    pub modal_state: Entity<ModalState>,
    pub sidebar_state: Entity<SidebarState>,
    pub selected_id: String,
    pub is_global_selected: bool,
    pub global_var_count: usize,
    pub environments: Vec<Environment>,
    pub renaming_env_id: Option<String>,
    pub rename_env_input: Option<Entity<InputState>>,
}

pub(super) fn render_env_list_sidebar(props: EnvListProps) -> Div {
    let t = theme();
    let ws_clone = props.env_state;
    let modal_clone = props.modal_state;
    let sidebar_clone = props.sidebar_state;
    let selected_id = props.selected_id;
    let is_global_selected = props.is_global_selected;
    let global_var_count = props.global_var_count;
    let envs = props.environments;
    let renaming_env_id = props.renaming_env_id;
    let rename_env_input = props.rename_env_input;

    div()
        .w(px(260.))
        .h_full()
        .border_r_1()
        .border_color(t.border)
        .bg(t.bg_sidebar)
        .flex()
        .flex_col()
        // Global Environment section
        .child(
            div()
                .px(px(8.))
                .pt(px(8.))
                .pb(px(4.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .px(px(6.))
                        .py(px(2.))
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(t.text_muted)
                        .child("GLOBAL"),
                )
                .child({
                    let modal_s = modal_clone.clone();
                    let is_sel = is_global_selected;
                    let count = global_var_count;
                    div()
                        .id("env-item-globals")
                        .px(px(8.))
                        .py(px(6.))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .bg(if is_sel {
                            t.bg_selected
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .hover(|s| s.bg(if is_sel { t.bg_selected } else { t.bg_hover }))
                        .flex()
                        .items_center()
                        .justify_between()
                        .on_click(move |_, _, cx| {
                            modal_s.update(cx, |state, cx| {
                                state.select_modal_environment(Some("globals".to_string()), cx);
                            });
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(Icon::new(IconName::Globe).size(px(12.)))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(if is_sel {
                                            gpui_kit::FontWeight::SEMIBOLD
                                        } else {
                                            gpui_kit::FontWeight::NORMAL
                                        })
                                        .text_color(if is_sel {
                                            t.text_primary
                                        } else {
                                            t.text_secondary
                                        })
                                        .child("Global Variables"),
                                ),
                        )
                        .child(
                            div()
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(10.))
                                .bg(t.bg_card)
                                .border_1()
                                .border_color(t.border)
                                .text_xs()
                                .text_color(t.text_muted)
                                .child(format!("{}", count)),
                        )
                }),
        )
        // Environments Section Header
        .child(
            div()
                .px(px(14.))
                .pt(px(10.))
                .pb(px(4.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(t.text_muted)
                        .child(format!("ENVIRONMENTS ({})", envs.len())),
                )
                .child({
                    let ws_add = ws_clone.clone();
                    div()
                        .id("btn-new-custom-env")
                        .cursor_pointer()
                        .p(px(2.))
                        .rounded(px(4.))
                        .text_color(t.text_muted)
                        .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                        .on_click(move |_, _, cx| {
                            ws_add.update(cx, |state, cx| {
                                state.create_environment(None, cx);
                            });
                        })
                        .child(Icon::new(IconName::Plus).size(px(14.)))
                }),
        )
        // Custom environments scrollable list
        .child(
            div()
                .flex_1()
                .px(px(8.))
                .overflow_y_scrollbar()
                .flex()
                .flex_col()
                .gap(px(2.))
                .when(envs.is_empty(), |this| {
                    this.child(
                        div()
                            .py(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child("No environments found"),
                    )
                })
                .children(envs.into_iter().map(|env| {
                    let is_sel = selected_id == env.id;
                    let is_renaming = renaming_env_id.as_deref() == Some(&env.id);
                    let env_id = env.id.clone();
                    let env_name = env.name.clone();
                    let env_id_click = env.id.clone();
                    let env_name_click = env.name.clone();
                    let count = env.variables.len();

                    let modal_sel = modal_clone.clone();
                    let sidebar_sel = sidebar_clone.clone();
                    let ws_sel = ws_clone.clone();

                    div()
                        .id(SharedString::from(format!("env-sidebar-item-{}", env.id)))
                        .px(px(8.))
                        .py(px(6.))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .bg(if is_sel {
                            t.bg_selected
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .hover(|s| s.bg(if is_sel { t.bg_selected } else { t.bg_hover }))
                        .flex()
                        .items_center()
                        .justify_between()
                        .on_click(move |ev, window, cx| {
                            if is_renaming {
                                return;
                            }
                            if ev.click_count() > 1 {
                                let cid = env_id_click.clone();
                                let cname = env_name_click.clone();
                                let ws_c = ws_sel.clone();
                                sidebar_sel.update(cx, |state, cx| {
                                    state.start_renaming_environment(cid, cname, ws_c, window, cx);
                                });
                            } else {
                                modal_sel.update(cx, |state, cx| {
                                    state.select_modal_environment(Some(env_id_click.clone()), cx);
                                });
                            }
                        })
                        .context_menu({
                            let sb_ren = sidebar_clone.clone();
                            let ws_r_ws = ws_clone.clone();
                            let ren_id = env.id.clone();
                            let ren_name = env.name.clone();
                            let ws_du = ws_clone.clone();
                            let du_id = env.id.clone();
                            let ws_de = ws_clone.clone();
                            let del_id = env.id.clone();

                            move |menu, _window, _cx| {
                                let sb_r = sb_ren.clone();
                                let ws_r = ws_r_ws.clone();
                                let r_id = ren_id.clone();
                                let r_name = ren_name.clone();
                                let ws_d = ws_du.clone();
                                let d_id = du_id.clone();
                                let ws_delete = ws_de.clone();
                                let id_delete = del_id.clone();

                                menu.item(PopupMenuItem::new("Rename").on_click(
                                    move |_, window, cx| {
                                        let ws_c = ws_r.clone();
                                        sb_r.update(cx, |state, cx| {
                                            state.start_renaming_environment(
                                                r_id.clone(),
                                                r_name.clone(),
                                                ws_c,
                                                window,
                                                cx,
                                            );
                                        });
                                    },
                                ))
                                .item(PopupMenuItem::new("Duplicate").on_click(move |_, _, cx| {
                                    ws_d.update(cx, |state, cx| {
                                        state.duplicate_environment(&d_id, cx);
                                    });
                                }))
                                .separator()
                                .item(
                                    PopupMenuItem::new("Delete").on_click(move |_, _, cx| {
                                        ws_delete.update(cx, |state, cx| {
                                            state.delete_environment(&id_delete, cx);
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
                                .overflow_hidden()
                                .child(
                                    if let (true, Some(inp)) =
                                        (is_renaming, rename_env_input.as_ref())
                                    {
                                        let sb_esc = sidebar_clone.clone();
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
                                                    state.cancel_renaming_environment(cx);
                                                });
                                            })
                                            .child(
                                                Input::new(inp)
                                                    .id(format!(
                                                        "rename-env-input-field-{}",
                                                        env_id
                                                    ))
                                                    .appearance(false)
                                                    .bordered(false)
                                                    .focus_bordered(false)
                                                    .cleanable(false)
                                                    .xsmall()
                                                    .font_weight(if is_sel {
                                                        gpui_kit::FontWeight::SEMIBOLD
                                                    } else {
                                                        gpui_kit::FontWeight::NORMAL
                                                    }),
                                            )
                                            .into_any_element()
                                    } else {
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .font_weight(if is_sel {
                                                gpui_kit::FontWeight::SEMIBOLD
                                            } else {
                                                gpui_kit::FontWeight::NORMAL
                                            })
                                            .text_color(if is_sel {
                                                t.text_primary
                                            } else {
                                                t.text_secondary
                                            })
                                            .child(env_name)
                                            .into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div().flex().items_center().gap(px(4.)).child(
                                div()
                                    .px(px(5.))
                                    .py(px(1.))
                                    .rounded(px(8.))
                                    .bg(t.bg_card)
                                    .border_1()
                                    .border_color(t.border)
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(format!("{}", count)),
                            ),
                        )
                })),
        )
        // Sidebar footer hint
        .child(
            div()
                .p(px(8.))
                .border_t_1()
                .border_color(t.border)
                .text_xs()
                .text_color(t.text_muted)
                .child("💡 Use {{variable}} in URLs, headers & scripts."),
        )
}
