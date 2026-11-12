use crate::components::env_autocomplete::{build_tokenized_content, detect_unclosed_env_var};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext as _, Entity, Subscription};
use zest_core::models::MultipartItem;

use super::MultipartContext;

/// Snapshot of a row's input handles and metadata, taken right before building
/// the row elements so the render pass never re-reads the table state.
#[derive(Clone)]
pub(super) struct MpRowSnapshot {
    pub(super) key_input: Entity<InputState>,
    pub(super) val_input: Entity<InputState>,
    pub(super) is_file: bool,
    pub(super) file_path: Option<String>,
    pub(super) enabled: bool,
}

/// Current values of a row at click time. Mirrors the `unwrap_or` fallbacks of
/// the original inline lookups (`enabled` defaults to `true`).
#[derive(Clone)]
pub(super) struct RowMeta {
    pub(super) key: String,
    pub(super) value: String,
    pub(super) is_file: bool,
    pub(super) file_path: Option<String>,
    pub(super) enabled: bool,
}

impl Default for RowMeta {
    fn default() -> Self {
        Self {
            key: String::new(),
            value: String::new(),
            is_file: false,
            file_path: None,
            enabled: true,
        }
    }
}

pub(super) struct MpRowInputs {
    key_input: Entity<InputState>,
    val_input: Entity<InputState>,
    is_file: bool,
    file_path: Option<String>,
    enabled: bool,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct MpTableState {
    pub(super) rows: Vec<MpRowInputs>,
}

impl MpTableState {
    pub(super) fn snapshots(&self) -> Vec<MpRowSnapshot> {
        self.rows
            .iter()
            .map(|r| MpRowSnapshot {
                key_input: r.key_input.clone(),
                val_input: r.val_input.clone(),
                is_file: r.is_file,
                file_path: r.file_path.clone(),
                enabled: r.enabled,
            })
            .collect()
    }

    pub(super) fn position_by_key(&self, probe: &Entity<InputState>) -> Option<usize> {
        self.rows.iter().position(|r| &r.key_input == probe)
    }

    pub(super) fn position_by_val(&self, probe: &Entity<InputState>) -> Option<usize> {
        self.rows.iter().position(|r| &r.val_input == probe)
    }

    pub(super) fn meta_at(&self, idx: usize, cx: &App) -> RowMeta {
        self.rows
            .get(idx)
            .map(|r| RowMeta {
                key: r.key_input.read(cx).value().to_string(),
                value: r.val_input.read(cx).value().to_string(),
                is_file: r.is_file,
                file_path: r.file_path.clone(),
                enabled: r.enabled,
            })
            .unwrap_or_default()
    }

    pub(super) fn set_enabled(&mut self, idx: usize, enabled: bool) {
        if idx < self.rows.len() {
            self.rows[idx].enabled = enabled;
        }
    }

    pub(super) fn set_is_file(&mut self, idx: usize, is_file: bool) {
        if idx < self.rows.len() {
            self.rows[idx].is_file = is_file;
        }
    }

    pub(super) fn set_file_path(&mut self, idx: usize, file_path: Option<String>) {
        if idx < self.rows.len() {
            self.rows[idx].file_path = file_path;
        }
    }

    pub(super) fn remove_row(&mut self, idx: usize) {
        if idx < self.rows.len() {
            self.rows.remove(idx);
        }
    }

    fn create_row_inputs(
        item: &MultipartItem,
        ctx: &MultipartContext<'_>,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> MpRowInputs {
        let init_k = item.key.clone();
        let init_v = item.value.clone();
        let key_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Key...");
            if let Some(env_ent) = ctx.env_state {
                let content = build_tokenized_content(&init_k, env_ent.read(cx));
                s.set_value(content, window, cx);
            } else {
                s.set_value(init_k, window, cx);
            }
            s
        });
        let val_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Value...");
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
            if let InputEvent::Change = event {
                let k = inp.read(cx).value().to_string();
                let v = val_ent.read(cx).value().to_string();
                let Some(idx) = this.position_by_key(&key_ent) else {
                    return;
                };
                let meta = this.meta_at(idx, cx);
                if let Some(ws) = &ws_up {
                    ws.update(cx, |ws, _| {
                        ws.update_active_request(&id_k, |r| {
                            r.update_multipart_item(
                                idx,
                                k.clone(),
                                v,
                                meta.is_file,
                                meta.file_path.clone(),
                                meta.enabled,
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
        });

        let key_ent2 = key_input.clone();
        let val_ent2 = val_input.clone();
        let id_v = ctx.req_id_str.to_string();
        let ws_up_v = ctx.workspace_state.cloned();
        let ed_up_v = ctx.editor_state.clone();
        let env_sub2 = ctx.env_state.cloned();

        let sub2 = cx.subscribe_in(&val_input, window, move |this, inp, event, window, cx| {
            if let InputEvent::Change = event {
                let v = inp.read(cx).value().to_string();
                let k = key_ent2.read(cx).value().to_string();
                let Some(idx) = this.position_by_val(&val_ent2) else {
                    return;
                };
                let meta = this.meta_at(idx, cx);
                if let Some(ws) = &ws_up_v {
                    ws.update(cx, |ws, _| {
                        ws.update_active_request(&id_v, |r| {
                            r.update_multipart_item(
                                idx,
                                k,
                                v.clone(),
                                meta.is_file,
                                meta.file_path.clone(),
                                meta.enabled,
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

    pub(super) fn reconcile(
        &mut self,
        items: &[MultipartItem],
        ctx: &MultipartContext<'_>,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
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
