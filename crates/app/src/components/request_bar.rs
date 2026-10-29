use crate::components::env_autocomplete::{
    build_tokenized_content, detect_unclosed_env_var, get_env_completions, get_variable_details,
    get_variable_kind_and_color, handle_autocomplete_keydown, render_candidate_detail_card,
    render_candidate_row, variable_value_tooltip, AutocompleteNavState, CANDIDATE_DETAIL_GAP,
    CANDIDATE_LIST_WIDTH,
};
use crate::components::status_badge::MethodBadge;
use crate::state::{EnvironmentState, ModalState, RequestSessionState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    deferred, div, px, App, AppContext as _, Entity, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window,
};
use zest_core::models::HttpMethod;

#[allow(clippy::type_complexity)]
#[derive(IntoElement)]
pub struct RequestBar {
    workspace_state: Entity<crate::state::WorkspaceDataState>,
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
    editor_state: Entity<RequestSessionState>,
    url_input: Entity<InputState>,
    on_send: std::rc::Rc<dyn Fn(&mut Window, &mut App)>,
}

impl RequestBar {
    pub fn new(
        workspace_state: Entity<crate::state::WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        modal_state: Entity<ModalState>,
        editor_state: Entity<RequestSessionState>,
        url_input: Entity<InputState>,
        on_send: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            workspace_state,
            env_state,
            modal_state,
            editor_state,
            url_input,
            on_send: std::rc::Rc::new(on_send),
        }
    }
}

impl RenderOnce for RequestBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let (method, is_sending) = {
            let ed = self.editor_state.read(cx);
            let active_id = ed.active_request_id();
            let method = self
                .workspace_state
                .read(cx)
                .find_request(active_id)
                .map(|r| r.method)
                .unwrap_or(HttpMethod::Get);
            (method, ed.is_sending)
        };
        let url = self.url_input.read(cx).value().to_string();

        let ws_clone = self.env_state.clone();
        let modal_clone = self.modal_state.clone();
        let ed_clone = self.editor_state.clone();
        let url_input_clone = self.url_input.clone();
        let on_send = self.on_send.clone();

        // Build completions only if URL contains {{
        let (completion_candidates, unclosed) = if url.contains("{{") {
            let ws = self.env_state.read(cx);
            let unclosed = detect_unclosed_env_var(&url);
            let candidates = unclosed
                .as_ref()
                .map(|(_, _, q)| get_env_completions(ws, q))
                .unwrap_or_default();

            (candidates, unclosed)
        } else {
            (Vec::new(), None)
        };

        let auto_state = window.use_keyed_state(
            gpui_kit::SharedString::from("auto-nav-url-bar"),
            cx,
            |_window, _cx| AutocompleteNavState::default(),
        );
        let scroll_handle = window.use_keyed_state(
            gpui_kit::SharedString::from("auto-scroll-url-bar"),
            cx,
            |_window, _cx| gpui_kit::ScrollHandle::new(),
        );

        let current_q = unclosed.as_ref().map(|(_, _, q)| q.clone());
        let last_q = auto_state.read(cx).last_query.clone();
        if last_q != current_q {
            auto_state.update(cx, |st, _| {
                st.last_query = current_q;
                st.dismissed = false;
                st.selected_index = 0;
            });
        }

        let is_focused = self.url_input.read(cx).focus_handle(cx).is_focused(window);
        let is_dismissed = auto_state.read(cx).dismissed;
        let show_autocomplete = is_focused && !is_dismissed && !completion_candidates.is_empty();

        let num_candidates = completion_candidates.len();
        if num_candidates > 0 && auto_state.read(cx).selected_index >= num_candidates {
            auto_state.update(cx, |st, _| {
                st.selected_index = num_candidates - 1;
            });
        }
        let selected_index = auto_state.read(cx).selected_index;

        div()
            .relative()
            .h(px(44.))
            .w_full()
            .px(px(8.))
            .py(px(4.))
            .flex()
            .items_center()
            .bg(t.bg_editor)
            .border_b_1()
            .border_color(t.border)
            // Unified Request Input Bar
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h(px(34.))
                    .px(px(4.))
                    .rounded(px(6.))
                    .bg(t.bg_input)
                    .border_1()
                    .border_color(t.border)
                    .hover(|s| s.border_color(t.border_focus))
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .when(show_autocomplete, |this| {
                        let auto_s = auto_state.clone();
                        let scr_h = scroll_handle.read(cx).clone();
                        let cands = completion_candidates.clone();
                        let (start_idx, end_idx) = unclosed
                            .as_ref()
                            .map(|(pos, end, _)| (*pos, *end))
                            .unwrap_or((0, 0));
                        let current_url = url.clone();
                        let inp_target = url_input_clone.clone();
                        let ed_target = ed_clone.clone();
                        let ws_c = ws_clone.clone();
                        let ws_data_target = self.workspace_state.clone();
                        let req_id_str = ed_clone.read(cx).active_request_id().to_string();

                        this.capture_key_down(move |event, window, cx| {
                            handle_autocomplete_keydown(
                                event,
                                &cands,
                                &auto_s,
                                &scr_h,
                                window,
                                cx,
                                |cand, window, cx| {
                                    let mut new_u = current_url.clone();
                                    if start_idx <= new_u.len()
                                        && end_idx <= new_u.len()
                                        && start_idx <= end_idx
                                    {
                                        new_u.replace_range(
                                            start_idx..end_idx,
                                            &format!("{{{{{}}}}}", cand.name),
                                        );
                                        let content = {
                                            let ws = ws_c.read(cx);
                                            build_tokenized_content(&new_u, ws)
                                        };
                                        inp_target.update(cx, |inp, cx| {
                                            inp.set_value(content, window, cx);
                                        });
                                        ws_data_target.update(cx, |ws, _| {
                                            ws.update_active_request(&req_id_str, |r| {
                                                r.apply_url(&new_u);
                                            });
                                        });
                                        ed_target.update(cx, |ed, cx| {
                                            ed.mark_dirty(cx);
                                        });
                                    }
                                },
                            );
                        })
                    })
                    // Method dropdown trigger
                    .child({
                        let ws_s = self.workspace_state.clone();
                        let ed_s = ed_clone.clone();
                        let req_id_str = ed_clone.read(cx).active_request_id().to_string();

                        Button::new("method-dropdown-btn")
                            .ghost()
                            .compact()
                            .pl(px(4.))
                            .pr(px(1.))
                            .py(px(3.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .child(
                                MethodBadge::new(method)
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD),
                            )
                            .dropdown_menu(move |menu, _window, _cx| {
                                [
                                    HttpMethod::Get,
                                    HttpMethod::Post,
                                    HttpMethod::Put,
                                    HttpMethod::Delete,
                                    HttpMethod::Patch,
                                    HttpMethod::Head,
                                    HttpMethod::Options,
                                ]
                                .into_iter()
                                .fold(menu, |m, mth| {
                                    let ws_set = ws_s.clone();
                                    let ed_set = ed_s.clone();
                                    let req_id = req_id_str.clone();
                                    m.item(
                                        PopupMenuItem::element(move |_window, _cx| {
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(theme().method_color(mth))
                                                .child(mth.as_str().to_string())
                                        })
                                        .on_click(
                                            move |_, _, cx| {
                                                ws_set.update(cx, |ws, _| {
                                                    ws.update_active_request(&req_id, |r| {
                                                        r.set_method(mth);
                                                    });
                                                });
                                                ed_set.update(cx, |ed, cx| {
                                                    ed.mark_dirty(cx);
                                                });
                                            },
                                        ),
                                    )
                                })
                            })
                    })
                    // Subtle vertical separator
                    .child(div().w(px(1.)).h(px(16.)).bg(t.border))
                    // URL Input Box - with inline tokens for environment variables
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .relative()
                            .flex()
                            .items_center()
                            .child({
                                let ws_env = ws_clone.clone();
                                let modal_click = modal_clone.clone();

                                Input::new(&self.url_input)
                                    .id("url-input-field")
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .cleanable(false)
                                    .px(px(2.))
                                    .font_family(".SystemMonoFont")
                                    .text_sm()
                                    .token(move |token_ctx, _window, cx| {
                                        let var_name = token_ctx.token().id().to_string();
                                        let ws = ws_env.read(cx);
                                        let (val, scope, is_res) =
                                            get_variable_details(ws, &var_name);
                                        let (_kind, color, bg_tint) =
                                            get_variable_kind_and_color(&scope, is_res);

                                        div()
                                            .id(gpui_kit::SharedString::from(format!(
                                                "inline-token-{}",
                                                var_name
                                            )))
                                            .tooltip(move |window, cx| {
                                                variable_value_tooltip(&val, &scope, is_res)
                                                    .build(window, cx)
                                            })
                                            .flex()
                                            .items_center()
                                            .px(px(6.))
                                            .py(px(1.))
                                            .rounded(px(4.))
                                            .bg(bg_tint)
                                            .text_sm()
                                            .font_family(".SystemMonoFont")
                                            .text_color(color)
                                            .cursor_pointer()
                                            .child(var_name)
                                    })
                                    .on_token_click(move |_, _window, cx| {
                                        modal_click.update(cx, |state, cx| {
                                            state.toggle_env_modal(cx);
                                        });
                                    })
                            }),
                    )
                    // Subtle vertical separator
                    .child(div().w(px(1.)).h(px(16.)).bg(t.border))
                    // Send / Cancel Action Button
                    .child({
                        let ed_s = ed_clone.clone();
                        let on_send_cb = on_send.clone();

                        Button::new("btn-send-or-cancel")
                            .ghost()
                            .compact()
                            .px(px(6.))
                            .py(px(3.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .tooltip(if is_sending {
                                "Cancel request".to_string()
                            } else if cfg!(target_os = "macos") {
                                "Send (⌘Enter)".to_string()
                            } else {
                                "Send (Ctrl+Enter)".to_string()
                            })
                            .on_click(move |_, window, cx| {
                                if is_sending {
                                    ed_s.update(cx, |state, cx| {
                                        if let Some(tx) = state.cancel_tx.take() {
                                            let _ = tx.send(());
                                        }
                                        state.stop_sending(cx);
                                    });
                                } else {
                                    on_send_cb(window, cx);
                                }
                            })
                            .child(
                                Icon::new(if is_sending {
                                    IconName::Close
                                } else {
                                    IconName::SendHorizontal
                                })
                                .size(px(14.))
                                .text_color(if is_sending {
                                    t.status_5xx
                                } else {
                                    t.text_secondary
                                }),
                            )
                    }),
            )
            // Autocomplete Dropdown popup when user types {{
            .when(show_autocomplete, |this| {
                let (start_idx, end_idx) = unclosed
                    .as_ref()
                    .map(|(pos, end, _)| (*pos, *end))
                    .unwrap_or((0, 0));
                let current_url = url.clone();
                let inp_target = url_input_clone.clone();
                let ed_target = ed_clone.clone();
                let ws_data_target = self.workspace_state.clone();
                let req_id_str = ed_clone.read(cx).active_request_id().to_string();
                let auto_s = auto_state.clone();
                let scr_h = scroll_handle.read(cx).clone();
                let selected_cand = completion_candidates.get(selected_index).cloned();

                this.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(42.))
                            .left(px(80.))
                            .w(px(CANDIDATE_LIST_WIDTH))
                            .max_h(px(240.))
                            .rounded(px(6.))
                            .bg(t.bg_card)
                            .border_1()
                            .border_color(t.border)
                            .shadow_2xl()
                            .occlude()
                            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, cx| {
                                window.prevent_default();
                                cx.stop_propagation();
                            })
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(
                                div()
                                    .id("url-auto-list")
                                    .flex_1()
                                    .p(px(4.))
                                    .track_scroll(&scr_h)
                                    .overflow_y_scroll()
                                    .bg(t.bg_card)
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .children(completion_candidates.into_iter().enumerate().map(
                                        |(idx, cand)| {
                                            let url_c = current_url.clone();
                                            let inp_c = inp_target.clone();
                                            let ed_c = ed_target.clone();
                                            let ws_c = ws_clone.clone();
                                            let ws_data_c = ws_data_target.clone();
                                            let req_id_c = req_id_str.clone();
                                            let is_selected = idx == selected_index;

                                            render_candidate_row(
                                                "url",
                                                idx,
                                                &cand,
                                                is_selected,
                                                &auto_s,
                                                move |cand, window, cx| {
                                                    let mut new_u = url_c.clone();
                                                    if start_idx <= new_u.len()
                                                        && end_idx <= new_u.len()
                                                        && start_idx <= end_idx
                                                    {
                                                        new_u.replace_range(
                                                            start_idx..end_idx,
                                                            &format!("{{{{{}}}}}", cand.name),
                                                        );
                                                        let content = {
                                                            let ws = ws_c.read(cx);
                                                            build_tokenized_content(&new_u, ws)
                                                        };
                                                        inp_c.update(cx, |inp, cx| {
                                                            inp.set_value(content, window, cx);
                                                        });
                                                        ws_data_c.update(cx, |ws, _| {
                                                            ws.update_active_request(
                                                                &req_id_c,
                                                                |r| {
                                                                    r.apply_url(&new_u);
                                                                },
                                                            );
                                                        });
                                                        ed_c.update(cx, |ed, cx| {
                                                            ed.mark_dirty(cx);
                                                        });
                                                    }
                                                },
                                            )
                                        },
                                    )),
                            )
                            // The scrollbar layer must stay outside the tracked list div,
                            // otherwise it shifts `scroll_to_item` child indices.
                            .vertical_scrollbar(&scr_h),
                    )
                    .with_priority(100),
                )
                // Compact detail card beside the popup, following the selection.
                .child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(42.))
                            .left(px(80. + CANDIDATE_LIST_WIDTH + CANDIDATE_DETAIL_GAP))
                            .child(render_candidate_detail_card(selected_cand.as_ref())),
                    )
                    .with_priority(100),
                )
            })
    }
}
