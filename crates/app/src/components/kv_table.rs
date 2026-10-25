use crate::components::env_autocomplete::{build_tokenized_content, detect_unclosed_env_var};
use crate::components::env_input::render_env_input;
use crate::state::{EnvironmentState, ModalState};
use crate::theme::{theme, AppTheme};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, App, AppContext as _, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled, Subscription,
    Window,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::popover::Popover;
use gpui_component::Icon;
use gpui_kit::assets::IconName;
use std::rc::Rc;
use zest_core::models::KeyValuePair;

/// A non-editable row appended below the editable pairs, used to surface inherited
/// headers inside the same table. `source_detail` is the full line shown in the
/// info popover (e.g. `Inherited from folder "API"`).
pub struct KvReadonlyRow {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub source_detail: String,
}

/// Static, non-editable field visually matching the env inputs rendered beside it.
pub fn render_disabled_input(value: &str, t: &AppTheme) -> Div {
    div()
        .w_full()
        .h(px(28.))
        .px(px(6.))
        .rounded(px(4.))
        .bg(t.bg_app)
        .border_1()
        .border_color(t.border_subtle)
        .flex()
        .items_center()
        .text_xs()
        .font_family(".SystemMonoFont")
        .text_color(t.text_secondary)
        .truncate()
        .child(value.to_string())
}

struct KvRowInputHolder {
    key_input: Entity<InputState>,
    val_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

#[allow(clippy::type_complexity)]
#[derive(IntoElement)]
pub struct KvTable {
    id_prefix: String,
    pairs: Vec<KeyValuePair>,
    on_update: Rc<dyn Fn(usize, String, String, bool, &mut Window, &mut App)>,
    on_add: Rc<dyn Fn(&mut Window, &mut App)>,
    on_remove: Rc<dyn Fn(usize, &mut Window, &mut App)>,
    env_state: Option<Entity<EnvironmentState>>,
    modal_state: Option<Entity<ModalState>>,
    readonly_rows: Vec<KvReadonlyRow>,
}

impl KvTable {
    pub fn new(
        id_prefix: impl Into<String>,
        pairs: Vec<KeyValuePair>,
        on_update: impl Fn(usize, String, String, bool, &mut Window, &mut App) + 'static,
        on_add: impl Fn(&mut Window, &mut App) + 'static,
        on_remove: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id_prefix: id_prefix.into(),
            pairs,
            on_update: Rc::new(on_update),
            on_add: Rc::new(on_add),
            on_remove: Rc::new(on_remove),
            env_state: None,
            modal_state: None,
            readonly_rows: Vec::new(),
        }
    }

    /// Appends non-editable rows (inherited headers) below the editable pairs.
    pub fn readonly_rows(mut self, rows: Vec<KvReadonlyRow>) -> Self {
        self.readonly_rows = rows;
        self
    }

    pub fn env_state(mut self, env: Entity<EnvironmentState>) -> Self {
        self.env_state = Some(env);
        self
    }

    pub fn optional_env_state(mut self, env: Option<Entity<EnvironmentState>>) -> Self {
        self.env_state = env;
        self
    }

    pub fn modal_state(mut self, modal: Entity<ModalState>) -> Self {
        self.modal_state = Some(modal);
        self
    }

    pub fn optional_modal_state(mut self, modal: Option<Entity<ModalState>>) -> Self {
        self.modal_state = modal;
        self
    }
}

impl RenderOnce for KvTable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let on_add_cb = self.on_add.clone();
        let id_prefix = self.id_prefix.clone();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(8.))
            // Table Header
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(6.))
                    .py(px(4.))
                    .gap(px(8.))
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(t.text_muted)
                    .border_b_1()
                    .border_color(t.border)
                    .child(div().w(px(20.)).child(""))
                    .child(div().w(px(220.)).child("Key"))
                    .child(div().flex_1().child("Value"))
                    .child(div().w(px(24.)).child("")),
            )
            // Read-only rows (e.g. inherited headers): disabled inputs, info popover
            // in place of the delete button. Muted checkbox so it never reads as an
            // editable request row.
            .children(self.readonly_rows.iter().enumerate().map(|(idx, row)| {
                let row_id = format!("{}-ro-{}", id_prefix, idx);
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(6.))
                    .py(px(2.))
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(20.))
                            .h(px(20.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(if row.enabled { t.text_muted } else { t.border })
                            .bg(if row.enabled {
                                t.text_muted
                            } else {
                                t.bg_input
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(t.bg_input)
                            .child(if row.enabled { "✓" } else { "" }),
                    )
                    .child(div().w(px(220.)).child(render_disabled_input(&row.key, t)))
                    .child(div().flex_1().child(render_disabled_input(&row.value, t)))
                    .child(
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Popover::new(SharedString::from(format!("{row_id}-info")))
                                    .trigger(
                                        Button::new(SharedString::from(format!(
                                            "{row_id}-info-btn"
                                        )))
                                        .ghost()
                                        .compact()
                                        .child(
                                            Icon::new(IconName::Info)
                                                .size(px(13.))
                                                .text_color(t.text_muted),
                                        ),
                                    )
                                    .content({
                                        let detail = row.source_detail.clone();
                                        move |_, _, _| div().text_xs().child(detail.clone())
                                    }),
                            ),
                    )
            }))
            // Editable rows
            .children(self.pairs.into_iter().enumerate().map(|(idx, pair)| {
                let on_update = self.on_update.clone();
                let on_remove = self.on_remove.clone();
                let key_val = pair.key.clone();
                let val_val = pair.value.clone();
                let enabled = pair.enabled;
                let row_prefix = format!("{}-row-{}", id_prefix, idx);

                let on_up_k = on_update.clone();
                let on_up_v = on_update.clone();
                let env_for_init = self.env_state.clone();
                let k_init_val = key_val.clone();
                let v_init_val = val_val.clone();
                let row_holder = window.use_keyed_state(
                    gpui::SharedString::from(format!("{}-holder", row_prefix)),
                    cx,
                    |window, cx| {
                        let init_k = if let Some(env_m) = &env_for_init {
                            build_tokenized_content(&k_init_val, env_m.read(cx))
                        } else {
                            k_init_val.into()
                        };
                        let init_v = if let Some(env_m) = &env_for_init {
                            build_tokenized_content(&v_init_val, env_m.read(cx))
                        } else {
                            v_init_val.into()
                        };
                        let key_input = cx.new(|cx| {
                            let mut s = InputState::new(window, cx).placeholder("Key...");
                            s.set_value(init_k, window, cx);
                            s
                        });
                        let val_input = cx.new(|cx| {
                            let mut s = InputState::new(window, cx).placeholder("Value...");
                            s.set_value(init_v, window, cx);
                            s
                        });

                        let v_in_1 = val_input.clone();
                        let env_sub_k = env_for_init.clone();
                        let sub1 = cx.subscribe_in(
                            &key_input,
                            window,
                            move |_, inp, event, window, cx| {
                                if let InputEvent::Change = event {
                                    let k = inp.read(cx).value().to_string();
                                    let v = v_in_1.read(cx).value().to_string();
                                    on_up_k(idx, k.clone(), v, enabled, window, cx);
                                    if k.contains("}}") && detect_unclosed_env_var(&k).is_none() {
                                        if let Some(env_m) = &env_sub_k {
                                            let tokenized =
                                                build_tokenized_content(&k, env_m.read(cx));
                                            inp.update(cx, |i, cx| {
                                                i.set_value(tokenized, window, cx);
                                            });
                                        }
                                    }
                                }
                            },
                        );

                        let k_in_2 = key_input.clone();
                        let env_sub_v = env_for_init.clone();
                        let sub2 = cx.subscribe_in(
                            &val_input,
                            window,
                            move |_, inp, event, window, cx| {
                                if let InputEvent::Change = event {
                                    let k = k_in_2.read(cx).value().to_string();
                                    let v = inp.read(cx).value().to_string();
                                    on_up_v(idx, k, v.clone(), enabled, window, cx);
                                    if v.contains("}}") && detect_unclosed_env_var(&v).is_none() {
                                        if let Some(env_m) = &env_sub_v {
                                            let tokenized =
                                                build_tokenized_content(&v, env_m.read(cx));
                                            inp.update(cx, |i, cx| {
                                                i.set_value(tokenized, window, cx);
                                            });
                                        }
                                    }
                                }
                            },
                        );

                        KvRowInputHolder {
                            key_input,
                            val_input,
                            _subscriptions: vec![sub1, sub2],
                        }
                    },
                );

                let (key_input, val_input) = {
                    let h = row_holder.read(cx);
                    (h.key_input.clone(), h.val_input.clone())
                };

                let on_update_k_render = {
                    let on_update = on_update.clone();
                    let val_in = val_input.clone();
                    move |new_k: String, window: &mut Window, cx: &mut App| {
                        let cur_v = val_in.read(cx).value().to_string();
                        on_update(idx, new_k, cur_v, enabled, window, cx);
                    }
                };

                let on_update_v_render = {
                    let on_update = on_update.clone();
                    let key_in = key_input.clone();
                    move |new_v: String, window: &mut Window, cx: &mut App| {
                        let cur_k = key_in.read(cx).value().to_string();
                        on_update(idx, cur_k, new_v, enabled, window, cx);
                    }
                };

                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(6.))
                    .py(px(2.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .hover(|s| s.bg(t.bg_hover))
                    // Checkbox
                    .child({
                        let on_update_c = on_update.clone();
                        let k = key_val.clone();
                        let v = val_val.clone();
                        div()
                            .id(gpui::SharedString::from(format!("{}-check", row_prefix)))
                            .w(px(20.))
                            .h(px(20.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(if enabled { t.primary } else { t.border })
                            .bg(if enabled { t.primary } else { t.bg_input })
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(t.primary_foreground)
                            .on_click(move |_, window, cx| {
                                on_update_c(idx, k.clone(), v.clone(), !enabled, window, cx);
                            })
                            .child(if enabled { "✓" } else { "" })
                    })
                    // Key Column
                    .child(div().w(px(220.)).child(render_env_input(
                        format!("{}-key", row_prefix),
                        &key_input,
                        self.env_state.as_ref(),
                        self.modal_state.as_ref(),
                        window,
                        cx,
                        on_update_k_render,
                    )))
                    // Value Column
                    .child(div().flex_1().child(render_env_input(
                        format!("{}-val", row_prefix),
                        &val_input,
                        self.env_state.as_ref(),
                        self.modal_state.as_ref(),
                        window,
                        cx,
                        on_update_v_render,
                    )))
                    // Delete Button
                    .child({
                        div()
                            .id(gpui::SharedString::from(format!("{}-del", row_prefix)))
                            .w(px(24.))
                            .h(px(24.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(t.bg_hover).text_color(t.status_5xx))
                            .text_color(t.text_muted)
                            .text_xs()
                            .on_click(move |_, window, cx| {
                                on_remove(idx, window, cx);
                            })
                            .child("×")
                    })
            }))
            // Add row button
            .child(
                div().pt(px(6.)).flex().items_center().gap(px(8.)).child(
                    div()
                        .id(gpui::SharedString::from(format!("{}-btn-add", id_prefix)))
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(4.))
                        .bg(t.bg_card)
                        .border_1()
                        .border_color(t.border)
                        .cursor_pointer()
                        .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(t.text_secondary)
                        .on_click(move |_, window, cx| {
                            on_add_cb(window, cx);
                        })
                        .child("+ Add Row"),
                ),
            )
    }
}
