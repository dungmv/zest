use crate::state::{effective_request_name, ModalState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::component::breadcrumb::{Breadcrumb, BreadcrumbItem};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement as _, Styled,
};

/// Center search button: opens the command palette and shows the active
/// request's folder path as a truncated breadcrumb.
pub(super) fn render_center_section(
    ws: &WorkspaceDataState,
    active_id: &str,
    example_name: Option<&String>,
    modal_state: &Entity<ModalState>,
) -> impl IntoElement {
    let t = theme();
    let modal_center_clone = modal_state.clone();

    let mut segments: Vec<String> = Vec::new();
    if let Some(req) = ws.find_request(active_id) {
        let mut curr_fid = req.folder_id.clone();
        let mut folder_names = Vec::new();
        while let Some(fid) = curr_fid {
            if let Some(f) = ws.folders.iter().find(|f| f.id == fid) {
                folder_names.push(f.name.clone());
                curr_fid = f.parent_id.clone();
            } else {
                break;
            }
        }
        folder_names.reverse();
        segments.extend(folder_names);
        segments.push(effective_request_name(req));
    }
    if let Some(ex_name) = example_name {
        segments.push(ex_name.clone());
    }

    let full_path = segments.join(" / ");
    let has_title = !full_path.trim().is_empty();

    div()
        .id("btn-titlebar-search")
        .occlude()
        .h(px(26.))
        .w(px(340.))
        .flex_shrink_0()
        .px(px(8.))
        .rounded(px(5.))
        .bg(t.bg_card)
        .border_1()
        .border_color(t.border)
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .cursor_pointer()
        .hover(|s| s.bg(t.bg_hover).border_color(t.border_focus))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| {
            let modal_cp = modal_center_clone.clone();
            modal_cp.update(cx, |state, cx| {
                state.toggle_command_palette(cx);
            });
        })
        .when(!full_path.is_empty(), |this| {
            this.tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(full_path.clone()).build(window, cx)
            })
        })
        .child(
            div()
                .flex()
                .items_center()
                .min_w(px(0.))
                .flex_1()
                .overflow_x_hidden()
                .child(if has_title {
                    Breadcrumb::new()
                        .text_xs()
                        .items_center()
                        .children(breadcrumb_items(&segments))
                        .into_any_element()
                } else {
                    div()
                        .truncate()
                        .text_xs()
                        .font_family(".SystemMonoFont")
                        .text_color(t.text_muted)
                        .child("Search requests, commands...")
                        .into_any_element()
                }),
        )
        .child(
            div()
                .flex_shrink_0()
                .px(px(4.))
                .py(px(1.))
                .rounded(px(3.))
                .bg(t.bg_input)
                .border_1()
                .border_color(t.border)
                .text_color(t.text_muted)
                .font_family(".SystemMonoFont")
                .text_size(px(10.))
                .child(if cfg!(target_os = "macos") {
                    "⌘K"
                } else {
                    "Ctrl+K"
                }),
        )
}

/// Truncate by character budget instead of a fixed segment count: keep as
/// many trailing segments as fit, eliding leading ancestors, so short names
/// can still show deep paths while long names never overflow the 340px
/// button.
fn breadcrumb_items(segments: &[String]) -> Vec<BreadcrumbItem> {
    const FOLDER_MAX_CHARS: usize = 20; // ~120px at text_xs
    const NAME_MAX_CHARS: usize = 26; // ~160px at text_xs
    const PATH_CHAR_BUDGET: usize = 44; // incl. " / " separators

    let total = segments.len();
    let mut items = Vec::new();
    if total == 0 {
        return items;
    }

    let mut used = segments[total - 1].chars().count().min(NAME_MAX_CHARS);
    let mut first_shown = total - 1;
    for i in (0..first_shown).rev() {
        let cost = segments[i].chars().count().min(FOLDER_MAX_CHARS) + 3; // " / "
        if used + cost > PATH_CHAR_BUDGET {
            break;
        }
        used += cost;
        first_shown = i;
    }
    if first_shown > 0 {
        items.push(BreadcrumbItem::new("…"));
    }
    for (i, seg) in segments[first_shown..].iter().enumerate() {
        let is_name = first_shown + i == total - 1;
        let max_w = if is_name {
            if total == 1 {
                240.
            } else {
                160.
            }
        } else {
            120.
        };
        items.push(BreadcrumbItem::new(seg.clone()).max_w(px(max_w)).truncate());
    }
    items
}
