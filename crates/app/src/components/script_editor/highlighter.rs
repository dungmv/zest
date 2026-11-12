use crate::theme::theme;
use gpui_kit::component::input::{
    EditorState as CodeEditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    Rope,
};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use std::ops::Range;

fn clamp_char_boundary_left(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn clamp_char_boundary_right(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

pub struct JavaScriptHighlighter {
    pub text: Rope,
    pub cached_str: String,
}

impl JavaScriptHighlighter {
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            cached_str: String::new(),
        }
    }
}

impl Default for JavaScriptHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

const JS_KEYWORDS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "of",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

const JS_LITERALS: &[&str] = &["true", "false", "null", "undefined", "NaN", "Infinity"];

const JS_GLOBALS: &[&str] = &[
    "z",
    "console",
    "JSON",
    "Math",
    "Date",
    "String",
    "Number",
    "Boolean",
    "Array",
    "Object",
    "RegExp",
    "Promise",
    "Error",
    "Map",
    "Set",
    "parseInt",
    "parseFloat",
    "encodeURIComponent",
    "decodeURIComponent",
];

impl InputHighlighter for JavaScriptHighlighter {
    fn language(&self) -> SharedString {
        "javascript".into()
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
        let bytes = slice_str.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        let mut raw_tokens = Vec::new();
        let t = theme();

        while i < len {
            let b = bytes[i];

            // 1. Single-line comment // ...
            if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
                let comment_start = i;
                i += 2;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                raw_tokens.push((
                    start + comment_start..start + i,
                    HighlightStyle {
                        color: Some(t.text_muted),
                        font_style: Some(gpui_kit::FontStyle::Italic),
                        ..Default::default()
                    },
                ));
            }
            // 2. Multi-line comment /* ... */
            else if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                let comment_start = i;
                i += 2;
                while i < len {
                    if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                raw_tokens.push((
                    start + comment_start..start + i,
                    HighlightStyle {
                        color: Some(t.text_muted),
                        font_style: Some(gpui_kit::FontStyle::Italic),
                        ..Default::default()
                    },
                ));
            }
            // 3. String literals: double quotes, single quotes, backticks
            else if b == b'"' || b == b'\'' || b == b'`' {
                let quote = b;
                let str_start = i;
                i += 1;
                let mut escaped = false;
                while i < len {
                    let ch = bytes[i];
                    if escaped {
                        escaped = false;
                        i += 1;
                    } else if ch == b'\\' {
                        escaped = true;
                        i += 1;
                    } else if ch == quote {
                        i += 1;
                        break;
                    } else if (quote == b'"' || quote == b'\'') && ch == b'\n' {
                        // Unclosed single-line string breaks at newline
                        break;
                    } else {
                        i += 1;
                    }
                }
                raw_tokens.push((
                    start + str_start..start + i,
                    HighlightStyle {
                        color: Some(t.json_string),
                        ..Default::default()
                    },
                ));
            }
            // 4. Numbers: hex (0x...), binary (0b...), decimal
            else if (b.is_ascii_digit())
                || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit())
            {
                let num_start = i;
                if b == b'0' && i + 1 < len && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
                    i += 2;
                    while i < len && bytes[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                } else if b == b'0' && i + 1 < len && (bytes[i + 1] == b'b' || bytes[i + 1] == b'B')
                {
                    i += 2;
                    while i < len && (bytes[i] == b'0' || bytes[i] == b'1') {
                        i += 1;
                    }
                } else {
                    i += 1;
                    while i < len
                        && (bytes[i].is_ascii_digit()
                            || matches!(bytes[i], b'.' | b'e' | b'E' | b'+' | b'-' | b'_'))
                    {
                        i += 1;
                    }
                }
                raw_tokens.push((
                    start + num_start..start + i,
                    HighlightStyle {
                        color: Some(t.json_number),
                        ..Default::default()
                    },
                ));
            }
            // 5. Identifiers, keywords, literals, globals
            else if b.is_ascii_alphabetic() || b == b'_' || b == b'$' {
                let id_start = i;
                i += 1;
                while i < len
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
                {
                    i += 1;
                }
                let id_str = &slice_str[id_start..i];

                if JS_KEYWORDS.contains(&id_str) {
                    raw_tokens.push((
                        start + id_start..start + i,
                        HighlightStyle {
                            color: Some(t.json_boolean),
                            font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                            ..Default::default()
                        },
                    ));
                } else if JS_LITERALS.contains(&id_str) {
                    raw_tokens.push((
                        start + id_start..start + i,
                        HighlightStyle {
                            color: Some(t.json_null),
                            font_style: Some(gpui_kit::FontStyle::Italic),
                            ..Default::default()
                        },
                    ));
                } else if JS_GLOBALS.contains(&id_str) {
                    raw_tokens.push((
                        start + id_start..start + i,
                        HighlightStyle {
                            color: Some(t.json_key),
                            font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                            ..Default::default()
                        },
                    ));
                } else {
                    // Check if followed by '(' -> function call
                    let mut j = i;
                    while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                        j += 1;
                    }
                    if j < len && bytes[j] == b'(' {
                        raw_tokens.push((
                            start + id_start..start + i,
                            HighlightStyle {
                                color: Some(t.primary),
                                ..Default::default()
                            },
                        ));
                    }
                }
            }
            // 6. Operators and punctuation
            else if matches!(
                b,
                b'{' | b'}'
                    | b'('
                    | b')'
                    | b'['
                    | b']'
                    | b';'
                    | b','
                    | b'.'
                    | b':'
                    | b'='
                    | b'+'
                    | b'-'
                    | b'*'
                    | b'/'
                    | b'%'
                    | b'!'
                    | b'<'
                    | b'>'
                    | b'&'
                    | b'|'
                    | b'^'
                    | b'~'
                    | b'?'
            ) {
                raw_tokens.push((
                    start + i..start + i + 1,
                    HighlightStyle {
                        color: Some(t.json_punctuation),
                        ..Default::default()
                    },
                ));
                i += 1;
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
        let mut brace_stack = Vec::new();
        let mut comment_start: Option<usize> = None;
        let s = text.to_string();
        let mut current_line = 0;
        let mut in_string = false;
        let mut string_quote = b'"';
        let mut escaped = false;
        let bytes = s.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            let b = bytes[i];
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == string_quote {
                    in_string = false;
                }
            } else if let Some(start_l) = comment_start {
                if b == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    comment_start = None;
                    if current_line > start_l {
                        folds.push(FoldRange::new(start_l, current_line));
                    }
                    i += 1;
                }
            } else {
                if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                    comment_start = Some(current_line);
                    i += 1;
                } else if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
                    // skip line
                    while i < len && bytes[i] != b'\n' {
                        i += 1;
                    }
                    current_line += 1;
                } else if b == b'"' || b == b'\'' || b == b'`' {
                    in_string = true;
                    string_quote = b;
                } else if b == b'{' || b == b'[' {
                    brace_stack.push(current_line);
                } else if b == b'}' || b == b']' {
                    if let Some(start_l) = brace_stack.pop() {
                        if current_line > start_l {
                            folds.push(FoldRange::new(start_l, current_line));
                        }
                    }
                } else if b == b'\n' {
                    current_line += 1;
                }
            }
            if in_string && b == b'\n' {
                current_line += 1;
            }
            i += 1;
        }

        folds
    }
}
