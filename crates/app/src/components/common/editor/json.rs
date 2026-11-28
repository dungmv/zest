use super::{clamp_char_boundary_left, clamp_char_boundary_right};
use crate::editor_scheme::editor_scheme;
use gpui_kit::component::input::{
    EditorState as CodeEditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    Rope,
};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use std::ops::Range;

pub struct JsonHighlighter {
    pub text: Rope,
    pub cached_str: String,
    comments: Vec<Range<usize>>,
}

impl JsonHighlighter {
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            cached_str: String::new(),
            comments: Vec::new(),
        }
    }
}

impl Default for JsonHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl InputHighlighter for JsonHighlighter {
    fn language(&self) -> SharedString {
        "json".into()
    }

    fn update(
        &mut self,
        _edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut Window,
        _cx: &mut Context<CodeEditorState>,
    ) {
        self.text = text.clone();
        self.cached_str = text.to_string();
        self.comments.clear();
        self.comments
            .extend(zest_core::jsonc::comment_ranges(&self.cached_str));
    }

    fn styles(
        &self,
        range: &Range<usize>,
        _resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let temp_str;
        let full_str: &str = if !self.cached_str.is_empty() {
            &self.cached_str
        } else if self.text.len() != 0 {
            temp_str = self.text.to_string();
            &temp_str
        } else {
            ""
        };
        if full_str.is_empty() || range.is_empty() {
            return vec![(range.clone(), HighlightStyle::default())];
        }

        let start = clamp_char_boundary_left(full_str, range.start);
        let end = clamp_char_boundary_right(full_str, range.end.min(full_str.len()));

        if start >= end {
            return vec![(range.clone(), HighlightStyle::default())];
        }

        let slice_str = &full_str[start..end];
        let slice_bytes = slice_str.as_bytes();
        let len = slice_bytes.len();
        let mut i = 0;
        let mut raw_tokens = Vec::new();
        let scheme = editor_scheme();

        let first_comment = self
            .comments
            .partition_point(|comment| comment.end <= start);
        let mut comments = self.comments[first_comment..].iter().peekable();
        while i < len {
            if let Some(comment) = comments.next_if(|comment| comment.start <= start + i) {
                let comment_end = comment.end.min(end);
                if comment_end > start + i {
                    raw_tokens.push((
                        start + i..comment_end,
                        HighlightStyle {
                            color: Some(scheme.comment),
                            ..Default::default()
                        },
                    ));
                    i = comment_end - start;
                    continue;
                }
            }
            let b = slice_bytes[i];
            if b == b'"' {
                let tok_start = i;
                i += 1;
                let mut escaped = false;
                while i < len {
                    let ch = slice_bytes[i];
                    if escaped {
                        escaped = false;
                        i += 1;
                    } else if ch == b'\\' {
                        escaped = true;
                        i += 1;
                    } else if ch == b'"' {
                        i += 1;
                        break;
                    } else if ch == b'\n' {
                        break;
                    } else {
                        i += 1;
                    }
                }
                let tok_end = i;
                let mut j = tok_end;
                while j < len
                    && (slice_bytes[j] == b' '
                        || slice_bytes[j] == b'\t'
                        || slice_bytes[j] == b'\r'
                        || slice_bytes[j] == b'\n')
                {
                    j += 1;
                }
                let is_key = j < len && slice_bytes[j] == b':';
                let color = if is_key { scheme.key } else { scheme.string };
                raw_tokens.push((
                    start + tok_start..start + tok_end,
                    HighlightStyle {
                        color: Some(color),
                        ..Default::default()
                    },
                ));
            } else if matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',') {
                raw_tokens.push((
                    start + i..start + i + 1,
                    HighlightStyle {
                        color: Some(scheme.punctuation),
                        ..Default::default()
                    },
                ));
                i += 1;
            } else if b == b'-' || b.is_ascii_digit() {
                let num_start = i;
                i += 1;
                while i < len
                    && (slice_bytes[i].is_ascii_digit()
                        || matches!(slice_bytes[i], b'.' | b'e' | b'E' | b'+' | b'-'))
                {
                    i += 1;
                }
                raw_tokens.push((
                    start + num_start..start + i,
                    HighlightStyle {
                        color: Some(scheme.number),
                        ..Default::default()
                    },
                ));
            } else if slice_str[i..].starts_with("true")
                && (i + 4 == len
                    || !slice_bytes[i + 4].is_ascii_alphanumeric() && slice_bytes[i + 4] != b'_')
            {
                raw_tokens.push((
                    start + i..start + i + 4,
                    HighlightStyle {
                        color: Some(scheme.keyword),
                        font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                        ..Default::default()
                    },
                ));
                i += 4;
            } else if slice_str[i..].starts_with("false")
                && (i + 5 == len
                    || !slice_bytes[i + 5].is_ascii_alphanumeric() && slice_bytes[i + 5] != b'_')
            {
                raw_tokens.push((
                    start + i..start + i + 5,
                    HighlightStyle {
                        color: Some(scheme.keyword),
                        font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                        ..Default::default()
                    },
                ));
                i += 5;
            } else if slice_str[i..].starts_with("null")
                && (i + 4 == len
                    || !slice_bytes[i + 4].is_ascii_alphanumeric() && slice_bytes[i + 4] != b'_')
            {
                raw_tokens.push((
                    start + i..start + i + 4,
                    HighlightStyle {
                        color: Some(scheme.null),
                        font_style: Some(gpui_kit::FontStyle::Italic),
                        ..Default::default()
                    },
                ));
                i += 4;
            } else {
                let ch = slice_str[i..].chars().next().unwrap();
                i += ch.len_utf8();
            }
        }

        let mut full_styles = Vec::new();
        let mut cursor = range.start;

        for (token_range, style) in raw_tokens {
            if token_range.start > cursor {
                full_styles.push((cursor..token_range.start, HighlightStyle::default()));
            }
            cursor = token_range.end;
            full_styles.push((token_range, style));
        }

        if cursor < range.end {
            full_styles.push((cursor..range.end, HighlightStyle::default()));
        }

        full_styles
    }

    fn fold_ranges(&self, text: &Rope) -> Vec<FoldRange> {
        let mut folds = Vec::new();
        let mut stack = Vec::new();
        let s = text.to_string();
        let mut current_line = 0;
        let mut in_string = false;
        let mut escaped = false;
        let bytes = s.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        let mut comments = zest_core::jsonc::comment_ranges(&s).peekable();
        while i < len {
            if let Some(comment) = comments.next_if(|range| range.start == i) {
                current_line += bytes[comment.clone()]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count();
                i = comment.end;
                continue;
            }
            let b = bytes[i];
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
            } else {
                match b {
                    b'"' => in_string = true,
                    b'{' | b'[' => stack.push(current_line),
                    b'}' | b']' => {
                        if let Some(start_line) = stack.pop() {
                            if current_line > start_line {
                                folds.push(FoldRange::new(start_line, current_line));
                            }
                        }
                    }
                    b'\n' => current_line += 1,
                    _ => {}
                }
            }
            if in_string && b == b'\n' {
                current_line += 1;
            }
            i += 1;
        }
        folds.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| b.end_line.cmp(&a.end_line))
        });
        folds.dedup_by_key(|f| f.start_line);
        folds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyResolver;
    impl HighlightStyleResolver for DummyResolver {
        fn style(&self, _: &str) -> Option<HighlightStyle> {
            None
        }
    }

    #[test]
    fn test_json_highlighter_styles_coverage() {
        let mut highlighter = JsonHighlighter::new();
        let json_text = r#"{"name": "Alice", "age": 30, "admin": true, "extra": null}"#;
        highlighter.text = Rope::from_str(json_text);

        let range = 0..json_text.len();
        let styles = highlighter.styles(&range, &DummyResolver);

        // Verify contiguous coverage
        let mut cursor = 0;
        for (sub_range, _) in &styles {
            assert_eq!(sub_range.start, cursor);
            assert!(sub_range.end > sub_range.start);
            cursor = sub_range.end;
        }
        assert_eq!(cursor, json_text.len());

        // Check that keys and values received colors
        let scheme = editor_scheme();
        let key_highlight = styles
            .iter()
            .find(|(r, s)| &json_text[r.clone()] == "\"name\"" && s.color == Some(scheme.key));
        assert!(key_highlight.is_some(), "Key should have json_key color");

        let str_highlight = styles
            .iter()
            .find(|(r, s)| &json_text[r.clone()] == "\"Alice\"" && s.color == Some(scheme.string));
        assert!(
            str_highlight.is_some(),
            "String should have json_string color"
        );

        let num_highlight = styles
            .iter()
            .find(|(r, s)| &json_text[r.clone()] == "30" && s.color == Some(scheme.number));
        assert!(
            num_highlight.is_some(),
            "Number should have json_number color"
        );

        let bool_highlight = styles
            .iter()
            .find(|(r, s)| &json_text[r.clone()] == "true" && s.color == Some(scheme.keyword));
        assert!(
            bool_highlight.is_some(),
            "Boolean should have the keyword color"
        );

        let null_highlight = styles
            .iter()
            .find(|(r, s)| &json_text[r.clone()] == "null" && s.color == Some(scheme.null));
        assert!(null_highlight.is_some(), "Null should have json_null color");
    }

    #[test]
    fn test_json_highlighter_folding() {
        let highlighter = JsonHighlighter::new();
        let json_text = "{\n  \"items\": [\n    1,\n    2\n  ]\n}";
        let rope = Rope::from_str(json_text);
        let folds = highlighter.fold_ranges(&rope);
        assert!(!folds.is_empty(), "Should detect foldable ranges");
    }

    #[test]
    fn json_highlighter_ignores_brackets_inside_comments_when_folding() {
        let source = "{\n  /* }\n     [ */\n  \"url\": \"https://example.com\"\n}";
        let highlighter = JsonHighlighter::new();
        let folds = highlighter.fold_ranges(&Rope::from_str(source));
        assert_eq!(folds.len(), 1);
        assert_eq!((folds[0].start_line, folds[0].end_line), (0, 4));
    }

    #[test]
    fn test_json_highlighter_large_file_performance() {
        let mut highlighter = JsonHighlighter::new();
        let items: Vec<String> = (0..500)
            .map(|i| {
                format!(
                    r#"{{"id": {}, "title": "large test item {}", "active": true}}"#,
                    i, i
                )
            })
            .collect();
        let json_text = format!("[\n{}\n]", items.join(",\n"));
        highlighter.text = Rope::from_str(&json_text);
        highlighter.cached_str = json_text.clone();

        let first_line_range = 0..1;
        let styles = highlighter.styles(&first_line_range, &DummyResolver);
        assert!(!styles.is_empty());
    }
}
