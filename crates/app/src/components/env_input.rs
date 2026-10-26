use crate::components::env_autocomplete::{
    build_tokenized_content, detect_unclosed_env_var, get_env_completions, get_variable_details,
    get_variable_kind_and_color, handle_autocomplete_keydown, render_candidate_detail_card,
    render_candidate_row, variable_value_tooltip, AutocompleteNavState, CANDIDATE_DETAIL_GAP,
    CANDIDATE_LIST_WIDTH,
};
use crate::state::{EnvironmentState, ModalState};
use crate::theme::theme;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    deferred, div, px, App, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement as _, Styled, Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement as _;

pub fn render_env_input<F>(
    input_id: impl Into<SharedString>,
    input_state: &Entity<InputState>,
    env_state: Option<&Entity<EnvironmentState>>,
    modal_state: Option<&Entity<ModalState>>,
    window: &mut Window,
    cx: &mut App,
    on_update: F,
) -> impl IntoElement
where
    F: 'static + Clone + Fn(String, &mut Window, &mut App),
{
    let input_id: SharedString = input_id.into();
    let t = theme();
    let current_val = input_state.read(cx).value().to_string();

    let (completion_candidates, unclosed) = if current_val.contains("{{") {
        if let Some(env_ent) = env_state {
            let env = env_ent.read(cx);
            let unclosed = detect_unclosed_env_var(&current_val);
            let candidates = unclosed
                .as_ref()
                .map(|(_, _, q)| get_env_completions(env, q))
                .unwrap_or_default();
            (candidates, unclosed)
        } else {
            (Vec::new(), None)
        }
    } else {
        (Vec::new(), None)
    };
    let auto_state = window.use_keyed_state(
        SharedString::from(format!("auto-nav-{}", input_id)),
        cx,
        |_window, _cx| AutocompleteNavState::default(),
    );
    let scroll_handle = window.use_keyed_state(
        SharedString::from(format!("auto-scroll-{}", input_id)),
        cx,
        |_window, _cx| gpui::ScrollHandle::new(),
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

    let is_focused = input_state.read(cx).focus_handle(cx).is_focused(window);
    let is_dismissed = auto_state.read(cx).dismissed;
    let show_autocomplete = is_focused && !is_dismissed && !completion_candidates.is_empty();

    let num_candidates = completion_candidates.len();
    if num_candidates > 0 && auto_state.read(cx).selected_index >= num_candidates {
        auto_state.update(cx, |st, _| {
            st.selected_index = num_candidates - 1;
        });
    }
    let selected_index = auto_state.read(cx).selected_index;

    let hover_token_state = window.use_keyed_state(
        SharedString::from(format!("hover-env-{}", input_id)),
        cx,
        |_window, _cx| None::<(String, gpui::Bounds<gpui::Pixels>)>,
    );
    let hovered_token = hover_token_state.read(cx).clone();

    let env_for_token = env_state.cloned();
    let modal_for_click = modal_state.cloned();
    let env_for_hover = env_state.cloned();
    let h_token = hover_token_state.clone();
    let inp_id_for_tok = input_id.clone();

    div()
        .relative()
        .w_full()
        .when(show_autocomplete, |this| {
            let auto_s = auto_state.clone();
            let scr_h = scroll_handle.read(cx).clone();
            let cands = completion_candidates.clone();
            let inp_c = input_state.clone();
            let env_c = env_state.cloned();
            let on_up_c = on_update.clone();
            let (start_idx, end_idx) = unclosed
                .as_ref()
                .map(|(s, e, _)| (*s, *e))
                .unwrap_or((0, 0));
            let cur_text = current_val.clone();

            this.capture_key_down(move |event, window, cx| {
                handle_autocomplete_keydown(
                    event,
                    &cands,
                    &auto_s,
                    &scr_h,
                    window,
                    cx,
                    |cand, window, cx| {
                        let mut new_text = cur_text.clone();
                        if start_idx <= new_text.len()
                            && end_idx <= new_text.len()
                            && start_idx <= end_idx
                        {
                            new_text.replace_range(
                                start_idx..end_idx,
                                &format!("{{{{{}}}}}", cand.name),
                            );
                            let content = if let Some(env_ent) = &env_c {
                                let env = env_ent.read(cx);
                                build_tokenized_content(&new_text, env)
                            } else {
                                gpui_component::input::InputContent::new(&new_text)
                            };
                            inp_c.update(cx, |inp, cx| {
                                inp.set_value(content, window, cx);
                            });
                            on_up_c(new_text, window, cx);
                        }
                    },
                );
            })
        })
        .child(
            div()
                .h(px(28.))
                .px(px(6.))
                .rounded(px(4.))
                .bg(t.bg_input)
                .border_1()
                .border_color(t.border)
                .hover(|s| s.border_color(t.border_focus))
                .flex()
                .items_center()
                .child(
                    Input::new(input_state)
                        .id(input_id.clone())
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .cleanable(true)
                        .px(px(0.))
                        .text_sm()
                        .font_family(".SystemMonoFont")
                        .token(move |token_ctx, _window, cx| {
                            let var_name = token_ctx.token().id().to_string();
                            let (scope, is_res) = if let Some(env_ent) = &env_for_token {
                                let env = env_ent.read(cx);
                                let (_val, sc, res) = get_variable_details(env, &var_name);
                                (sc, res)
                            } else {
                                ("Missing".to_string(), false)
                            };
                            let (_kind, color, bg_tint) =
                                get_variable_kind_and_color(&scope, is_res);

                            div()
                                .id(SharedString::from(format!(
                                    "{}-tok-{}",
                                    inp_id_for_tok, var_name
                                )))
                                .flex()
                                .items_center()
                                .px(px(5.))
                                .py(px(1.))
                                .rounded(px(3.))
                                .bg(bg_tint)
                                .text_sm()
                                .font_family(".SystemMonoFont")
                                .text_color(color)
                                .cursor_pointer()
                                .child(var_name)
                        })
                        .on_token_hover(move |event, _window, cx| {
                            let is_h = event.is_hovered();
                            let name = event.token().id().to_string();
                            let bounds = event.bounds();
                            h_token.update(cx, |h, cx| {
                                if is_h {
                                    *h = Some((name, bounds));
                                } else {
                                    *h = None;
                                }
                                cx.notify();
                            });
                        })
                        .on_token_click(move |_, _window, cx| {
                            if let Some(modal) = &modal_for_click {
                                modal.update(cx, |st, cx| {
                                    st.toggle_env_modal(cx);
                                });
                            }
                        }),
                ),
        )
        .when_some(hovered_token, |this, (var_name, _bounds)| {
            let (val, scope, is_res) = if let Some(env_ent) = &env_for_hover {
                let env = env_ent.read(cx);
                get_variable_details(env, &var_name)
            } else {
                ("".to_string(), "".to_string(), false)
            };

            this.child(
                deferred(
                    div()
                        .absolute()
                        .top(px(34.))
                        .left(px(0.))
                        .child(variable_value_tooltip(&val, &scope, is_res)),
                )
                .with_priority(90),
            )
        })
        .when(show_autocomplete, |this| {
            let (start_idx, end_idx) = unclosed
                .as_ref()
                .map(|(pos, end, _)| (*pos, *end))
                .unwrap_or((0, 0));
            let current_text = current_val.clone();
            let inp_target = input_state.clone();
            let env_target = env_state.cloned();
            let on_up = on_update.clone();
            let auto_s = auto_state.clone();
            let scr_h = scroll_handle.read(cx).clone();
            let selected_cand = completion_candidates.get(selected_index).cloned();

            this.child(
                deferred(
                    div()
                        .absolute()
                        .top(px(32.))
                        .left(px(0.))
                        .w(px(CANDIDATE_LIST_WIDTH))
                        .max_h(px(200.))
                        .rounded(px(6.))
                        .bg(t.bg_card)
                        .border_1()
                        .border_color(t.border)
                        .shadow_2xl()
                        .occlude()
                        .on_mouse_down(gpui::MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .id(SharedString::from(format!("{}-auto-list", input_id)))
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
                                        let cur_text = current_text.clone();
                                        let inp_c = inp_target.clone();
                                        let env_c = env_target.clone();
                                        let on_up_c = on_up.clone();
                                        let is_selected = idx == selected_index;

                                        render_candidate_row(
                                            &input_id,
                                            idx,
                                            &cand,
                                            is_selected,
                                            &auto_s,
                                            move |cand, window, cx| {
                                                let mut new_text = cur_text.clone();
                                                if start_idx <= new_text.len()
                                                    && end_idx <= new_text.len()
                                                    && start_idx <= end_idx
                                                {
                                                    new_text.replace_range(
                                                        start_idx..end_idx,
                                                        &format!("{{{{{}}}}}", cand.name),
                                                    );
                                                    let content = if let Some(env_ent) = &env_c {
                                                        let env = env_ent.read(cx);
                                                        build_tokenized_content(&new_text, env)
                                                    } else {
                                                        gpui_component::input::InputContent::new(
                                                            &new_text,
                                                        )
                                                    };
                                                    inp_c.update(cx, |inp, cx| {
                                                        inp.set_value(content, window, cx);
                                                    });
                                                    on_up_c(new_text, window, cx);
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
                        .top(px(32.))
                        .left(px(CANDIDATE_LIST_WIDTH + CANDIDATE_DETAIL_GAP))
                        .child(render_candidate_detail_card(selected_cand.as_ref())),
                )
                .with_priority(100),
            )
        })
}
