mod file_picker;
mod row;

use crate::env::{build_tokenized_content, detect_unclosed_env_var};
use crate::state::{CollectionState, EnvironmentState, ModalState, RequestSessionState};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    div, px, AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Styled,
    Subscription, Window,
};
use zest_core::models::MultipartItem;

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

/// Current values of a row at click time.
#[derive(Clone, Default)]
pub(super) struct RowMeta {
    pub(super) key: String,
    pub(super) value: String,
    pub(super) is_file: bool,
    pub(super) file_path: Option<String>,
    pub(super) enabled: bool,
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
    pub(super) req_id: String,
    pub(super) workspace_state: Option<Entity<CollectionState>>,
    pub(super) editor_state: Entity<RequestSessionState>,
}

impl MpTableState {
    pub(super) fn new(
        req_id: String,
        workspace_state: Option<Entity<CollectionState>>,
        editor_state: Entity<RequestSessionState>,
    ) -> Self {
        Self {
            rows: Vec::new(),
            req_id,
            workspace_state,
            editor_state,
        }
    }

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

    pub(super) fn sync_row_with(
        &self,
        idx: usize,
        cx: &mut App,
        mutate: impl FnOnce(&mut RowMeta),
    ) {
        let Some(mut meta) = self.rows.get(idx).map(|r| RowMeta {
            key: r.key_input.read(cx).value().to_string(),
            value: r.val_input.read(cx).value().to_string(),
            is_file: r.is_file,
            file_path: r.file_path.clone(),
            enabled: r.enabled,
        }) else {
            return;
        };
        mutate(&mut meta);

        if let Some(ws) = &self.workspace_state {
            let req_id = self.req_id.clone();
            ws.update(cx, |ws, _| {
                ws.update_active_request(&req_id, |r| {
                    r.update_multipart_item(
                        idx,
                        meta.key,
                        meta.value,
                        meta.is_file,
                        meta.file_path,
                        meta.enabled,
                    );
                });
            });
        }
        self.editor_state
            .update(cx, |state, cx| state.mark_dirty(cx));
    }

    pub(super) fn sync_row(&self, idx: usize, cx: &mut App) {
        self.sync_row_with(idx, cx, |_| {});
    }

    pub(super) fn set_enabled(&mut self, idx: usize, enabled: bool, cx: &mut App) {
        if idx < self.rows.len() {
            self.rows[idx].enabled = enabled;
            self.sync_row(idx, cx);
        }
    }

    pub(super) fn set_is_file(&mut self, idx: usize, is_file: bool, cx: &mut App) {
        if idx < self.rows.len() {
            self.rows[idx].is_file = is_file;
            self.sync_row(idx, cx);
        }
    }

    pub(super) fn set_file_path(&mut self, idx: usize, file_path: Option<String>, cx: &mut App) {
        if idx < self.rows.len() {
            self.rows[idx].file_path = file_path;
            self.rows[idx].is_file = true;
            self.sync_row(idx, cx);
        }
    }

    pub(super) fn update_key(&mut self, idx: usize, key: String, cx: &mut App) {
        if idx < self.rows.len() {
            self.sync_row_with(idx, cx, |m| m.key = key);
        }
    }

    pub(super) fn update_val(&mut self, idx: usize, val: String, cx: &mut App) {
        if idx < self.rows.len() {
            self.sync_row_with(idx, cx, |m| m.value = val);
        }
    }

    pub(super) fn remove_row(&mut self, idx: usize, cx: &mut App) {
        if idx < self.rows.len() {
            self.rows.remove(idx);
            if let Some(ws) = &self.workspace_state {
                let req_id = self.req_id.clone();
                ws.update(cx, |ws, _| {
                    ws.update_active_request(&req_id, |r| {
                        r.remove_multipart_item(idx);
                    });
                });
            }
            self.editor_state
                .update(cx, |state, cx| state.mark_dirty(cx));
        }
    }

    fn create_row_inputs(
        item: &MultipartItem,
        env_state: Option<&Entity<EnvironmentState>>,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> MpRowInputs {
        let init_k = item.key.clone();
        let init_v = item.value.clone();
        let key_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Key...");
            if let Some(env_ent) = env_state {
                let content = build_tokenized_content(&init_k, env_ent.read(cx));
                s.set_value(content, window, cx);
            } else {
                s.set_value(init_k, window, cx);
            }
            s
        });
        let val_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Value...");
            if let Some(env_ent) = env_state {
                let content = build_tokenized_content(&init_v, env_ent.read(cx));
                s.set_value(content, window, cx);
            } else {
                s.set_value(init_v, window, cx);
            }
            s
        });

        let key_ent = key_input.clone();
        let val_ent = val_input.clone();
        let env_sub1 = env_state.cloned();

        let sub1 = cx.subscribe_in(&key_input, window, move |this, inp, event, window, cx| {
            if let InputEvent::Change = event {
                if let Some(idx) = this.position_by_key(&key_ent) {
                    this.sync_row(idx, cx);
                }
                let k = inp.read(cx).value().to_string();
                if k.contains("}}") && detect_unclosed_env_var(&k).is_none() {
                    if let Some(env_ent) = &env_sub1 {
                        let tokenized = build_tokenized_content(&k, env_ent.read(cx));
                        inp.update(cx, |s, cx| s.set_value(tokenized, window, cx));
                    }
                }
            }
        });

        let env_sub2 = env_state.cloned();

        let sub2 = cx.subscribe_in(&val_input, window, move |this, inp, event, window, cx| {
            if let InputEvent::Change = event {
                if let Some(idx) = this.position_by_val(&val_ent) {
                    this.sync_row(idx, cx);
                }
                let v = inp.read(cx).value().to_string();
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
        env_state: Option<&Entity<EnvironmentState>>,
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
                    let content = if let Some(env_ent) = env_state {
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
                    let content = if let Some(env_ent) = env_state {
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
                    .push(Self::create_row_inputs(item, env_state, window, cx));
            }
        }
    }
}

/// Borrows the states the multipart editor reads and mutates while rendering.
pub struct MultipartContext<'a> {
    pub req_id_str: &'a str,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub workspace_state: Option<&'a Entity<CollectionState>>,
    pub env_state: Option<&'a Entity<EnvironmentState>>,
    pub modal_state: Option<&'a Entity<ModalState>>,
}

pub fn render_multipart_editor(
    ctx: MultipartContext<'_>,
    items: &[MultipartItem],
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let items = items.to_vec();
    let req_id_str = ctx.req_id_str.to_string();

    div()
        .size_full()
        .overflow_y_scrollbar()
        .p(px(8.))
        .flex()
        .flex_col()
        .gap(px(6.))
        // Multipart Rows
        .child({
            let table_state = window.use_keyed_state(
                gpui_kit::SharedString::from(format!("mp-table-inputs-{}", req_id_str)),
                cx,
                |_window, _cx| {
                    MpTableState::new(
                        req_id_str.clone(),
                        ctx.workspace_state.cloned(),
                        ctx.editor_state.clone(),
                    )
                },
            );

            table_state.update(cx, |st, cx| {
                st.workspace_state = ctx.workspace_state.cloned();
                st.editor_state = ctx.editor_state.clone();
                st.reconcile(&items, ctx.env_state, window, cx);
            });

            let row_inputs: Vec<MpRowSnapshot> = table_state.read(cx).snapshots();

            let row_ctx = row::RowCtx {
                table_state: &table_state,
                env_state: ctx.env_state,
                modal_state: ctx.modal_state,
                req_id: ctx.req_id_str,
            };

            div()
                .w_full()
                .flex()
                .flex_col()
                .children(items.into_iter().enumerate().map(|(idx, item)| {
                    let snapshot = row_inputs.get(idx).cloned().unwrap_or_else(|| {
                        let dummy = cx.new(|cx| InputState::new(window, cx));
                        MpRowSnapshot {
                            key_input: dummy.clone(),
                            val_input: dummy,
                            is_file: item.is_file,
                            file_path: item.file_path.clone(),
                            enabled: item.enabled,
                        }
                    });
                    row::render_multipart_row(&row_ctx, idx, snapshot, window, cx)
                }))
        })
        .into_any_element()
}
