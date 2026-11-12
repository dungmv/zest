use crate::components::env_input::render_env_input;
use crate::state::{EnvironmentState, ModalState, RequestSessionState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};

use super::state::{MpRowSnapshot, MpTableState};

/// Shared handles each row's callbacks need to reach workspace/session state.
pub(super) struct RowCtx<'a> {
    pub(super) table_state: &'a Entity<MpTableState>,
    pub(super) editor_state: &'a Entity<RequestSessionState>,
    pub(super) workspace_state: Option<&'a Entity<WorkspaceDataState>>,
    pub(super) env_state: Option<&'a Entity<EnvironmentState>>,
    pub(super) modal_state: Option<&'a Entity<ModalState>>,
    pub(super) req_id: &'a str,
}

/// Owned clones of the handles a row callback needs, so closures stay
/// `'static` without capturing the render-time `RowCtx` borrow.
#[derive(Clone)]
struct RowHandles {
    table_state: Entity<MpTableState>,
    key_input: Entity<InputState>,
    editor_state: Entity<RequestSessionState>,
    workspace_state: Option<Entity<WorkspaceDataState>>,
    req_id: String,
}

impl RowHandles {
    fn new(ctx: &RowCtx<'_>, key_input: Entity<InputState>) -> Self {
        Self {
            table_state: ctx.table_state.clone(),
            key_input,
            editor_state: ctx.editor_state.clone(),
            workspace_state: ctx.workspace_state.cloned(),
            req_id: ctx.req_id.to_string(),
        }
    }
}

/// Builds the checked "Text" / "File" menu entry that flips a row's type.
fn type_menu_item(
    label: &'static str,
    target_is_file: bool,
    checked: bool,
    handles: RowHandles,
) -> PopupMenuItem {
    PopupMenuItem::new(label)
        .checked(checked)
        .on_click(move |_, _, cx| {
            let Some(pos) = handles
                .table_state
                .read(cx)
                .position_by_key(&handles.key_input)
            else {
                return;
            };
            let meta = handles.table_state.read(cx).meta_at(pos, cx);
            handles
                .table_state
                .update(cx, |st, _| st.set_is_file(pos, target_is_file));
            if let Some(ws) = &handles.workspace_state {
                ws.update(cx, |ws, _| {
                    ws.update_active_request(&handles.req_id, |r| {
                        r.update_multipart_item(
                            pos,
                            meta.key,
                            meta.value,
                            target_is_file,
                            meta.file_path,
                            meta.enabled,
                        );
                    });
                });
            }
            handles
                .editor_state
                .update(cx, |state, cx| state.mark_dirty(cx));
        })
}

pub(super) fn render_multipart_row(
    ctx: &RowCtx<'_>,
    idx: usize,
    snap: MpRowSnapshot,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let t = theme();
    let mp_prefix = format!("mp-{}-row-{}", ctx.req_id, idx);
    let MpRowSnapshot {
        key_input,
        val_input,
        is_file,
        file_path,
        enabled,
    } = snap;

    div()
        .w_full()
        .flex()
        .items_center()
        .px(px(6.))
        .py(px(4.))
        .gap(px(8.))
        .rounded(px(4.))
        .hover(|s| s.bg(t.bg_hover))
        // Checkbox
        .child({
            let ts_toggle = ctx.table_state.clone();
            let key_in_toggle = key_input.clone();
            let ed_toggle = ctx.editor_state.clone();
            let ws_toggle = ctx.workspace_state.cloned();
            let id_toggle = ctx.req_id.to_string();
            Checkbox::new(format!("check-mp-{}", idx))
                .checked(enabled)
                .on_click(move |&next_enabled, _, cx| {
                    let Some(pos) = ts_toggle.read(cx).position_by_key(&key_in_toggle) else {
                        return;
                    };
                    let meta = ts_toggle.read(cx).meta_at(pos, cx);
                    ts_toggle.update(cx, |st, _| st.set_enabled(pos, next_enabled));
                    if let Some(ws) = &ws_toggle {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&id_toggle, |r| {
                                r.update_multipart_item(
                                    pos,
                                    meta.key,
                                    meta.value,
                                    meta.is_file,
                                    meta.file_path,
                                    next_enabled,
                                );
                            });
                        });
                    }
                    ed_toggle.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                })
        })
        // Key
        .child({
            let ts_k = ctx.table_state.clone();
            let key_in_k = key_input.clone();
            let ed_on_k = ctx.editor_state.clone();
            let ws_on_k = ctx.workspace_state.cloned();
            let id_on_k = ctx.req_id.to_string();
            let on_key_update = move |new_k: String, _window: &mut Window, cx: &mut App| {
                let Some(pos) = ts_k.read(cx).position_by_key(&key_in_k) else {
                    return;
                };
                let meta = ts_k.read(cx).meta_at(pos, cx);
                if let Some(ws) = &ws_on_k {
                    ws.update(cx, |ws, _| {
                        ws.update_active_request(&id_on_k, |r| {
                            r.update_multipart_item(
                                pos,
                                new_k,
                                meta.value,
                                meta.is_file,
                                meta.file_path,
                                meta.enabled,
                            );
                        });
                    });
                }
                ed_on_k.update(cx, |state, cx| {
                    state.mark_dirty(cx);
                });
            };

            div().w(px(160.)).flex_shrink_0().child(render_env_input(
                format!("{}-key", mp_prefix),
                &key_input,
                ctx.env_state,
                ctx.modal_state,
                window,
                cx,
                on_key_update,
            ))
        })
        // Type dropdown: Text | File
        .child({
            let cur_type_label = if is_file { "File" } else { "Text" };
            let menu_handles = RowHandles::new(ctx, key_input.clone());

            Button::new(format!("dropdown-mp-type-{}", idx))
                .secondary()
                .compact()
                .px(px(8.))
                .py(px(2.))
                .h(px(28.))
                .w(px(76.))
                .flex_shrink_0()
                .rounded(px(4.))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .text_color(t.text_primary)
                                .child(cur_type_label),
                        )
                        .child(
                            Icon::new(IconName::ChevronDown)
                                .size(px(12.))
                                .text_color(t.text_muted),
                        ),
                )
                .dropdown_menu(move |menu, _window, _cx| {
                    menu.item(type_menu_item(
                        "Text",
                        false,
                        !is_file,
                        menu_handles.clone(),
                    ))
                    .item(type_menu_item(
                        "File",
                        true,
                        is_file,
                        menu_handles.clone(),
                    ))
                })
        })
        // Value or Path (Single consolidated element fitting flex_1)
        .child({
            div().flex_1().min_w(px(0.)).child(if is_file {
                render_file_picker(ctx, idx, &file_path, &key_input).into_any_element()
            } else {
                let ts_v = ctx.table_state.clone();
                let val_in_v = val_input.clone();
                let ed_on_v = ctx.editor_state.clone();
                let ws_on_v = ctx.workspace_state.cloned();
                let id_on_v = ctx.req_id.to_string();
                let on_val_update = move |new_v: String, _window: &mut Window, cx: &mut App| {
                    let Some(pos) = ts_v.read(cx).position_by_val(&val_in_v) else {
                        return;
                    };
                    let meta = ts_v.read(cx).meta_at(pos, cx);
                    if let Some(ws) = &ws_on_v {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&id_on_v, |r| {
                                r.update_multipart_item(
                                    pos,
                                    meta.key,
                                    new_v,
                                    meta.is_file,
                                    meta.file_path,
                                    meta.enabled,
                                );
                            });
                        });
                    }
                    ed_on_v.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                };

                render_env_input(
                    format!("{}-val", mp_prefix),
                    &val_input,
                    ctx.env_state,
                    ctx.modal_state,
                    window,
                    cx,
                    on_val_update,
                )
                .into_any_element()
            })
        })
        // Delete row
        .child({
            let ts_del = ctx.table_state.clone();
            let key_in_del = key_input.clone();
            let ed_del = ctx.editor_state.clone();
            let ws_del = ctx.workspace_state.cloned();
            let id_del = ctx.req_id.to_string();

            div()
                .id(gpui_kit::SharedString::from(format!("del-mp-row-{}", idx)))
                .size(px(24.))
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(t.text_muted)
                .hover(move |s| s.text_color(t.json_null).bg(t.json_null.opacity(0.12)))
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new("Delete field").build(window, cx)
                })
                .on_click(move |_, _, cx: &mut App| {
                    let Some(pos) = ts_del.read(cx).position_by_key(&key_in_del) else {
                        return;
                    };
                    ts_del.update(cx, |st, _| st.remove_row(pos));
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
                })
                .child(Icon::new(IconName::Trash).size(px(14.)))
        })
        .into_any_element()
}

/// Read-only "choose file" button shown for `File` rows; spawns the native
/// file dialog off the UI thread and writes the picked path back.
fn render_file_picker(
    ctx: &RowCtx<'_>,
    idx: usize,
    file_path: &Option<String>,
    key_input: &Entity<InputState>,
) -> impl IntoElement {
    let t = theme();
    let ts_b = ctx.table_state.clone();
    let kin_b = key_input.clone();
    let ed_bg = ctx.editor_state.clone();
    let ws_bg = ctx.workspace_state.cloned();
    let id_bg = ctx.req_id.to_string();
    let has_file = file_path.is_some();
    let display_text = file_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Choose file...")
        .to_string();
    let tip_path = file_path.clone();

    div()
        .id(format!("btn-select-file-{}", idx))
        .h(px(28.))
        .w_full()
        .min_w(px(0.))
        .px(px(8.))
        .rounded(px(4.))
        .bg(if has_file { t.bg_card } else { t.bg_input })
        .border_1()
        .border_color(t.border)
        .hover(|s| s.bg(t.bg_hover).border_color(t.border_focus))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(px(6.))
        .when_some(tip_path, |this, path| {
            this.tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(path.clone()).build(window, cx)
            })
        })
        .on_click(move |_, _, cx| {
            let ed_bg = ed_bg.clone();
            let ws_bg = ws_bg.clone();
            let id_bg = id_bg.clone();
            let ts_b = ts_b.clone();
            let kin_b = kin_b.clone();
            cx.spawn(async move |cx| {
                if let Some(file) = rfd::AsyncFileDialog::new().pick_file().await {
                    let p = file.path().to_string_lossy().to_string();
                    cx.update(|cx| {
                        let Some(pos) = ts_b.read(cx).position_by_key(&kin_b) else {
                            return;
                        };
                        let meta = ts_b.read(cx).meta_at(pos, cx);
                        ts_b.update(cx, |st, _| st.set_file_path(pos, Some(p.clone())));
                        if let Some(ws) = &ws_bg {
                            ws.update(cx, |ws, _| {
                                ws.update_active_request(&id_bg, |r| {
                                    r.update_multipart_item(
                                        pos,
                                        meta.key,
                                        meta.value,
                                        true,
                                        Some(p),
                                        meta.enabled,
                                    );
                                });
                            });
                        }
                        ed_bg.update(cx, |state, cx| {
                            state.mark_dirty(cx);
                        });
                    });
                }
            })
            .detach();
        })
        .child(
            Icon::new(if has_file {
                IconName::File
            } else {
                IconName::Folder
            })
            .size(px(14.))
            .flex_shrink_0()
            .text_color(if has_file {
                t.text_primary
            } else {
                t.text_muted
            }),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_xs()
                .font_family(if has_file { ".SystemMonoFont" } else { "" })
                .text_color(if has_file {
                    t.text_primary
                } else {
                    t.text_muted
                })
                .child(display_text),
        )
}
