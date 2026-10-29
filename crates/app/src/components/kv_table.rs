use crate::components::env_autocomplete::{build_tokenized_content, detect_unclosed_env_var};
use crate::components::env_input::render_env_input;
use crate::state::{EnvironmentState, ModalState};
use crate::theme::{theme, AppTheme};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled,
    Subscription, Window,
};
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

/// Standard width for the Key column in key-value tables (160px).
pub const KEY_COLUMN_WIDTH: Pixels = px(160.);

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
        .text_sm()
        .font_family(".SystemMonoFont")
        .text_color(t.text_secondary)
        .truncate()
        .child(value.to_string())
}

pub type KvUpdateCallback = Rc<dyn Fn(usize, String, String, bool, &mut Window, &mut App)>;

pub struct KvRowInputs {
    pub key_input: Entity<InputState>,
    pub val_input: Entity<InputState>,
    pub enabled: bool,
    _subscriptions: Vec<Subscription>,
}

pub struct KvTableState {
    pub rows: Vec<KvRowInputs>,
}

impl KvTableState {
    fn create_row_inputs(
        pair: &KeyValuePair,
        env_state: Option<&Entity<EnvironmentState>>,
        on_update: KvUpdateCallback,
        window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> KvRowInputs {
        let init_k = if let Some(env_m) = env_state {
            build_tokenized_content(&pair.key, env_m.read(cx))
        } else {
            pair.key.clone().into()
        };
        let init_v = if let Some(env_m) = env_state {
            build_tokenized_content(&pair.value, env_m.read(cx))
        } else {
            pair.value.clone().into()
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

        let key_ent = key_input.clone();
        let val_ent = val_input.clone();
        let on_up_k = on_update.clone();
        let env_sub_k = env_state.cloned();

        let sub1 = cx.subscribe_in(&key_input, window, move |this, inp, event, window, cx| {
            if let InputEvent::Change = event {
                let k = inp.read(cx).value().to_string();
                let current_idx = this.rows.iter().position(|r| r.key_input == key_ent);
                let current_v = val_ent.read(cx).value().to_string();
                let enabled = current_idx.map(|i| this.rows[i].enabled).unwrap_or(true);
                if let Some(idx) = current_idx {
                    on_up_k(idx, k.clone(), current_v, enabled, window, cx);
                    if k.contains("}}") && detect_unclosed_env_var(&k).is_none() {
                        if let Some(env_m) = &env_sub_k {
                            let tokenized = build_tokenized_content(&k, env_m.read(cx));
                            inp.update(cx, |i, cx| {
                                i.set_value(tokenized, window, cx);
                            });
                        }
                    }
                }
            }
        });

        let key_ent2 = key_input.clone();
        let val_ent2 = val_input.clone();
        let on_up_v = on_update;
        let env_sub_v = env_state.cloned();

        let sub2 = cx.subscribe_in(&val_input, window, move |this, inp, event, window, cx| {
            if let InputEvent::Change = event {
                let v = inp.read(cx).value().to_string();
                let current_idx = this.rows.iter().position(|r| r.val_input == val_ent2);
                let current_k = key_ent2.read(cx).value().to_string();
                let enabled = current_idx.map(|i| this.rows[i].enabled).unwrap_or(true);
                if let Some(idx) = current_idx {
                    on_up_v(idx, current_k, v.clone(), enabled, window, cx);
                    if v.contains("}}") && detect_unclosed_env_var(&v).is_none() {
                        if let Some(env_m) = &env_sub_v {
                            let tokenized = build_tokenized_content(&v, env_m.read(cx));
                            inp.update(cx, |i, cx| {
                                i.set_value(tokenized, window, cx);
                            });
                        }
                    }
                }
            }
        });

        KvRowInputs {
            key_input,
            val_input,
            enabled: pair.enabled,
            _subscriptions: vec![sub1, sub2],
        }
    }

    fn reconcile(
        &mut self,
        pairs: &[KeyValuePair],
        env_state: Option<&Entity<EnvironmentState>>,
        on_update: &KvUpdateCallback,
        window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) {
        if self.rows.len() > pairs.len() {
            self.rows.truncate(pairs.len());
        }

        for (idx, pair) in pairs.iter().enumerate() {
            if idx < self.rows.len() {
                self.rows[idx].enabled = pair.enabled;
                let cur_k = self.rows[idx].key_input.read(cx).value().to_string();
                if cur_k != pair.key {
                    let content = if let Some(env_m) = env_state {
                        build_tokenized_content(&pair.key, env_m.read(cx))
                    } else {
                        pair.key.clone().into()
                    };
                    self.rows[idx].key_input.update(cx, |inp, cx| {
                        inp.set_value(content, window, cx);
                    });
                }
                let cur_v = self.rows[idx].val_input.read(cx).value().to_string();
                if cur_v != pair.value {
                    let content = if let Some(env_m) = env_state {
                        build_tokenized_content(&pair.value, env_m.read(cx))
                    } else {
                        pair.value.clone().into()
                    };
                    self.rows[idx].val_input.update(cx, |inp, cx| {
                        inp.set_value(content, window, cx);
                    });
                }
            } else {
                self.rows.push(Self::create_row_inputs(
                    pair,
                    env_state,
                    on_update.clone(),
                    window,
                    cx,
                ));
            }
        }
    }
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
    show_add_button: bool,
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
            show_add_button: true,
        }
    }

    pub fn show_add_button(mut self, show: bool) -> Self {
        self.show_add_button = show;
        self
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
                        Checkbox::new(format!("{row_id}-chk"))
                            .checked(row.enabled)
                            .disabled(true),
                    )
                    .child(
                        div()
                            .w(KEY_COLUMN_WIDTH)
                            .child(render_disabled_input(&row.key, t)),
                    )
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
                                                .small()
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
            .child({
                let table_state = window.use_keyed_state(
                    gpui_kit::SharedString::from(format!("{}-table-inputs", id_prefix)),
                    cx,
                    |_window, _cx| KvTableState { rows: Vec::new() },
                );

                table_state.update(cx, |st, cx| {
                    st.reconcile(
                        &self.pairs,
                        self.env_state.as_ref(),
                        &self.on_update,
                        window,
                        cx,
                    );
                });

                let row_inputs: Vec<(Entity<InputState>, Entity<InputState>, bool)> = {
                    let st = table_state.read(cx);
                    st.rows
                        .iter()
                        .map(|r| (r.key_input.clone(), r.val_input.clone(), r.enabled))
                        .collect()
                };

                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .children(self.pairs.into_iter().enumerate().map(|(idx, pair)| {
                        let on_update = self.on_update.clone();
                        let on_remove = self.on_remove.clone();
                        let (key_input, val_input, row_enabled) =
                            row_inputs.get(idx).cloned().unwrap_or_else(|| {
                                let dummy = cx.new(|cx| InputState::new(window, cx));
                                (dummy.clone(), dummy, pair.enabled)
                            });
                        let enabled = row_enabled;
                        let row_prefix = format!("{}-row-{}", id_prefix, idx);

                        let ts_k_render = table_state.clone();
                        let key_in_render = key_input.clone();
                        let on_update_k_render = {
                            let on_update = on_update.clone();
                            let val_in = val_input.clone();
                            move |new_k: String, window: &mut Window, cx: &mut App| {
                                let cur_v = val_in.read(cx).value().to_string();
                                let cur_idx = ts_k_render
                                    .read(cx)
                                    .rows
                                    .iter()
                                    .position(|r| r.key_input == key_in_render)
                                    .unwrap_or(idx);
                                on_update(cur_idx, new_k, cur_v, enabled, window, cx);
                            }
                        };

                        let ts_v_render = table_state.clone();
                        let val_in_render = val_input.clone();
                        let on_update_v_render = {
                            let on_update = on_update.clone();
                            let key_in = key_input.clone();
                            move |new_v: String, window: &mut Window, cx: &mut App| {
                                let cur_k = key_in.read(cx).value().to_string();
                                let cur_idx = ts_v_render
                                    .read(cx)
                                    .rows
                                    .iter()
                                    .position(|r| r.val_input == val_in_render)
                                    .unwrap_or(idx);
                                on_update(cur_idx, cur_k, new_v, enabled, window, cx);
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
                                let ts_chk = table_state.clone();
                                let on_update_c = on_update.clone();
                                let key_in_chk = key_input.clone();
                                let val_in_chk = val_input.clone();
                                Checkbox::new(format!("{}-check", row_prefix))
                                    .checked(enabled)
                                    .on_click(move |&next_enabled, window, cx| {
                                        let pos = {
                                            let st = ts_chk.read(cx);
                                            st.rows.iter().position(|r| r.key_input == key_in_chk)
                                        };
                                        if let Some(pos) = pos {
                                            ts_chk.update(cx, |st, _| {
                                                if pos < st.rows.len() {
                                                    st.rows[pos].enabled = next_enabled;
                                                }
                                            });
                                            let cur_k = key_in_chk.read(cx).value().to_string();
                                            let cur_v = val_in_chk.read(cx).value().to_string();
                                            on_update_c(
                                                pos,
                                                cur_k,
                                                cur_v,
                                                next_enabled,
                                                window,
                                                cx,
                                            );
                                        }
                                    })
                            })
                            // Key Column
                            .child(div().w(KEY_COLUMN_WIDTH).child(render_env_input(
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
                                let ts_del = table_state.clone();
                                let key_in_del = key_input.clone();
                                div()
                                    .id(gpui_kit::SharedString::from(format!("{}-del", row_prefix)))
                                    .w(px(24.))
                                    .h(px(24.))
                                    .rounded(px(4.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(t.bg_hover).text_color(t.status_5xx))
                                    .text_color(t.text_muted)
                                    .on_click(move |_, window, cx| {
                                        let pos = {
                                            let st = ts_del.read(cx);
                                            st.rows.iter().position(|r| r.key_input == key_in_del)
                                        };
                                        if let Some(pos) = pos {
                                            ts_del.update(cx, |st, _| {
                                                if pos < st.rows.len() {
                                                    st.rows.remove(pos);
                                                }
                                            });
                                            on_remove(pos, window, cx);
                                        }
                                    })
                                    .child(Icon::new(IconName::Trash).small())
                            })
                    }))
            })
            // Add row button
            .when(self.show_add_button, |this| {
                this.child(
                    div().pt(px(6.)).flex().items_center().gap(px(8.)).child(
                        div()
                            .id(gpui_kit::SharedString::from(format!(
                                "{}-btn-add",
                                id_prefix
                            )))
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
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(t.text_secondary)
                            .on_click(move |_, window, cx| {
                                on_add_cb(window, cx);
                            })
                            .child("+ Add Row"),
                    ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_table_key_column_width() {
        assert_eq!(KEY_COLUMN_WIDTH, px(160.));
    }

    #[test]
    fn test_kv_table_row_removal_model_indices() {
        let mut pairs = vec![
            KeyValuePair::new("k1", "v1"),
            KeyValuePair::new("k2", "v2"),
            KeyValuePair::new("k3", "v3"),
        ];

        // Simulate deleting row 1 (the middle row)
        let removed_idx = 1;
        pairs.remove(removed_idx);

        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].key, "k1");
        assert_eq!(pairs[0].value, "v1");
        assert_eq!(pairs[1].key, "k3");
        assert_eq!(pairs[1].value, "v3");

        // Simulate deleting row 0 (the first row)
        pairs.remove(0);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].key, "k3");
        assert_eq!(pairs[0].value, "v3");
    }
}
