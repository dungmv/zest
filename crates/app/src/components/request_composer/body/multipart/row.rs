use super::file_picker::render_file_picker;
use crate::components::env_input::render_env_input;
use crate::state::{EnvironmentState, ModalState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::{
    div, px, AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};

use super::{MpRowSnapshot, MpTableState};

/// Shared handles each row's callbacks need to reach table state and UI context.
pub(super) struct RowCtx<'a> {
    pub(super) table_state: &'a Entity<MpTableState>,
    pub(super) env_state: Option<&'a Entity<EnvironmentState>>,
    pub(super) modal_state: Option<&'a Entity<ModalState>>,
    pub(super) req_id: &'a str,
}

/// Owned handle to table state and this row's key input, allowing callbacks
/// to dispatch mutations without capturing `RowCtx`.
#[derive(Clone)]
pub(super) struct RowHandles {
    table_state: Entity<MpTableState>,
    key_input: Entity<InputState>,
}

impl RowHandles {
    pub(super) fn new(table_state: Entity<MpTableState>, key_input: Entity<InputState>) -> Self {
        Self {
            table_state,
            key_input,
        }
    }

    pub(super) fn mutate(&self, cx: &mut App, f: impl FnOnce(&mut MpTableState, usize, &mut App)) {
        let Some(pos) = self.table_state.read(cx).position_by_key(&self.key_input) else {
            return;
        };
        self.table_state.update(cx, |st, cx| f(st, pos, cx));
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
            handles.mutate(cx, |st, pos, cx| st.set_is_file(pos, target_is_file, cx));
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

    let handles = RowHandles::new(ctx.table_state.clone(), key_input.clone());

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
            let handles = handles.clone();
            Checkbox::new(format!("check-mp-{}", idx))
                .checked(enabled)
                .on_click(move |&next_enabled, _, cx| {
                    handles.mutate(cx, |st, pos, cx| st.set_enabled(pos, next_enabled, cx));
                })
        })
        // Key
        .child({
            let handles = handles.clone();
            div().w(px(160.)).flex_shrink_0().child(render_env_input(
                format!("{}-key", mp_prefix),
                &key_input,
                ctx.env_state,
                ctx.modal_state,
                window,
                cx,
                move |new_k: String, _window: &mut Window, cx: &mut App| {
                    handles.mutate(cx, |st, pos, cx| st.update_key(pos, new_k, cx));
                },
            ))
        })
        // Type dropdown: Text | File
        .child({
            let cur_type_label = if is_file { "File" } else { "Text" };
            let handles_text = handles.clone();
            let handles_file = handles.clone();

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
                        handles_text.clone(),
                    ))
                    .item(type_menu_item(
                        "File",
                        true,
                        is_file,
                        handles_file.clone(),
                    ))
                })
        })
        // Value or Path (Single consolidated element fitting flex_1)
        .child({
            div().flex_1().min_w(px(0.)).child(if is_file {
                render_file_picker(&handles, idx, &file_path).into_any_element()
            } else {
                let handles = handles.clone();
                render_env_input(
                    format!("{}-val", mp_prefix),
                    &val_input,
                    ctx.env_state,
                    ctx.modal_state,
                    window,
                    cx,
                    move |new_v: String, _window: &mut Window, cx: &mut App| {
                        handles.mutate(cx, |st, pos, cx| st.update_val(pos, new_v, cx));
                    },
                )
                .into_any_element()
            })
        })
        // Delete row
        .child({
            let handles = handles.clone();
            div()
                .id(gpui_kit::SharedString::from(format!("del-mp-row-{}", idx)))
                .size(px(24.))
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(t.text_muted)
                .hover(move |s| s.text_color(t.danger_text).bg(t.danger_text.opacity(0.12)))
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new("Delete field").build(window, cx)
                })
                .on_click(move |_, _, cx: &mut App| {
                    handles.mutate(cx, |st, pos, cx| st.remove_row(pos, cx));
                })
                .child(Icon::new(IconName::Trash).size(px(14.)))
        })
        .into_any_element()
}
