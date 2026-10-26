use crate::components::env_autocomplete::{build_tokenized_content, detect_unclosed_env_var};
use crate::components::env_input::render_env_input;
use crate::state::{EnvironmentState, ModalState, RequestSessionState, WorkspaceDataState};
use crate::theme::theme;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, AnyElement, App, AppContext as _, Entity, InteractiveElement as _, IntoElement,
    ParentElement, StatefulInteractiveElement as _, Styled, Window,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::Icon;
use gpui_kit::assets::IconName;
use std::cell::RefCell;
use std::rc::Rc;
use zest_core::models::MultipartItem;

pub struct MultipartContext<'a> {
    pub req_id_str: &'a str,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub workspace_state: Option<&'a Entity<WorkspaceDataState>>,
    pub env_state: Option<&'a Entity<EnvironmentState>>,
    pub modal_state: Option<&'a Entity<ModalState>>,
}

#[derive(Clone)]
struct MpRowSnapshot {
    key_input: Entity<gpui_component::input::InputState>,
    val_input: Entity<gpui_component::input::InputState>,
    is_file: bool,
    file_path: Option<String>,
    enabled: bool,
}

struct MpRowInputs {
    key_input: Entity<gpui_component::input::InputState>,
    val_input: Entity<gpui_component::input::InputState>,
    is_file: bool,
    file_path: Option<String>,
    enabled: bool,
    _subscriptions: Vec<gpui::Subscription>,
}

struct MpTableState {
    rows: Vec<MpRowInputs>,
}

impl MpTableState {
    fn create_row_inputs(
        item: &MultipartItem,
        ctx: &MultipartContext<'_>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> MpRowInputs {
        let init_k = item.key.clone();
        let init_v = item.value.clone();
        let key_input = cx.new(|cx| {
            let mut s = gpui_component::input::InputState::new(window, cx).placeholder("Key...");
            if let Some(env_ent) = ctx.env_state {
                let content = build_tokenized_content(&init_k, env_ent.read(cx));
                s.set_value(content, window, cx);
            } else {
                s.set_value(init_k, window, cx);
            }
            s
        });
        let val_input = cx.new(|cx| {
            let mut s = gpui_component::input::InputState::new(window, cx).placeholder("Value...");
            if let Some(env_ent) = ctx.env_state {
                let content = build_tokenized_content(&init_v, env_ent.read(cx));
                s.set_value(content, window, cx);
            } else {
                s.set_value(init_v, window, cx);
            }
            s
        });

        let key_ent = key_input.clone();
        let val_ent = val_input.clone();
        let id_k = ctx.req_id_str.to_string();
        let ws_up = ctx.workspace_state.cloned();
        let ed_up_k = ctx.editor_state.clone();
        let env_sub1 = ctx.env_state.cloned();

        let sub1 = cx.subscribe_in(&key_input, window, move |this, inp, event, window, cx| {
            if let gpui_component::input::InputEvent::Change = event {
                let k = inp.read(cx).value().to_string();
                let v = val_ent.read(cx).value().to_string();
                let (current_idx, is_file, file_path, enabled) = {
                    let idx = this.rows.iter().position(|r| r.key_input == key_ent);
                    let meta = idx
                        .map(|i| {
                            (
                                this.rows[i].is_file,
                                this.rows[i].file_path.clone(),
                                this.rows[i].enabled,
                            )
                        })
                        .unwrap_or((false, None, true));
                    (idx, meta.0, meta.1, meta.2)
                };
                if let Some(idx) = current_idx {
                    if let Some(ws) = &ws_up {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&id_k, |r| {
                                r.update_multipart_item(
                                    idx,
                                    k.clone(),
                                    v,
                                    is_file,
                                    file_path,
                                    enabled,
                                );
                            });
                        });
                    }
                    ed_up_k.update(cx, |state, cx| state.mark_dirty(cx));
                    if k.contains("}}") && detect_unclosed_env_var(&k).is_none() {
                        if let Some(env_ent) = &env_sub1 {
                            let tokenized = build_tokenized_content(&k, env_ent.read(cx));
                            inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                        }
                    }
                }
            }
        });

        let key_ent2 = key_input.clone();
        let val_ent2 = val_input.clone();
        let id_v = ctx.req_id_str.to_string();
        let ws_up_v = ctx.workspace_state.cloned();
        let ed_up_v = ctx.editor_state.clone();
        let env_sub2 = ctx.env_state.cloned();

        let sub2 = cx.subscribe_in(&val_input, window, move |this, inp, event, window, cx| {
            if let gpui_component::input::InputEvent::Change = event {
                let v = inp.read(cx).value().to_string();
                let k = key_ent2.read(cx).value().to_string();
                let (current_idx, is_file, file_path, enabled) = {
                    let idx = this.rows.iter().position(|r| r.val_input == val_ent2);
                    let meta = idx
                        .map(|i| {
                            (
                                this.rows[i].is_file,
                                this.rows[i].file_path.clone(),
                                this.rows[i].enabled,
                            )
                        })
                        .unwrap_or((false, None, true));
                    (idx, meta.0, meta.1, meta.2)
                };
                if let Some(idx) = current_idx {
                    if let Some(ws) = &ws_up_v {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&id_v, |r| {
                                r.update_multipart_item(
                                    idx,
                                    k,
                                    v.clone(),
                                    is_file,
                                    file_path,
                                    enabled,
                                );
                            });
                        });
                    }
                    ed_up_v.update(cx, |state, cx| state.mark_dirty(cx));
                    if v.contains("}}") && detect_unclosed_env_var(&v).is_none() {
                        if let Some(env_ent) = &env_sub2 {
                            let tokenized = build_tokenized_content(&v, env_ent.read(cx));
                            inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                        }
                    }
                }
            }
        });

        MpRowInputs {
            key_input,
            val_input,
            is_file: item.is_file,
            file_path: item.file_path.clone(),
            enabled: item.enabled,
            _subscriptions: vec![sub1, sub2],
        }
    }

    fn reconcile(
        &mut self,
        items: &[MultipartItem],
        ctx: &MultipartContext<'_>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.rows.len() > items.len() {
            self.rows.truncate(items.len());
        }

        for (idx, item) in items.iter().enumerate() {
            if idx < self.rows.len() {
                self.rows[idx].is_file = item.is_file;
                self.rows[idx].file_path = item.file_path.clone();
                self.rows[idx].enabled = item.enabled;
                let cur_k = self.rows[idx].key_input.read(cx).value().to_string();
                if cur_k != item.key {
                    let content = if let Some(env_ent) = ctx.env_state {
                        build_tokenized_content(&item.key, env_ent.read(cx))
                    } else {
                        item.key.clone().into()
                    };
                    self.rows[idx]
                        .key_input
                        .update(cx, |inp, cx| inp.set_value(content, window, cx));
                }
                let cur_v = self.rows[idx].val_input.read(cx).value().to_string();
                if cur_v != item.value {
                    let content = if let Some(env_ent) = ctx.env_state {
                        build_tokenized_content(&item.value, env_ent.read(cx))
                    } else {
                        item.value.clone().into()
                    };
                    self.rows[idx]
                        .val_input
                        .update(cx, |inp, cx| inp.set_value(content, window, cx));
                }
            } else {
                self.rows
                    .push(Self::create_row_inputs(item, ctx, window, cx));
            }
        }
    }
}

pub fn render_multipart_editor(
    ctx: MultipartContext<'_>,
    items: &[MultipartItem],
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let t = theme();
    let items = items.to_vec();
    let ed_c = ctx.editor_state.clone();
    let req_id_str = ctx.req_id_str.to_string();
    let workspace_state = ctx.workspace_state;
    let env_state = ctx.env_state;
    let modal_state = ctx.modal_state;

    div()
        .size_full()
        .overflow_y_scrollbar()
        .p(px(8.))
        .flex()
        .flex_col()
        .gap(px(6.))
        // Multipart Table Header
        .child(
            div()
                .flex()
                .items_center()
                .px(px(6.))
                .py(px(4.))
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(t.text_muted)
                .border_b_1()
                .border_color(t.border)
                .child(div().w(px(24.)).child(""))
                .child(div().w(px(160.)).child("Key"))
                .child(div().w(px(70.)).child("Type"))
                .child(div().flex_1().child("Value / File Path"))
                .child(div().w(px(80.)).child("Browse"))
                .child(div().w(px(28.)).child("")),
        )
        // Multipart Rows
        .child({
            let table_state = window.use_keyed_state(
                gpui::SharedString::from(format!("mp-table-inputs-{}", req_id_str)),
                cx,
                |_window, _cx| MpTableState { rows: Vec::new() },
            );

            table_state.update(cx, |st, cx| {
                st.reconcile(&items, &ctx, window, cx);
            });

            let row_inputs: Vec<MpRowSnapshot> = {
                let st = table_state.read(cx);
                st.rows
                    .iter()
                    .map(|r| MpRowSnapshot {
                        key_input: r.key_input.clone(),
                        val_input: r.val_input.clone(),
                        is_file: r.is_file,
                        file_path: r.file_path.clone(),
                        enabled: r.enabled,
                    })
                    .collect()
            };

            div()
                .w_full()
                .flex()
                .flex_col()
                .children(items.into_iter().enumerate().map(|(idx, item)| {
                    let ed_row = ed_c.clone();
                    let ed_del = ed_c.clone();
                    let ed_pick = ed_c.clone();
                    let ws_row = workspace_state.cloned();
                    let ws_del = ws_row.clone();
                    let ws_pick = ws_row.clone();
                    let id_row = req_id_str.clone();
                    let id_del = id_row.clone();
                    let id_pick = id_row.clone();
                    let snapshot = row_inputs.get(idx).cloned().unwrap_or_else(|| {
                        let dummy = cx.new(|cx| gpui_component::input::InputState::new(window, cx));
                        MpRowSnapshot {
                            key_input: dummy.clone(),
                            val_input: dummy,
                            is_file: item.is_file,
                            file_path: item.file_path.clone(),
                            enabled: item.enabled,
                        }
                    });
                    let key_input = snapshot.key_input;
                    let val_input = snapshot.val_input;
                    let is_file = snapshot.is_file;
                    let file_path = snapshot.file_path;
                    let enabled = snapshot.enabled;
                    let mp_prefix = format!("mp-{}-row-{}", req_id_str, idx);

                    let ts_toggle = table_state.clone();
                    let ts_type = table_state.clone();
                    let ts_del = table_state.clone();
                    let key_in_toggle = key_input.clone();
                    let key_in_type = key_input.clone();
                    let key_in_del = key_input.clone();

                    div()
                        .flex()
                        .items_center()
                        .px(px(6.))
                        .py(px(4.))
                        .gap(px(8.))
                        .rounded(px(4.))
                        .hover(|s| s.bg(t.bg_hover))
                        // Checkbox
                        .child({
                            let ed_toggle = ed_row.clone();
                            let ws_toggle = ws_row.clone();
                            let id_toggle = id_row.clone();
                            div()
                                .id(format!("check-mp-{}", idx))
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
                                .on_click(move |_, _, cx| {
                                    let (current_idx, k, v, is_f, fp, next_enabled) = {
                                        let st = ts_toggle.read(cx);
                                        let pos = st
                                            .rows
                                            .iter()
                                            .position(|r| r.key_input == key_in_toggle);
                                        let meta = pos
                                            .map(|i| {
                                                (
                                                    st.rows[i]
                                                        .key_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string(),
                                                    st.rows[i]
                                                        .val_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string(),
                                                    st.rows[i].is_file,
                                                    st.rows[i].file_path.clone(),
                                                    !st.rows[i].enabled,
                                                )
                                            })
                                            .unwrap_or((
                                                String::new(),
                                                String::new(),
                                                false,
                                                None,
                                                false,
                                            ));
                                        (pos, meta.0, meta.1, meta.2, meta.3, meta.4)
                                    };
                                    if let Some(pos) = current_idx {
                                        ts_toggle.update(cx, |st, _| {
                                            if pos < st.rows.len() {
                                                st.rows[pos].enabled = next_enabled;
                                            }
                                        });
                                        if let Some(ws) = &ws_toggle {
                                            ws.update(cx, |ws, _| {
                                                ws.update_active_request(&id_toggle, |r| {
                                                    r.update_multipart_item(
                                                        pos,
                                                        k,
                                                        v,
                                                        is_f,
                                                        fp,
                                                        next_enabled,
                                                    );
                                                });
                                            });
                                        }
                                        ed_toggle.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    }
                                })
                                .child(if enabled { "✓" } else { "" })
                        })
                        // Key
                        .child({
                            let ed_on_k = ed_row.clone();
                            let ws_on_k = ws_row.clone();
                            let id_on_k = id_row.clone();
                            let ts_k = table_state.clone();
                            let key_in_k = key_input.clone();
                            let on_key_update =
                                move |new_k: String, _window: &mut Window, cx: &mut App| {
                                    let (current_idx, cur_v, is_f, fp, cur_en) = {
                                        let st = ts_k.read(cx);
                                        let pos =
                                            st.rows.iter().position(|r| r.key_input == key_in_k);
                                        let meta = pos
                                            .map(|i| {
                                                (
                                                    st.rows[i]
                                                        .val_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string(),
                                                    st.rows[i].is_file,
                                                    st.rows[i].file_path.clone(),
                                                    st.rows[i].enabled,
                                                )
                                            })
                                            .unwrap_or((String::new(), false, None, true));
                                        (pos, meta.0, meta.1, meta.2, meta.3)
                                    };
                                    if let Some(pos) = current_idx {
                                        if let Some(ws) = &ws_on_k {
                                            ws.update(cx, |ws, _| {
                                                ws.update_active_request(&id_on_k, |r| {
                                                    r.update_multipart_item(
                                                        pos, new_k, cur_v, is_f, fp, cur_en,
                                                    );
                                                });
                                            });
                                        }
                                        ed_on_k.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    }
                                };

                            div().w(px(160.)).child(render_env_input(
                                format!("{}-key", mp_prefix),
                                &key_input,
                                env_state,
                                modal_state,
                                window,
                                cx,
                                on_key_update,
                            ))
                        })
                        // Type toggle: Text | File
                        .child({
                            let ed_toggle_type = ed_row.clone();
                            let ws_toggle_type = ws_row.clone();
                            let id_toggle_type = id_row.clone();
                            div()
                                .id(format!("toggle-mp-type-{}", idx))
                                .w(px(70.))
                                .h(px(28.))
                                .rounded(px(4.))
                                .bg(t.bg_card)
                                .border_1()
                                .border_color(t.border)
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(t.text_secondary)
                                .on_click(move |_, _, cx| {
                                    let (current_idx, k, v, next_is_file, fp, cur_en) = {
                                        let st = ts_type.read(cx);
                                        let pos =
                                            st.rows.iter().position(|r| r.key_input == key_in_type);
                                        let meta = pos
                                            .map(|i| {
                                                (
                                                    st.rows[i]
                                                        .key_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string(),
                                                    st.rows[i]
                                                        .val_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string(),
                                                    !st.rows[i].is_file,
                                                    st.rows[i].file_path.clone(),
                                                    st.rows[i].enabled,
                                                )
                                            })
                                            .unwrap_or((
                                                String::new(),
                                                String::new(),
                                                false,
                                                None,
                                                true,
                                            ));
                                        (pos, meta.0, meta.1, meta.2, meta.3, meta.4)
                                    };
                                    if let Some(pos) = current_idx {
                                        ts_type.update(cx, |st, _| {
                                            if pos < st.rows.len() {
                                                st.rows[pos].is_file = next_is_file;
                                            }
                                        });
                                        if let Some(ws) = &ws_toggle_type {
                                            ws.update(cx, |ws, _| {
                                                ws.update_active_request(&id_toggle_type, |r| {
                                                    r.update_multipart_item(
                                                        pos,
                                                        k,
                                                        v,
                                                        next_is_file,
                                                        fp,
                                                        cur_en,
                                                    );
                                                });
                                            });
                                        }
                                        ed_toggle_type.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    }
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(3.))
                                        .child(
                                            Icon::new(if is_file {
                                                IconName::File
                                            } else {
                                                IconName::Type
                                            })
                                            .size(px(12.)),
                                        )
                                        .child(if is_file { "File" } else { "Text" }),
                                )
                        })
                        // Value or Path
                        .child({
                            let ed_on_v = ed_row;
                            let ws_on_v = ws_row;
                            let id_on_v = id_row;
                            let ts_v = table_state.clone();
                            let val_in_v = val_input.clone();
                            let on_val_update =
                                move |new_v: String, _window: &mut Window, cx: &mut App| {
                                    let (current_idx, cur_k, is_f, fp, cur_en) = {
                                        let st = ts_v.read(cx);
                                        let pos =
                                            st.rows.iter().position(|r| r.val_input == val_in_v);
                                        let meta = pos
                                            .map(|i| {
                                                (
                                                    st.rows[i]
                                                        .key_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string(),
                                                    st.rows[i].is_file,
                                                    st.rows[i].file_path.clone(),
                                                    st.rows[i].enabled,
                                                )
                                            })
                                            .unwrap_or((String::new(), false, None, true));
                                        (pos, meta.0, meta.1, meta.2, meta.3)
                                    };
                                    if let Some(pos) = current_idx {
                                        if let Some(ws) = &ws_on_v {
                                            ws.update(cx, |ws, _| {
                                                ws.update_active_request(&id_on_v, |r| {
                                                    r.update_multipart_item(
                                                        pos, cur_k, new_v, is_f, fp, cur_en,
                                                    );
                                                });
                                            });
                                        }
                                        ed_on_v.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    }
                                };

                            div().flex_1().child(if is_file {
                                div()
                                    .h(px(28.))
                                    .px(px(6.))
                                    .rounded(px(4.))
                                    .bg(t.bg_input)
                                    .border_1()
                                    .border_color(t.border)
                                    .flex()
                                    .items_center()
                                    .child(
                                        div()
                                            .px(px(4.))
                                            .text_xs()
                                            .font_family(".SystemMonoFont")
                                            .text_color(t.text_primary)
                                            .child(
                                                file_path.clone().unwrap_or_else(|| {
                                                    "Select a file...".to_string()
                                                }),
                                            ),
                                    )
                                    .into_any_element()
                            } else {
                                render_env_input(
                                    format!("{}-val", mp_prefix),
                                    &val_input,
                                    env_state,
                                    modal_state,
                                    window,
                                    cx,
                                    on_val_update,
                                )
                                .into_any_element()
                            })
                        })
                        // Browse button (only enabled if is_file)
                        .child({
                            let ed_p = ed_pick;
                            let ws_p = ws_pick;
                            let id_p = id_pick;
                            let ts_browse = table_state.clone();
                            let key_in_browse = key_input;
                            let btn = div()
                                .id(format!("browse-mp-{}", idx))
                                .w(px(80.))
                                .h(px(28.))
                                .rounded(px(4.))
                                .bg(if is_file {
                                    t.bg_card
                                } else {
                                    gpui::transparent_black()
                                })
                                .border_1()
                                .border_color(if is_file {
                                    t.border
                                } else {
                                    gpui::transparent_black()
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(if is_file {
                                    t.text_secondary
                                } else {
                                    t.text_muted
                                });

                            if is_file {
                                btn.cursor_pointer()
                                    .hover(|s| s.bg(t.bg_hover))
                                    .on_click(move |_, _, cx| {
                                        let ed_bg = ed_p.clone();
                                        let ws_bg = ws_p.clone();
                                        let id_bg = id_p.clone();
                                        let ts_b = ts_browse.clone();
                                        let kin_b = key_in_browse.clone();
                                        cx.spawn(async move |cx| {
                                            if let Some(file) =
                                                rfd::AsyncFileDialog::new().pick_file().await
                                            {
                                                let p = file.path().to_string_lossy().to_string();
                                                cx.update(|cx| {
                                                    let (current_idx, k, v, cur_en) = {
                                                        let st = ts_b.read(cx);
                                                        let pos = st
                                                            .rows
                                                            .iter()
                                                            .position(|r| r.key_input == kin_b);
                                                        let meta = pos
                                                            .map(|i| {
                                                                (
                                                                    st.rows[i]
                                                                        .key_input
                                                                        .read(cx)
                                                                        .value()
                                                                        .to_string(),
                                                                    st.rows[i]
                                                                        .val_input
                                                                        .read(cx)
                                                                        .value()
                                                                        .to_string(),
                                                                    st.rows[i].enabled,
                                                                )
                                                            })
                                                            .unwrap_or((
                                                                String::new(),
                                                                String::new(),
                                                                true,
                                                            ));
                                                        (pos, meta.0, meta.1, meta.2)
                                                    };
                                                    if let Some(pos) = current_idx {
                                                        ts_b.update(cx, |st, _| {
                                                            if pos < st.rows.len() {
                                                                st.rows[pos].file_path =
                                                                    Some(p.clone());
                                                            }
                                                        });
                                                        if let Some(ws) = &ws_bg {
                                                            ws.update(cx, |ws, _| {
                                                                ws.update_active_request(
                                                                    &id_bg,
                                                                    |r| {
                                                                        r.update_multipart_item(
                                                                            pos,
                                                                            k,
                                                                            v,
                                                                            true,
                                                                            Some(p),
                                                                            cur_en,
                                                                        );
                                                                    },
                                                                );
                                                            });
                                                        }
                                                        ed_bg.update(cx, |state, cx| {
                                                            state.mark_dirty(cx);
                                                        });
                                                    }
                                                });
                                            }
                                        })
                                        .detach();
                                    })
                                    .child("Browse...")
                            } else {
                                btn.child("")
                            }
                        })
                        // Delete row
                        .child(
                            div()
                                .id(format!("del-mp-row-{}", idx))
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
                                .on_click(move |_, _, cx| {
                                    let current_idx = {
                                        let st = ts_del.read(cx);
                                        st.rows.iter().position(|r| r.key_input == key_in_del)
                                    };
                                    if let Some(pos) = current_idx {
                                        ts_del.update(cx, |st, _| {
                                            if pos < st.rows.len() {
                                                st.rows.remove(pos);
                                            }
                                        });
                                        if let Some(ws) = &ws_del {
                                            ws.update(cx, |ws, _| {
                                                ws.update_active_request(&id_del, |r| {
                                                    r.remove_multipart_item(pos);
                                                });
                                            });
                                        }
                                        ed_del.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    }
                                })
                                .child("×"),
                        )
                }))
        })
        // Add Part button
        .child(div().pt(px(4.)).child({
            let ed_add = ed_c.clone();
            let ws_add = workspace_state.cloned();
            let id_add = req_id_str.clone();
            div()
                .id("btn-add-multipart-part")
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .w(px(90.))
                .bg(t.bg_card)
                .border_1()
                .border_color(t.border)
                .cursor_pointer()
                .hover(|s| s.bg(t.bg_hover))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .text_xs()
                .text_color(t.text_secondary)
                .on_click(move |_, _, cx| {
                    if let Some(ws) = &ws_add {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&id_add, |r| {
                                r.add_multipart_item();
                            });
                        });
                    }
                    ed_add.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                })
                .child("+ Add Part")
        }))
        .into_any_element()
}
