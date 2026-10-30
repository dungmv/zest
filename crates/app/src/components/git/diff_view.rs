use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{
    Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiffViewMode {
    Split,
    Unified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    Normal,
    Added,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct DiffSideLine {
    pub line_no: Option<usize>,
    pub content: String,
    pub kind: DiffLineKind,
}

#[derive(Debug, Clone)]
pub enum SideBySideRow {
    FileHeader(String),
    Diff {
        left: Option<DiffSideLine>,
        right: Option<DiffSideLine>,
    },
}

/// Split unified git patch text into individual file diff slices with zero allocations.
pub fn split_diff_into_file_slices(diff: &str) -> Vec<(&str, &str)> {
    let mut result = Vec::new();
    let header = "diff --git ";
    let mut header_indices: Vec<usize> = Vec::new();

    let mut start = 0;
    while let Some(idx) = diff[start..].find(header) {
        let abs_idx = start + idx;
        if abs_idx == 0 || diff.as_bytes()[abs_idx - 1] == b'\n' {
            header_indices.push(abs_idx);
        }
        start = abs_idx + header.len();
    }

    if header_indices.is_empty() {
        return result;
    }

    for i in 0..header_indices.len() {
        let chunk_start = header_indices[i];
        let chunk_end = if i + 1 < header_indices.len() {
            header_indices[i + 1]
        } else {
            diff.len()
        };
        let chunk = diff[chunk_start..chunk_end].trim_end();

        let first_line = chunk.lines().next().unwrap_or("");
        let file_name = if let Some(b_idx) = first_line.rfind(" b/") {
            first_line[b_idx + 3..].trim_matches('"')
        } else {
            first_line.trim_start_matches("diff --git ")
        };
        result.push((file_name, chunk));
    }

    result
}

#[cfg(test)]
pub fn split_diff_into_files(diff: &str) -> Vec<(String, String)> {
    split_diff_into_file_slices(diff)
        .into_iter()
        .map(|(f, c)| (f.to_string(), c.to_string()))
        .collect()
}

pub fn parse_side_by_side_diff(diff: &str) -> Vec<SideBySideRow> {
    let mut rows = Vec::new();
    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut pending_deleted: Vec<String> = Vec::new();
    let mut pending_added: Vec<String> = Vec::new();

    let multi_file = diff.matches("diff --git").count() > 1;

    let flush_pending = |deleted: &mut Vec<String>,
                         added: &mut Vec<String>,
                         old_cur: &mut usize,
                         new_cur: &mut usize,
                         rows: &mut Vec<SideBySideRow>| {
        let max_len = deleted.len().max(added.len());
        for i in 0..max_len {
            let left = if i < deleted.len() {
                let ln = *old_cur;
                *old_cur += 1;
                Some(DiffSideLine {
                    line_no: Some(ln),
                    content: deleted[i].clone(),
                    kind: DiffLineKind::Deleted,
                })
            } else {
                None
            };

            let right = if i < added.len() {
                let ln = *new_cur;
                *new_cur += 1;
                Some(DiffSideLine {
                    line_no: Some(ln),
                    content: added[i].clone(),
                    kind: DiffLineKind::Added,
                })
            } else {
                None
            };

            rows.push(SideBySideRow::Diff { left, right });
        }
        deleted.clear();
        added.clear();
    };

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            flush_pending(
                &mut pending_deleted,
                &mut pending_added,
                &mut old_line,
                &mut new_line,
                &mut rows,
            );
            old_line = 1;
            new_line = 1;
            if multi_file {
                let file_name = line.split(" b/").nth(1).unwrap_or(line);
                rows.push(SideBySideRow::FileHeader(file_name.to_string()));
            }
            continue;
        }

        if line.starts_with("index ")
            || line.starts_with("---")
            || line.starts_with("+++")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
            || line.starts_with("old mode")
            || line.starts_with("new mode")
            || line.starts_with("similarity index")
            || line.starts_with("dissimilarity index")
            || line.starts_with("copy from")
            || line.starts_with("copy to")
            || line.starts_with("rename from")
            || line.starts_with("rename to")
            || line.starts_with("Binary files")
            || line.starts_with(r"\ No newline")
        {
            continue;
        }

        if line.starts_with("@@") {
            flush_pending(
                &mut pending_deleted,
                &mut pending_added,
                &mut old_line,
                &mut new_line,
                &mut rows,
            );
            if let Some(hunk_parts) = line.split("@@").nth(1) {
                let parts: Vec<&str> = hunk_parts.split_whitespace().collect();
                for p in parts {
                    if let Some(stripped) = p.strip_prefix('-') {
                        let num = stripped
                            .split(',')
                            .next()
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(1);
                        old_line = num;
                    } else if let Some(stripped) = p.strip_prefix('+') {
                        let num = stripped
                            .split(',')
                            .next()
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(1);
                        new_line = num;
                    }
                }
            }
        } else if let Some(stripped) = line.strip_prefix('-') {
            pending_deleted.push(stripped.to_string());
        } else if let Some(stripped) = line.strip_prefix('+') {
            pending_added.push(stripped.to_string());
        } else {
            flush_pending(
                &mut pending_deleted,
                &mut pending_added,
                &mut old_line,
                &mut new_line,
                &mut rows,
            );
            let content = if let Some(stripped) = line.strip_prefix(' ') {
                stripped
            } else {
                line
            };
            let ln_old = old_line;
            let ln_new = new_line;
            old_line += 1;
            new_line += 1;
            rows.push(SideBySideRow::Diff {
                left: Some(DiffSideLine {
                    line_no: Some(ln_old),
                    content: content.to_string(),
                    kind: DiffLineKind::Normal,
                }),
                right: Some(DiffSideLine {
                    line_no: Some(ln_new),
                    content: content.to_string(),
                    kind: DiffLineKind::Normal,
                }),
            });
        }
    }

    flush_pending(
        &mut pending_deleted,
        &mut pending_added,
        &mut old_line,
        &mut new_line,
        &mut rows,
    );
    rows
}

pub fn render_diff_view_toggle(
    view_mode: DiffViewMode,
    view_mode_entity: Entity<DiffViewMode>,
) -> gpui_kit::AnyElement {
    let t = theme();
    let is_split = view_mode == DiffViewMode::Split;
    let is_unified = view_mode == DiffViewMode::Unified;

    div()
        .flex()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(4.))
        .bg(t.bg_sidebar)
        .border_1()
        .border_color(t.border)
        .child({
            let st = view_mode_entity.clone();
            Button::new("btn-diff-split")
                .ghost()
                .compact()
                .px(px(6.))
                .py(px(2.))
                .h(px(22.))
                .rounded(px(3.))
                .bg(if is_split {
                    t.bg_card
                } else {
                    gpui_kit::transparent_black()
                })
                .border_1()
                .border_color(if is_split {
                    t.border
                } else {
                    gpui_kit::transparent_black()
                })
                .tooltip("Split (Side-by-Side) Diff")
                .on_click(move |_, _, cx| {
                    st.update(cx, |m, cx| {
                        *m = DiffViewMode::Split;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_xs()
                        .font_weight(if is_split {
                            gpui_kit::FontWeight::SEMIBOLD
                        } else {
                            gpui_kit::FontWeight::NORMAL
                        })
                        .text_color(if is_split {
                            t.text_primary
                        } else {
                            t.text_muted
                        })
                        .child("Split"),
                )
        })
        .child({
            let st = view_mode_entity.clone();
            Button::new("btn-diff-unified")
                .ghost()
                .compact()
                .px(px(6.))
                .py(px(2.))
                .h(px(22.))
                .rounded(px(3.))
                .bg(if is_unified {
                    t.bg_card
                } else {
                    gpui_kit::transparent_black()
                })
                .border_1()
                .border_color(if is_unified {
                    t.border
                } else {
                    gpui_kit::transparent_black()
                })
                .tooltip("Unified Diff")
                .on_click(move |_, _, cx| {
                    st.update(cx, |m, cx| {
                        *m = DiffViewMode::Unified;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_xs()
                        .font_weight(if is_unified {
                            gpui_kit::FontWeight::SEMIBOLD
                        } else {
                            gpui_kit::FontWeight::NORMAL
                        })
                        .text_color(if is_unified {
                            t.text_primary
                        } else {
                            t.text_muted
                        })
                        .child("Unified"),
                )
        })
        .into_any_element()
}

pub fn render_diff_content(
    diff_content: Option<String>,
    view_mode: DiffViewMode,
) -> gpui_kit::AnyElement {
    let t = theme();

    match diff_content {
        Some(diff) if !diff.trim().is_empty() => {
            let green_bg = gpui_kit::Hsla {
                h: 142.0 / 360.0,
                s: 0.7,
                l: 0.45,
                a: 0.15,
            };
            let green_text: gpui_kit::Hsla = gpui_kit::rgb(0x4ade80).into();
            let red_bg = gpui_kit::Hsla {
                h: 0.0,
                s: 0.7,
                l: 0.45,
                a: 0.15,
            };
            let red_text: gpui_kit::Hsla = gpui_kit::rgb(0xef4444).into();
            let gray_text = t.text_muted;

            match view_mode {
                DiffViewMode::Split => {
                    let rows = parse_side_by_side_diff(&diff);
                    if rows.is_empty() {
                        return div()
                            .p(px(12.))
                            .text_xs()
                            .text_color(t.text_muted)
                            .child("(Empty file)")
                            .into_any_element();
                    }
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .font_family(".SystemMonoFont")
                        .text_xs()
                        .children(rows.into_iter().map(|row| {
                            match row {
                                SideBySideRow::FileHeader(file) => div()
                                    .w_full()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .bg(t.bg_sidebar)
                                    .border_y_1()
                                    .border_color(t.border)
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(t.text_primary)
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        Icon::new(IconName::FileText)
                                            .size(px(13.))
                                            .text_color(t.text_secondary),
                                    )
                                    .child(file),
                                SideBySideRow::Diff { left, right } => {
                                    div()
                                        .w_full()
                                        .flex()
                                        .border_b_1()
                                        .border_color(gpui_kit::Hsla {
                                            a: 0.25,
                                            ..t.border
                                        })
                                        // Left column (Old)
                                        .child({
                                            let (bg, txt_color, line_num, txt) = match left {
                                                Some(l) => {
                                                    let (bg, col) = match l.kind {
                                                        DiffLineKind::Deleted => {
                                                            (Some(red_bg), red_text)
                                                        }
                                                        DiffLineKind::Added => {
                                                            (Some(green_bg), green_text)
                                                        }
                                                        DiffLineKind::Normal => (None, gray_text),
                                                    };
                                                    (
                                                        bg,
                                                        col,
                                                        l.line_no
                                                            .map(|n| n.to_string())
                                                            .unwrap_or_default(),
                                                        l.content,
                                                    )
                                                }
                                                None => (
                                                    Some(gpui_kit::Hsla {
                                                        a: 0.15,
                                                        ..t.bg_card
                                                    }),
                                                    gray_text,
                                                    String::new(),
                                                    String::new(),
                                                ),
                                            };

                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .border_r_1()
                                                .border_color(t.border)
                                                .when_some(bg, |this, b| this.bg(b))
                                                .flex()
                                                .items_start()
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .w(px(34.))
                                                        .flex_shrink_0()
                                                        .px(px(4.))
                                                        .py(px(1.))
                                                        .text_right()
                                                        .text_color(t.text_muted)
                                                        .child(line_num),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .px(px(6.))
                                                        .py(px(1.))
                                                        .text_color(txt_color)
                                                        .overflow_hidden()
                                                        .child(txt),
                                                )
                                        })
                                        // Right column (New)
                                        .child({
                                            let (bg, txt_color, line_num, txt) = match right {
                                                Some(r) => {
                                                    let (bg, col) = match r.kind {
                                                        DiffLineKind::Added => {
                                                            (Some(green_bg), green_text)
                                                        }
                                                        DiffLineKind::Deleted => {
                                                            (Some(red_bg), red_text)
                                                        }
                                                        DiffLineKind::Normal => {
                                                            (None, t.text_primary)
                                                        }
                                                    };
                                                    (
                                                        bg,
                                                        col,
                                                        r.line_no
                                                            .map(|n| n.to_string())
                                                            .unwrap_or_default(),
                                                        r.content,
                                                    )
                                                }
                                                None => (
                                                    Some(gpui_kit::Hsla {
                                                        a: 0.15,
                                                        ..t.bg_card
                                                    }),
                                                    gray_text,
                                                    String::new(),
                                                    String::new(),
                                                ),
                                            };

                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .when_some(bg, |this, b| this.bg(b))
                                                .flex()
                                                .items_start()
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .w(px(34.))
                                                        .flex_shrink_0()
                                                        .px(px(4.))
                                                        .py(px(1.))
                                                        .text_right()
                                                        .text_color(t.text_muted)
                                                        .child(line_num),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .px(px(6.))
                                                        .py(px(1.))
                                                        .text_color(txt_color)
                                                        .overflow_hidden()
                                                        .child(txt),
                                                )
                                        })
                                }
                            }
                        }))
                        .into_any_element()
                }
                DiffViewMode::Unified => {
                    let multi_file = diff.matches("diff --git").count() > 1;
                    let elements: Vec<gpui_kit::Div> = diff
                        .lines()
                        .filter_map(|line| {
                            if line.starts_with("index ")
                                || line.starts_with("---")
                                || line.starts_with("+++")
                                || line.starts_with("new file mode")
                                || line.starts_with("deleted file mode")
                                || line.starts_with("old mode")
                                || line.starts_with("new mode")
                                || line.starts_with("similarity index")
                                || line.starts_with("dissimilarity index")
                                || line.starts_with("copy from")
                                || line.starts_with("copy to")
                                || line.starts_with("rename from")
                                || line.starts_with("rename to")
                                || line.starts_with("Binary files")
                                || line.starts_with(r"\ No newline")
                                || line.starts_with("@@")
                            {
                                return None;
                            }

                            if line.starts_with("diff --git") {
                                if !multi_file {
                                    return None;
                                }
                                let file_name = line.split(" b/").nth(1).unwrap_or(line);
                                return Some(
                                    div()
                                        .w_full()
                                        .px(px(8.))
                                        .py(px(4.))
                                        .bg(t.bg_sidebar)
                                        .border_y_1()
                                        .border_color(t.border)
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_primary)
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            Icon::new(IconName::FileText)
                                                .size(px(13.))
                                                .text_color(t.text_secondary),
                                        )
                                        .child(file_name.to_string()),
                                );
                            }

                            let (bg, text_color) = if line.starts_with('+') {
                                (Some(green_bg), green_text)
                            } else if line.starts_with('-') {
                                (Some(red_bg), red_text)
                            } else if line.starts_with("@@") {
                                (None, t.primary)
                            } else {
                                (None, gray_text)
                            };

                            Some(
                                div()
                                    .px(px(6.))
                                    .py(px(1.))
                                    .when_some(bg, |this, b| this.bg(b))
                                    .text_color(text_color)
                                    .child(line.to_string()),
                            )
                        })
                        .collect();

                    if elements.is_empty() {
                        return div()
                            .p(px(12.))
                            .text_xs()
                            .text_color(t.text_muted)
                            .child("(Empty file)")
                            .into_any_element();
                    }

                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .font_family(".SystemMonoFont")
                        .text_xs()
                        .children(elements)
                        .into_any_element()
                }
            }
        }
        Some(_) => Empty::new()
            .header(
                EmptyHeader::new()
                    .media(
                        EmptyMedia::new()
                            .with_variant(EmptyMediaVariant::Icon)
                            .child(Icon::new(IconName::FileCheck).size(px(24.))),
                    )
                    .title(EmptyTitle::new().child("No Modifications"))
                    .description(
                        EmptyDescription::new()
                            .child("File has no modifications compared to HEAD."),
                    ),
            )
            .into_any_element(),
        None => Empty::new()
            .header(
                EmptyHeader::new()
                    .media(
                        EmptyMedia::new()
                            .with_variant(EmptyMediaVariant::Icon)
                            .child(Icon::new(IconName::GitCompare).size(px(24.))),
                    )
                    .title(EmptyTitle::new().child("No File Selected"))
                    .description(
                        EmptyDescription::new()
                            .child("Select a modified or staged file to view its git diff."),
                    ),
            )
            .into_any_element(),
    }
}

#[cfg(test)]
#[path = "diff_view_tests.rs"]
mod diff_view_tests;
