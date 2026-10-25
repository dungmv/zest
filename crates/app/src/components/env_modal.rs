use crate::app::actions::DismissModal;
use crate::state::{EnvironmentState, ModalState, SidebarState};
use crate::theme::theme;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, AnyElement, App, AppContext as _, Entity, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{Icon, Sizable as _};
use gpui_kit::assets::IconName;
use zest_core::models::{Environment, EnvironmentVariable};

#[derive(IntoElement)]
pub struct EnvironmentModal {
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
    sidebar_state: Entity<SidebarState>,
}

impl EnvironmentModal {
    pub fn new(
        env_state: Entity<EnvironmentState>,
        modal_state: Entity<ModalState>,
        sidebar_state: Entity<SidebarState>,
    ) -> Self {
        Self {
            env_state,
            modal_state,
            sidebar_state,
        }
    }
}

impl RenderOnce for EnvironmentModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let ws = self.env_state.read(cx);
        let modal = self.modal_state.read(cx);
        let sidebar = self.sidebar_state.read(cx);

        let envs = ws.environments.clone();
        let global_env = ws.global_environment.clone();
        let active_env_id = ws.active_environment_id.clone();
        let selected_id = modal
            .selected_env_modal_id
            .clone()
            .unwrap_or_else(|| "globals".to_string());
        let ws_clone = self.env_state.clone();
        let modal_clone = self.modal_state.clone();
        let sidebar_clone = self.sidebar_state.clone();
        let renaming_env_id = sidebar.renaming_env_id().map(ToString::to_string);
        let rename_env_input = sidebar.rename_env_input().cloned();

        let is_global_selected = selected_id == "globals";
        let active_custom_env = envs.iter().find(|e| e.id == selected_id).cloned();

        let filtered_envs: Vec<Environment> = envs.clone();

        // Target variables to display in the right pane
        let target_variables: Vec<(usize, EnvironmentVariable)> = if is_global_selected {
            global_env
                .variables
                .iter()
                .enumerate()
                .map(|(i, v)| (i, v.clone()))
                .collect()
        } else if let Some(custom) = &active_custom_env {
            custom
                .variables
                .iter()
                .enumerate()
                .map(|(i, v)| (i, v.clone()))
                .collect()
        } else {
            Vec::new()
        };

        let is_target_active = !is_global_selected
            && active_custom_env.as_ref().map(|e| e.id.as_str()) == active_env_id.as_deref();

        div()
            .id("env-modal-overlay")
            .absolute()
            .inset_0()
            .bg(gpui::Hsla { a: 0.65, ..t.bg_app })
            .flex()
            .items_center()
            .justify_center()
            .on_click({
                let modal_s = modal_clone.clone();
                move |_, _, cx| {
                    modal_s.update(cx, |state, cx| {
                        state.close_env_modal(cx);
                    });
                }
            })
            .on_key_down({
                let modal_s = modal_clone.clone();
                move |event, _, cx| {
                    if event.keystroke.key.as_str() == "escape" {
                        modal_s.update(cx, |state, cx| {
                            state.close_env_modal(cx);
                        });
                    }
                }
            })
            .child(
                div()
                    .id("env-modal-dialog")
                    .occlude()
                    .w(px(980.))
                    .h(px(580.))
                    .rounded(px(10.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_key_down({
                        let modal_s = modal_clone.clone();
                        move |event, _, cx| {
                            if event.keystroke.key.as_str() == "escape" {
                                modal_s.update(cx, |state, cx| {
                                    state.close_env_modal(cx);
                                });
                            }
                        }
                    })
                    // Modal Header
                    .child(
                        div()
                            .h(px(46.))
                            .px(px(16.))
                            .border_b_1()
                            .border_color(t.border)
                            .bg(t.bg_titlebar)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .child("Environments & Variables"),
                                    ),
                            )
                            .child({
                                let modal_s = modal_clone.clone();
                                div()
                                    .id("close-env-modal")
                                    .cursor_pointer()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .text_color(t.text_muted)
                                    .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                                    .on_click(move |_, _, cx| {
                                        modal_s.update(cx, |state, cx| {
                                            state.toggle_env_modal(cx);
                                        });
                                    })
                                    .child("✕")
                            }),
                    )
                    // Modal Body: Sidebar (Left) + Detail Editor (Right)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .overflow_hidden()
                            // LEFT SIDEBAR
                            .child(
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
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(t.text_muted)
                                                    .child("GLOBAL"),
                                            )
                                            .child({
                                                let modal_s = modal_clone.clone();
                                                let is_sel = is_global_selected;
                                                let count = global_env.variables.len();
                                                div()
                                                    .id("env-item-globals")
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .rounded(px(5.))
                                                    .cursor_pointer()
                                                    .bg(if is_sel { t.bg_selected } else { gpui::transparent_black() })
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
                                                                    .font_weight(if is_sel { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL })
                                                                    .text_color(if is_sel { t.text_primary } else { t.text_secondary })
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
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
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
                                            .when(filtered_envs.is_empty(), |this| {
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
                                            .children(filtered_envs.into_iter().map(|env| {
                                                let is_sel = selected_id == env.id;
                                                let is_act = active_env_id.as_deref() == Some(&env.id);
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
                                                    .id(gpui::SharedString::from(format!("env-sidebar-item-{}", env.id)))
                                                    .px(px(8.))
                                                    .py(px(6.))
                                                    .rounded(px(5.))
                                                    .cursor_pointer()
                                                    .bg(if is_sel { t.bg_selected } else { gpui::transparent_black() })
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

                                                            menu
                                                                .item(
                                                                    PopupMenuItem::new("Rename")
                                                                        .on_click(move |_, window, cx| {
                                                                            let ws_c = ws_r.clone();
                                                                            sb_r.update(cx, |state, cx| {
                                                                                state.start_renaming_environment(r_id.clone(), r_name.clone(), ws_c, window, cx);
                                                                            });
                                                                        }),
                                                                )
                                                                .item(
                                                                    PopupMenuItem::new("Duplicate")
                                                                        .on_click(move |_, _, cx| {
                                                                            ws_d.update(cx, |state, cx| {
                                                                                state.duplicate_environment(&d_id, cx);
                                                                            });
                                                                        }),
                                                                )
                                                                .separator()
                                                                .item(
                                                                    PopupMenuItem::new("Delete")
                                                                        .on_click(move |_, _, cx| {
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
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(if is_act { t.status_2xx } else { t.text_muted })
                                                                    .child(if is_act { "●" } else { "○" }),
                                                            )
                                                            .child(if let (true, Some(inp)) = (is_renaming, rename_env_input.as_ref()) {
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
                                                                    // Escape arrives as the globally-bound
                                                                    // DismissModal action; the deepest handler
                                                                    // cancels the rename instead of the modal.
                                                                    .on_action(move |_: &DismissModal, _, cx| {
                                                                        sb_esc.update(cx, |state, cx| {
                                                                            state.cancel_renaming_environment(cx);
                                                                        });
                                                                    })
                                                                    .child(
                                                                        Input::new(inp)
                                                                            .id(format!("rename-env-input-field-{}", env_id))
                                                                            .appearance(false)
                                                                            .bordered(false)
                                                                            .focus_bordered(false)
                                                                            .cleanable(false)
                                                                            .xsmall()
                                                                            .font_weight(if is_sel { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL }),
                                                                    )
                                                                    .into_any_element()
                                                            } else {
                                                                div()
                                                                    .truncate()
                                                                    .text_xs()
                                                                    .font_weight(if is_sel { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL })
                                                                    .text_color(if is_sel { t.text_primary } else { t.text_secondary })
                                                                    .child(env_name)
                                                                    .into_any_element()
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(4.))
                                                            // Count badge
                                                            .child(
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
                                    ),
                            )
                            // RIGHT DETAIL PANE
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .bg(t.bg_panel)
                                    .flex()
                                    .flex_col()
                                    // Detail Header
                                    .child(
                                        div()
                                            .h(px(54.))
                                            .px(px(16.))
                                            .border_b_1()
                                            .border_color(t.border)
                                            .bg(t.bg_card)
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                // Left of header: Name + Active toggle
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(10.))
                                                    .when(is_global_selected, |this| {
                                                        this.child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .gap(px(1.))
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(gpui::FontWeight::BOLD)
                                                                        .text_color(t.text_primary)
                                                                        .child(div().flex().items_center().gap(px(4.)).child(Icon::new(IconName::Globe).size(px(14.))).child("Global Variables")),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(t.text_muted)
                                                                        .child("Shared across all requests. Active environment overrides matching keys."),
                                                                ),
                                                        )
                                                    })
                                                    .when(!is_global_selected, |this| {
                                                        if let Some(custom) = &active_custom_env {
                                                            this.child(
                                                                div()
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap(px(8.))
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(gpui::FontWeight::BOLD)
                                                                            .text_color(t.text_primary)
                                                                            .child(custom.name.clone()),
                                                                    )
                                                                    .when(is_target_active, |this| {
                                                                        this.child(
                                                                            div()
                                                                                .px(px(6.))
                                                                                .py(px(2.))
                                                                                .rounded(px(4.))
                                                                                .bg(gpui::Hsla { a: 0.15, ..t.status_2xx })
                                                                                .text_color(t.status_2xx)
                                                                                .text_xs()
                                                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                                                .child("● Active"),
                                                                        )
                                                                    }),
                                                            )
                                                        } else {
                                                            this.child(div().text_sm().child("Select an environment"))
                                                        }
                                                    }),
                                            )
                                            .child(
                                                // Right of header: Add variable
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child({
                                                        let ws_add = ws_clone.clone();
                                                        let is_glob = is_global_selected;
                                                        let cur_id = selected_id.clone();

                                                        div()
                                                            .id("btn-detail-add-variable")
                                                            .cursor_pointer()
                                                            .px(px(10.))
                                                            .py(px(4.))
                                                            .rounded(px(4.))
                                                            .bg(t.primary)
                                                            .text_color(t.primary_foreground)
                                                            .hover(|s| s.bg(t.primary_hover))
                                                            .text_xs()
                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                            .on_click(move |_, _, cx| {
                                                                ws_add.update(cx, |state, cx| {
                                                                    let new_var = EnvironmentVariable {
                                                                        key: "".to_string(),
                                                                        initial_value: "".to_string(),
                                                                        current_value: "".to_string(),
                                                                        secret: false,
                                                                    };
                                                                    if is_glob {
                                                                        let mut g = state.global_environment.clone();
                                                                        g.variables.push(new_var);
                                                                        state.save_global_environment(g, cx);
                                                                    } else {
                                                                        let mut list = state.environments.clone();
                                                                        if let Some(e) = list.iter_mut().find(|x| x.id == cur_id) {
                                                                            e.variables.push(new_var);
                                                                        }
                                                                        state.save_environments(list, cx);
                                                                    }
                                                                });
                                                            })
                                                            .child("+ Add Variable")
                                                    }),
                                            ),
                                    )
                                    // Variables Table Container
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .overflow_hidden()
                                            // Table Header
                                            .child(
                                                div()
                                                    .h(px(30.))
                                                    .px(px(16.))
                                                    .bg(t.bg_card)
                                                    .border_b_1()
                                                    .border_color(t.border)
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(t.text_muted)
                                                    .child(div().w(px(180.)).child("VARIABLE NAME"))
                                                    .child(div().flex_1().child("INITIAL VALUE (GIT)"))
                                                    .child(div().flex_1().child("CURRENT VALUE (LOCAL)"))
                                                    .child(div().w(px(64.)).text_center().child("SECRET"))
                                                    .child(div().w(px(24.)).child("")),
                                            )
                                            // Table Rows
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .px(px(12.))
                                                    .py(px(6.))
                                                    .overflow_y_scrollbar()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(4.))
                                                    .when(target_variables.is_empty(), |this| {
                                                        this.child(
                                                            div()
                                                                .py(px(60.))
                                                                .flex()
                                                                .flex_col()
                                                                .items_center()
                                                                .justify_center()
                                                                .gap(px(8.))
                                                                .text_xs()
                                                                .text_color(t.text_muted)
                                                                .child("No variables yet.")
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(t.primary)
                                                                        .child("Click '+ Add Variable' above to add a new variable."),
                                                                ),
                                                        )
                                                    })
                                                    .children(target_variables.into_iter().map(|(orig_idx, var)| {
                                                        let is_glob = is_global_selected;
                                                        let cur_id = selected_id.clone();
                                                        let row_id = format!("{}-row-{}", selected_id, orig_idx);
                                                        let v_key = var.key.clone();
                                                        let v_init = var.initial_value.clone();
                                                        let v_curr = var.current_value.clone();
                                                        let secret = var.secret;

                                                        let ws_up_k = ws_clone.clone();
                                                        let ws_up_init = ws_clone.clone();
                                                        let ws_up_curr = ws_clone.clone();
                                                        let ws_secret = ws_clone.clone();
                                                        let ws_del = ws_clone.clone();

                                                        let init_k = v_key.clone();
                                                        let init_val_init = v_init.clone();
                                                        let init_val_curr = v_curr.clone();
                                                        let holder = window.use_keyed_state(
                                                            gpui::SharedString::from(format!("{}-holder", row_id)),
                                                            cx,
                                                            |window, cx| {
                                                                let k_in = cx.new(|cx| {
                                                                    let mut s = InputState::new(window, cx).placeholder("Variable key...");
                                                                    s.set_value(init_k, window, cx);
                                                                    s
                                                                });
                                                                let init_in = cx.new(|cx| {
                                                                    let mut s = InputState::new(window, cx).placeholder("Initial value (git)...");
                                                                    s.set_value(init_val_init, window, cx);
                                                                    s
                                                                });
                                                                let curr_in = cx.new(|cx| {
                                                                    let mut s = InputState::new(window, cx).placeholder("Current value (local)...");
                                                                    s.set_value(init_val_curr, window, cx);
                                                                    s
                                                                });

                                                                let k1 = k_in.clone();
                                                                let cur_id1 = cur_id.clone();
                                                                let sub1 = cx.subscribe_in(&k_in, window, move |_, _, ev, _, cx| {
                                                                    if let InputEvent::Change = ev {
                                                                        let nk = k1.read(cx).value().to_string();
                                                                        let cid = cur_id1.clone();
                                                                        ws_up_k.update(cx, |state, _cx| {
                                                                            if is_glob {
                                                                                let mut g = state.global_environment.clone();
                                                                                if let Some(vr) = g.variables.get_mut(orig_idx) {
                                                                                    vr.key = nk;
                                                                                }
                                                                                state.save_global_environment_silently(g);
                                                                            } else {
                                                                                let mut list = state.environments.clone();
                                                                                if let Some(e) = list.iter_mut().find(|x| x.id == cid) {
                                                                                    if let Some(vr) = e.variables.get_mut(orig_idx) {
                                                                                        vr.key = nk;
                                                                                    }
                                                                                }
                                                                                state.save_environments_debounced(list, _cx);
                                                                            }
                                                                        });
                                                                    }
                                                                });

                                                                let init2 = init_in.clone();
                                                                let cur_id2 = cur_id.clone();
                                                                let sub2 = cx.subscribe_in(&init_in, window, move |_, _, ev, _, cx| {
                                                                    if let InputEvent::Change = ev {
                                                                        let n_init = init2.read(cx).value().to_string();
                                                                        let cid = cur_id2.clone();
                                                                        ws_up_init.update(cx, |state, _cx| {
                                                                            if is_glob {
                                                                                let mut g = state.global_environment.clone();
                                                                                if let Some(vr) = g.variables.get_mut(orig_idx) {
                                                                                    vr.initial_value = n_init;
                                                                                }
                                                                                state.save_global_environment_silently(g);
                                                                            } else {
                                                                                let mut list = state.environments.clone();
                                                                                if let Some(e) = list.iter_mut().find(|x| x.id == cid) {
                                                                                    if let Some(vr) = e.variables.get_mut(orig_idx) {
                                                                                        vr.initial_value = n_init;
                                                                                    }
                                                                                }
                                                                                state.save_environments_debounced(list, _cx);
                                                                            }
                                                                        });
                                                                    }
                                                                });

                                                                let curr3 = curr_in.clone();
                                                                let cur_id3 = cur_id.clone();
                                                                let sub3 = cx.subscribe_in(&curr_in, window, move |_, _, ev, _, cx| {
                                                                    if let InputEvent::Change = ev {
                                                                        let n_curr = curr3.read(cx).value().to_string();
                                                                        let cid = cur_id3.clone();
                                                                        ws_up_curr.update(cx, |state, _cx| {
                                                                            if is_glob {
                                                                                let mut g = state.global_environment.clone();
                                                                                if let Some(vr) = g.variables.get_mut(orig_idx) {
                                                                                    vr.current_value = n_curr;
                                                                                }
                                                                                state.save_global_environment_silently(g);
                                                                            } else {
                                                                                let mut list = state.environments.clone();
                                                                                if let Some(e) = list.iter_mut().find(|x| x.id == cid) {
                                                                                    if let Some(vr) = e.variables.get_mut(orig_idx) {
                                                                                        vr.current_value = n_curr;
                                                                                    }
                                                                                }
                                                                                state.save_environments_debounced(list, _cx);
                                                                            }
                                                                        });
                                                                    }
                                                                });

                                                                (k_in, init_in, curr_in, vec![sub1, sub2, sub3])
                                                            },
                                                        );

                                                        let (k_input, init_input, curr_input) = {
                                                            let h = holder.read(cx);
                                                            (h.0.clone(), h.1.clone(), h.2.clone())
                                                        };

                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .px(px(4.))
                                                            .py(px(2.))
                                                            .gap(px(8.))
                                                            .rounded(px(4.))
                                                            .hover(|s| s.bg(t.bg_hover))
                                                            // Key Input
                                                            .child(
                                                                div()
                                                                    .w(px(180.))
                                                                    .h(px(28.))
                                                                    .px(px(6.))
                                                                    .rounded(px(4.))
                                                                    .bg(t.bg_input)
                                                                    .border_1()
                                                                    .border_color(t.border)
                                                                    .flex()
                                                                    .items_center()
                                                                    .child(
                                                                        Input::new(&k_input)
                                                                            .id(gpui::SharedString::from(format!("{}-key", row_id)))
                                                                            .appearance(false)
                                                                            .bordered(false)
                                                                            .focus_bordered(false),
                                                                    ),
                                                            )
                                                            // Initial Value Input
                                                            .child(
                                                                div()
                                                                    .flex_1()
                                                                    .h(px(28.))
                                                                    .px(px(6.))
                                                                    .rounded(px(4.))
                                                                    .bg(t.bg_input)
                                                                    .border_1()
                                                                    .border_color(t.border)
                                                                    .flex()
                                                                    .items_center()
                                                                    .child(
                                                                        Input::new(&init_input)
                                                                            .id(gpui::SharedString::from(format!("{}-init-val", row_id)))
                                                                            .appearance(false)
                                                                            .bordered(false)
                                                                            .focus_bordered(false),
                                                                    ),
                                                            )
                                                            // Current Value Input
                                                            .child(
                                                                div()
                                                                    .flex_1()
                                                                    .h(px(28.))
                                                                    .px(px(6.))
                                                                    .rounded(px(4.))
                                                                    .bg(t.bg_input)
                                                                    .border_1()
                                                                    .border_color(t.border)
                                                                    .flex()
                                                                    .items_center()
                                                                    .child(
                                                                        Input::new(&curr_input)
                                                                            .id(gpui::SharedString::from(format!("{}-curr-val", row_id)))
                                                                            .appearance(false)
                                                                            .bordered(false)
                                                                            .focus_bordered(false),
                                                                    ),
                                                            )
                                                            // Secret toggle button
                                                            .child({
                                                                let cur_id_s = cur_id.clone();
                                                                div()
                                                                    .id(gpui::SharedString::from(format!("{}-secret", row_id)))
                                                                    .w(px(64.))
                                                                    .h(px(24.))
                                                                    .rounded(px(4.))
                                                                    .bg(if secret { gpui::Hsla { a: 0.15, ..t.status_4xx } } else { gpui::transparent_black() })
                                                                    .hover(|s| s.bg(t.bg_card))
                                                                    .cursor_pointer()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .text_xs()
                                                                    .text_color(if secret { t.status_4xx } else { t.text_muted })
                                                                    .on_click(move |_, _, cx| {
                                                                        let cid = cur_id_s.clone();
                                                                        ws_secret.update(cx, |state, cx| {
                                                                            if is_glob {
                                                                                let mut g = state.global_environment.clone();
                                                                                if let Some(vr) = g.variables.get_mut(orig_idx) {
                                                                                    vr.secret = !secret;
                                                                                }
                                                                                state.save_global_environment(g, cx);
                                                                            } else {
                                                                                let mut list = state.environments.clone();
                                                                                if let Some(e) = list.iter_mut().find(|x| x.id == cid) {
                                                                                    if let Some(vr) = e.variables.get_mut(orig_idx) {
                                                                                        vr.secret = !secret;
                                                                                    }
                                                                                }
                                                                                state.save_environments(list, cx);
                                                                            }
                                                                        });
                                                                    })
                                                                    .child(if secret { div().flex().items_center().gap(px(4.)).child(Icon::new(IconName::Lock).size(px(14.))).child(" Secret").into_any_element() } else { div().flex().items_center().gap(px(4.)).child(Icon::new(IconName::LockOpen).size(px(14.))).child(" Visible").into_any_element() })
                                                            })
                                                            // Delete row button
                                                            .child({
                                                                let cur_id_d = cur_id.clone();
                                                                div()
                                                                    .id(gpui::SharedString::from(format!("{}-del", row_id)))
                                                                    .w(px(24.))
                                                                    .h(px(24.))
                                                                    .rounded(px(4.))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .cursor_pointer()
                                                                    .hover(|s| s.bg(t.bg_card).text_color(t.status_5xx))
                                                                    .text_color(t.text_muted)
                                                                    .text_xs()
                                                                    .on_click(move |_, _, cx| {
                                                                        let cid = cur_id_d.clone();
                                                                        ws_del.update(cx, |state, cx| {
                                                                            if is_glob {
                                                                                let mut g = state.global_environment.clone();
                                                                                if orig_idx < g.variables.len() {
                                                                                    g.variables.remove(orig_idx);
                                                                                }
                                                                                state.save_global_environment(g, cx);
                                                                            } else {
                                                                                let mut list = state.environments.clone();
                                                                                if let Some(e) = list.iter_mut().find(|x| x.id == cid) {
                                                                                    if orig_idx < e.variables.len() {
                                                                                        e.variables.remove(orig_idx);
                                                                                    }
                                                                                }
                                                                                state.save_environments(list, cx);
                                                                            }
                                                                        });
                                                                    })
                                                                    .child("×")
                                                            })
                                                    }))
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
