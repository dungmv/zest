use crate::env::{
    build_tokenized_content, detect_unclosed_env_var, handle_autocomplete_keydown,
    list_candidates_in_state, open_variable_in_modal, render_candidate_detail_card,
    render_candidate_row, resolve_in_state, variable_style, variable_value_tooltip,
    AutocompleteNavState, ResolvedVariable, VariableScope, CANDIDATE_DETAIL_GAP,
    CANDIDATE_LIST_WIDTH,
};
use crate::state::{EnvironmentState, ModalKind, ModalState};
use crate::theme::theme;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    deferred, div, px, App, Div, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement as _, Styled, Window,
};

fn render_var_chip(
    chip_id: SharedString,
    var_name: String,
    env_state: Option<&Entity<EnvironmentState>>,
    cx: &App,
) -> Stateful<Div> {
    let resolved = if let Some(env_ent) = env_state {
        resolve_in_state(env_ent.read(cx), &var_name)
    } else {
        ResolvedVariable {
            name: var_name.clone(),
            value: String::new(),
            scope: VariableScope::Missing,
            is_secret: false,
        }
    };
    let style = variable_style(&resolved.scope);

    div()
        .id(chip_id)
        .tooltip(move |window, cx| variable_value_tooltip(&resolved).build(window, cx))
        .flex()
        .flex_shrink_0()
        .items_center()
        .h(px(20.))
        .px(px(5.))
        .rounded(px(3.))
        .bg(style.bg_tint)
        .text_sm()
        .line_height(px(20.))
        .font_family(".SystemMonoFont")
        .text_color(style.text)
        .cursor_pointer()
        .child(var_name)
}

/// Static, non-editable field visually matching `render_env_input`, including
/// inline `{{var}}` chips with resolved-value tooltips and click-to-edit navigation.
pub fn render_disabled_env_input(
    input_id: impl Into<SharedString>,
    value: &str,
    env_state: Option<&Entity<EnvironmentState>>,
    modal_state: Option<&Entity<ModalState>>,
    cx: &App,
) -> Div {
    let t = theme();
    let base = div()
        .w_full()
        .h(px(28.))
        .px(px(6.))
        .rounded(px(4.))
        .bg(t.bg_app)
        .border_1()
        .border_color(t.border_subtle)
        .flex()
        .items_center()
        .overflow_hidden()
        .text_sm()
        .font_family(".SystemMonoFont")
        .text_color(t.text_secondary);

    let spans = zest_core::env::scan_variable_spans(value);
    if spans.is_empty() {
        return base.truncate().child(value.to_string());
    }

    let input_id: SharedString = input_id.into();
    let mut row = base;
    let mut cursor = 0usize;

    for (idx, span) in spans.into_iter().enumerate() {
        if span.start < cursor || span.end > value.len() || span.start >= span.end {
            continue;
        }
        if span.start > cursor {
            if let Some(prefix) = value.get(cursor..span.start) {
                row = row.child(
                    div()
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .child(prefix.to_string()),
                );
            }
        }
        let var_name = span.name;
        let chip_id = SharedString::from(format!("{}-tok-{}-{}", input_id, idx, var_name));
        let mut chip = render_var_chip(chip_id, var_name.clone(), env_state, cx);
        if modal_state.is_some() {
            let env_c = env_state.cloned();
            let modal_c = modal_state.cloned();
            chip = chip.on_click(move |_, _window, cx| {
                if let (Some(env_ent), Some(modal_ent)) = (&env_c, &modal_c) {
                    open_variable_in_modal(env_ent, modal_ent, &var_name, cx);
                } else if let Some(modal) = &modal_c {
                    modal.update(cx, |st, cx| {
                        st.open(ModalKind::Environment, None, cx);
                    });
                }
            });
        }
        row = row.child(chip);
        cursor = span.end;
    }

    if cursor < value.len() {
        if let Some(suffix) = value.get(cursor..) {
            row = row.child(
                div()
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .child(suffix.to_string()),
            );
        }
    }

    row
}

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
                .map(|(_, _, q)| list_candidates_in_state(env, q))
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

    let env_for_token = env_state.cloned();
    let env_for_click = env_state.cloned();
    let modal_for_click = modal_state.cloned();
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
                                gpui_kit::component::input::InputContent::new(&new_text)
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
                        .cleanable(false)
                        .px(px(0.))
                        .text_sm()
                        .font_family(".SystemMonoFont")
                        .token(move |token_ctx, _window, cx| {
                            let var_name = token_ctx.token().id().to_string();
                            let chip_id = SharedString::from(format!(
                                "{}-tok-{}",
                                inp_id_for_tok, var_name
                            ));
                            render_var_chip(chip_id, var_name, env_for_token.as_ref(), cx)
                        })
                        .on_token_click(move |ev, _window, cx| {
                            let var_name = ev.token().id().to_string();
                            if let (Some(env_ent), Some(modal_ent)) =
                                (&env_for_click, &modal_for_click)
                            {
                                open_variable_in_modal(env_ent, modal_ent, &var_name, cx);
                            } else if let Some(modal) = &modal_for_click {
                                modal.update(cx, |st, cx| {
                                    st.open(ModalKind::Environment, None, cx);
                                });
                            }
                        }),
                ),
        )
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
                        .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, cx| {
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
                                                        gpui_kit::component::input::InputContent::new(
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
