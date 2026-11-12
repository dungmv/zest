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

#[cfg(test)]
#[path = "diff_view_tests.rs"]
mod diff_view_tests;
