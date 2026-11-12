use crate::state::WorkspaceDataState;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::separator::Separator;
use gpui_kit::component::Icon;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};

pub fn render_general_tab(
    ws_state: &Entity<WorkspaceDataState>,
    ws_name_input: &Entity<InputState>,
    workspace_name: &str,
    workspace_path_str: &str,
    cx: &App,
) -> impl IntoElement {
    let t = theme();
    let ws_open = ws_state.clone();
    let ws_save = ws_state.clone();
    let name_inp_save = ws_name_input.clone();
    let cur_inp_val = ws_name_input.read(cx).value().to_string();
    let is_name_changed =
        !cur_inp_val.trim().is_empty() && cur_inp_val.trim() != workspace_name.trim();

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::BOLD)
                .text_color(t.text_secondary)
                .child("WORKSPACE & STORAGE"),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .p(px(14.))
                .rounded(px(6.))
                .bg(t.bg_app)
                .border_1()
                .border_color(t.border)
                .flex()
                .flex_col()
                .gap(px(14.))
                // Workspace Name (Editable + Save)
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .child("Workspace Name"),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(30.))
                                        .px(px(8.))
                                        .rounded(px(5.))
                                        .bg(t.bg_input)
                                        .border_1()
                                        .border_color(t.border)
                                        .flex()
                                        .items_center()
                                        .child(
                                            Input::new(ws_name_input)
                                                .id("ws-settings-name-input")
                                                .appearance(false)
                                                .bordered(false)
                                                .focus_bordered(false),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-ws-settings-save-name")
                                        .flex_shrink_0()
                                        .h(px(30.))
                                        .px(px(12.))
                                        .rounded(px(5.))
                                        .bg(if is_name_changed {
                                            t.primary
                                        } else {
                                            t.bg_input
                                        })
                                        .border_1()
                                        .border_color(if is_name_changed {
                                            t.primary
                                        } else {
                                            t.border
                                        })
                                        .text_color(if is_name_changed {
                                            t.primary_foreground
                                        } else {
                                            t.text_muted
                                        })
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .hover(|s| s.opacity(0.9))
                                        .on_click(move |_, _, cx| {
                                            let val = name_inp_save.read(cx).value().to_string();
                                            let trimmed = val.trim();
                                            if !trimmed.is_empty() {
                                                ws_save.update(cx, |ws, cx| {
                                                    ws.rename_workspace(trimmed.to_string(), cx);
                                                });
                                            }
                                        })
                                        .child("Save"),
                                ),
                        ),
                )
                // Separator
                .child(Separator::horizontal())
                // Storage Directory
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .child("Storage Directory"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .font_family(".SystemMonoFont")
                                        .text_color(t.text_secondary)
                                        .child(workspace_path_str.to_string()),
                                )
                                .child(
                                    div()
                                        .id("btn-ws-settings-open-folder")
                                        .flex_shrink_0()
                                        .px(px(8.))
                                        .py(px(3.))
                                        .rounded(px(4.))
                                        .bg(t.bg_input)
                                        .border_1()
                                        .border_color(t.border)
                                        .text_color(t.text_secondary)
                                        .hover(|s| s.bg(t.bg_hover).text_color(t.text_primary))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .text_xs()
                                        .on_click(move |_, _, cx| {
                                            ws_open.update(cx, |ws, _| {
                                                ws.open_workspace_folder();
                                            });
                                        })
                                        .child(Icon::new(IconName::FolderOpen).size(px(12.)))
                                        .child("Reveal"),
                                ),
                        ),
                ),
        )
}
