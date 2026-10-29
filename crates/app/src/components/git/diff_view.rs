use crate::theme::theme;
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

pub fn split_diff_into_files(diff: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut current_file: Option<String> = None;
    let mut current_chunk: Vec<&str> = Vec::new();

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if let Some(file) = current_file.take() {
                result.push((file, current_chunk.join("\n")));
                current_chunk.clear();
            }
            let file_name = if let Some(b_idx) = line.rfind(" b/") {
                line[b_idx + 3..].trim_matches('"').to_string()
            } else {
                line.trim_start_matches("diff --git ").to_string()
            };
            current_file = Some(file_name);
            current_chunk.push(line);
        } else if current_file.is_some() {
            current_chunk.push(line);
        }
    }

    if let Some(file) = current_file.take() {
        result.push((file, current_chunk.join("\n")));
    }

    result
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
            div()
                .id("btn-diff-split")
                .px(px(6.))
                .py(px(2.))
                .rounded(px(3.))
                .cursor_pointer()
                .text_xs()
                .when(view_mode == DiffViewMode::Split, |s| {
                    s.bg(t.bg_card)
                        .text_color(t.text_primary)
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                })
                .when(view_mode != DiffViewMode::Split, |s| {
                    s.text_color(t.text_muted)
                        .hover(|h| h.text_color(t.text_secondary))
                })
                .on_click(move |_, _, cx| {
                    st.update(cx, |m, cx| {
                        *m = DiffViewMode::Split;
                        cx.notify();
                    });
                })
                .child("◫ Split")
        })
        .child({
            let st = view_mode_entity.clone();
            div()
                .id("btn-diff-unified")
                .px(px(6.))
                .py(px(2.))
                .rounded(px(3.))
                .cursor_pointer()
                .text_xs()
                .when(view_mode == DiffViewMode::Unified, |s| {
                    s.bg(t.bg_card)
                        .text_color(t.text_primary)
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                })
                .when(view_mode != DiffViewMode::Unified, |s| {
                    s.text_color(t.text_muted)
                        .hover(|h| h.text_color(t.text_secondary))
                })
                .on_click(move |_, _, cx| {
                    st.update(cx, |m, cx| {
                        *m = DiffViewMode::Unified;
                        cx.notify();
                    });
                })
                .child("▤ Unified")
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
                                    .child(format!("📄 {}", file)),
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
                                        .child(format!("📄 {}", file_name)),
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
        Some(_) => div()
            .p(px(12.))
            .text_xs()
            .text_color(t.text_muted)
            .child("File has no modifications compared to HEAD.")
            .into_any_element(),
        None => div()
            .p(px(12.))
            .text_xs()
            .text_color(t.text_muted)
            .child("Select a modified or staged file to view its git diff.")
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_filters_new_file_mode_and_initial_hunk() {
        let raw_diff = r#"diff --git a/collections/speedtest/new/new-request.yaml b/collections/speedtest/new/new-request.yaml
new file mode 100644
index 0000000..c221a93
--- /dev/null
+++ b/collections/speedtest/new/new-request.yaml
@@ -0,0 +1,3 @@
+name: New Request
+method: GET
+url: https://example.com"#;

        let rows = parse_side_by_side_diff(raw_diff);

        // Should NOT have new file mode as code row
        for r in &rows {
            if let SideBySideRow::Diff { left, right } = r {
                if let Some(l) = left {
                    assert!(!l.content.contains("new file mode"));
                    assert!(!l.content.contains("@@"));
                }
                if let Some(r) = right {
                    assert!(!r.content.contains("new file mode"));
                    assert!(!r.content.contains("@@"));
                }
            }
        }

        // Must have 3 added lines
        assert_eq!(rows.len(), 3);
        if let SideBySideRow::Diff {
            left: None,
            right: Some(r),
        } = &rows[0]
        {
            assert_eq!(r.content, "name: New Request");
            assert_eq!(r.kind, DiffLineKind::Added);
            assert_eq!(r.line_no, Some(1));
        } else {
            panic!("Expected added diff row at index 0");
        }
    }

    #[test]
    fn test_parse_diff_suppresses_hunk_headers_and_numbers_accurately() {
        let raw_diff = r#"diff --git a/file.txt b/file.txt
index 1111111..2222222 100644
--- a/file.txt
+++ b/file.txt
@@ -10,3 +10,3 @@
 normal line
-old line
+new line"#;

        let rows = parse_side_by_side_diff(raw_diff);
        // Hunk header @@ -10,3 is suppressed: only 2 code rows (1 normal line + 1 modified row)
        assert_eq!(rows.len(), 2);
        if let SideBySideRow::Diff {
            left: Some(l),
            right: Some(r),
        } = &rows[0]
        {
            assert_eq!(l.content, "normal line");
            assert_eq!(l.line_no, Some(10));
            assert_eq!(r.line_no, Some(10));
        } else {
            panic!("Expected normal diff row at index 0");
        }
    }

    #[test]
    fn test_parse_multi_file_diff_inserts_file_headers() {
        let raw_diff = r#"diff --git a/file1.txt b/file1.txt
index 1111111..2222222 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,1 +1,1 @@
-a
+b
diff --git a/file2.txt b/file2.txt
index 3333333..4444444 100644
--- a/file2.txt
+++ b/file2.txt
@@ -1,1 +1,1 @@
-c
+d"#;

        let rows = parse_side_by_side_diff(raw_diff);
        let file_headers: Vec<_> = rows
            .iter()
            .filter_map(|r| match r {
                SideBySideRow::FileHeader(f) => Some(f.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(file_headers, vec!["file1.txt", "file2.txt"]);
    }

    #[test]
    fn test_split_diff_into_files() {
        let diff = r#"diff --git a/file1.txt b/file1.txt
index 1111111..2222222 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,1 +1,1 @@
-a
+b
diff --git a/dir/file2.rs b/dir/file2.rs
index 3333333..4444444 100644
--- a/dir/file2.rs
+++ b/dir/file2.rs
@@ -1,1 +1,1 @@
-c
+d"#;

        let files = split_diff_into_files(diff);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, "file1.txt");
        assert!(files[0].1.contains("-a"));
        assert_eq!(files[1].0, "dir/file2.rs");
        assert!(files[1].1.contains("+d"));
    }
}
