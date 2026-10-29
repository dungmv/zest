use crate::components::env_autocomplete::{detect_unclosed_env_var, get_env_completions};
use crate::state::EnvironmentState;
use crate::theme::theme;
use anyhow::Result;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Context, Entity, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled, Task, Window,
};
use gpui_kit::component::input::{
    CompletionProvider, Editor, EditorState as CodeEditorState, FoldRange, HighlightStyleResolver,
    InputEdit, InputHighlighter, Rope,
};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::RopeExt;
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Position, Range as LspRange, TextEdit,
};
use std::ops::Range;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptEditorKind {
    PreRequest,
    Tests,
}

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

// =========================================================================
// JavaScript Syntax Highlighter
// =========================================================================

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

// =========================================================================
// Contextual Auto-Complete Provider
// =========================================================================

struct CandidateDef {
    name: &'static str,
    insert_text: &'static str,
    kind: CompletionItemKind,
    detail: &'static str,
    doc: &'static str,
}

pub struct ScriptCompletionProvider {
    kind: ScriptEditorKind,
    env_state: Option<Entity<EnvironmentState>>,
}

impl ScriptCompletionProvider {
    #[allow(dead_code)]
    pub fn new(kind: ScriptEditorKind) -> Self {
        Self {
            kind,
            env_state: None,
        }
    }

    pub fn with_environment(kind: ScriptEditorKind, env_state: Entity<EnvironmentState>) -> Self {
        Self {
            kind,
            env_state: Some(env_state),
        }
    }

    fn get_candidates(&self, base_path: &str) -> Vec<CandidateDef> {
        let is_tests = self.kind == ScriptEditorKind::Tests;

        match base_path {
            // Root z
            "z" => {
                let mut list = vec![
                    CandidateDef {
                        name: "environment",
                        insert_text: "environment",
                        kind: CompletionItemKind::PROPERTY,
                        detail: "Environment",
                        doc: "Persistent environment variables storage for sharing state between requests.",
                    },
                    CandidateDef {
                        name: "variables",
                        insert_text: "variables",
                        kind: CompletionItemKind::PROPERTY,
                        detail: "Variables",
                        doc: "Collection & runtime variables accessible during execution.",
                    },
                    CandidateDef {
                        name: "request",
                        insert_text: "request",
                        kind: CompletionItemKind::PROPERTY,
                        detail: "Request",
                        doc: "Current HTTP request configuration (headers, body, url, method).",
                    },
                    CandidateDef {
                        name: "crypto",
                        insert_text: "crypto",
                        kind: CompletionItemKind::PROPERTY,
                        detail: "Crypto",
                        doc: "Cryptographic utilities (HMAC, SHA256, MD5, Base64, UUID).",
                    },
                    CandidateDef {
                        name: "assert",
                        insert_text: "assert()",
                        kind: CompletionItemKind::METHOD,
                        detail: "(condition: boolean, message?: string): void",
                        doc: "Asserts that a condition is true, throwing an error with message if false.",
                    },
                ];

                if is_tests {
                    list.push(CandidateDef {
                        name: "response",
                        insert_text: "response",
                        kind: CompletionItemKind::PROPERTY,
                        detail: "Response",
                        doc: "Received HTTP response data (status, statusText, time, body, json(), headers).",
                    });
                    list.push(CandidateDef {
                        name: "test",
                        insert_text: "test(\"\", () => {\n    \n})",
                        kind: CompletionItemKind::METHOD,
                        detail: "(name: string, callback: () => void): void",
                        doc: "Defines a named test assertion suite. Executes callback and records pass/fail.",
                    });
                }

                list
            }

            // z.environment
            "z.environment" => vec![
                CandidateDef {
                    name: "get",
                    insert_text: "get()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(key: string): string | null",
                    doc: "Returns the value of the environment variable, or null if not found.",
                },
                CandidateDef {
                    name: "set",
                    insert_text: "set()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(key: string, value: string): void",
                    doc: "Sets an environment variable value for the current active environment.",
                },
                CandidateDef {
                    name: "has",
                    insert_text: "has()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(key: string): boolean",
                    doc: "Checks whether an environment variable exists.",
                },
                CandidateDef {
                    name: "unset",
                    insert_text: "unset()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(key: string): void",
                    doc: "Deletes the specified environment variable.",
                },
                CandidateDef {
                    name: "clear",
                    insert_text: "clear()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(): void",
                    doc: "Clears all environment variables in this environment.",
                },
                CandidateDef {
                    name: "toObject",
                    insert_text: "toObject()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(): Record<string, string>",
                    doc:
                        "Returns all active environment variables as a key-value JavaScript object.",
                },
            ],

            // z.variables
            "z.variables" => vec![
                CandidateDef {
                    name: "get",
                    insert_text: "get()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(key: string): string | null",
                    doc: "Returns the value of a scoped variable.",
                },
                CandidateDef {
                    name: "set",
                    insert_text: "set()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(key: string, value: string): void",
                    doc: "Sets the value of a scoped variable.",
                },
                CandidateDef {
                    name: "has",
                    insert_text: "has()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(key: string): boolean",
                    doc: "Returns true if the scoped variable exists.",
                },
            ],

            // z.request
            "z.request" => vec![
                CandidateDef {
                    name: "headers",
                    insert_text: "headers",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "RequestHeaders",
                    doc: "Request headers accessor (add, get, all).",
                },
                CandidateDef {
                    name: "body",
                    insert_text: "body",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "RequestBody",
                    doc: "Request body accessor (json, setJson, raw).",
                },
                CandidateDef {
                    name: "url",
                    insert_text: "url",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "string",
                    doc: "The target URL string for this request.",
                },
                CandidateDef {
                    name: "method",
                    insert_text: "method",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "string",
                    doc: "The HTTP method of this request (e.g. GET, POST).",
                },
            ],

            // z.request.headers
            "z.request.headers" => vec![
                CandidateDef {
                    name: "add",
                    insert_text: "add()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(name: string, value: string): void",
                    doc: "Adds or updates an HTTP header on the outgoing request.",
                },
                CandidateDef {
                    name: "get",
                    insert_text: "get()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(name: string): string | null",
                    doc: "Gets the value of a request header by name.",
                },
                CandidateDef {
                    name: "all",
                    insert_text: "all()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(): Record<string, string>",
                    doc: "Returns all request headers as a key-value object.",
                },
            ],

            // z.request.body
            "z.request.body" => vec![
                CandidateDef {
                    name: "json",
                    insert_text: "json()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(): any",
                    doc: "Parses the request JSON payload into a mutable JavaScript object.",
                },
                CandidateDef {
                    name: "setJson",
                    insert_text: "setJson()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(value: any): void",
                    doc: "Serializes the given JavaScript object and replaces the request body.",
                },
                CandidateDef {
                    name: "raw",
                    insert_text: "raw()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(): string",
                    doc: "Returns the raw body payload string.",
                },
            ],

            // z.response
            "z.response" if is_tests => vec![
                CandidateDef {
                    name: "status",
                    insert_text: "status",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "number",
                    doc: "HTTP response status code (e.g. 200, 404, 500).",
                },
                CandidateDef {
                    name: "statusText",
                    insert_text: "statusText",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "string",
                    doc: "HTTP status text (e.g. 'OK', 'Not Found').",
                },
                CandidateDef {
                    name: "time",
                    insert_text: "time",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "number",
                    doc: "Total roundtrip duration in milliseconds.",
                },
                CandidateDef {
                    name: "json",
                    insert_text: "json()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(): any",
                    doc: "Parses the response body as a JSON object, or returns null if invalid.",
                },
                CandidateDef {
                    name: "body",
                    insert_text: "body",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "string",
                    doc: "Full UTF-8 response body content.",
                },
                CandidateDef {
                    name: "headers",
                    insert_text: "headers",
                    kind: CompletionItemKind::PROPERTY,
                    detail: "ResponseHeaders",
                    doc: "Headers received in the HTTP response.",
                },
            ],

            // z.response.headers
            "z.response.headers" if is_tests => {
                vec![
                    CandidateDef {
                        name: "get",
                        insert_text: "get()",
                        kind: CompletionItemKind::METHOD,
                        detail: "(name: string): string | null",
                        doc: "Gets the value of a response header (case-insensitive).",
                    },
                    CandidateDef {
                        name: "all",
                        insert_text: "all()",
                        kind: CompletionItemKind::METHOD,
                        detail: "(): Record<string, string>",
                        doc: "Returns all response headers as a key-value object.",
                    },
                ]
            }

            // z.crypto
            "z.crypto" => vec![
                CandidateDef {
                    name: "hmacSha256",
                    insert_text: "hmacSha256()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(secret: string, message: string): string",
                    doc: "Computes HMAC-SHA256 hex digest for message with the secret.",
                },
                CandidateDef {
                    name: "hmacSha512",
                    insert_text: "hmacSha512()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(secret: string, message: string): string",
                    doc: "Computes HMAC-SHA512 hex digest.",
                },
                CandidateDef {
                    name: "sha256",
                    insert_text: "sha256()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(data: string): string",
                    doc: "Computes SHA-256 hex digest.",
                },
                CandidateDef {
                    name: "sha512",
                    insert_text: "sha512()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(data: string): string",
                    doc: "Computes SHA-512 hex digest.",
                },
                CandidateDef {
                    name: "md5",
                    insert_text: "md5()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(data: string): string",
                    doc: "Computes MD5 hex digest.",
                },
                CandidateDef {
                    name: "sha1",
                    insert_text: "sha1()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(data: string): string",
                    doc: "Computes SHA-1 hex digest.",
                },
                CandidateDef {
                    name: "base64Encode",
                    insert_text: "base64Encode()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(data: string): string",
                    doc: "Encodes string data to standard Base64.",
                },
                CandidateDef {
                    name: "base64Decode",
                    insert_text: "base64Decode()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(data: string): string",
                    doc: "Decodes standard Base64 string to original text.",
                },
                CandidateDef {
                    name: "uuidv4",
                    insert_text: "uuidv4()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(): string",
                    doc: "Generates a random UUID version 4 string.",
                },
            ],

            // console
            "console" => vec![
                CandidateDef {
                    name: "log",
                    insert_text: "log()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(...args: any[]): void",
                    doc: "Logs standard messages to the script output console.",
                },
                CandidateDef {
                    name: "warn",
                    insert_text: "warn()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(...args: any[]): void",
                    doc: "Logs warning messages to the script output console.",
                },
                CandidateDef {
                    name: "error",
                    insert_text: "error()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(...args: any[]): void",
                    doc: "Logs error messages to the script output console.",
                },
                CandidateDef {
                    name: "info",
                    insert_text: "info()",
                    kind: CompletionItemKind::METHOD,
                    detail: "(...args: any[]): void",
                    doc: "Logs informational messages to the script output console.",
                },
            ],

            // Global scope
            "" => {
                let mut list = vec![
                    CandidateDef {
                        name: "z",
                        insert_text: "z.",
                        kind: CompletionItemKind::MODULE,
                        detail: "Zest",
                        doc: "Zest root scripting context (z.response, z.environment, z.request, z.test).",
                    },
                    CandidateDef {
                        name: "console",
                        insert_text: "console.",
                        kind: CompletionItemKind::MODULE,
                        detail: "Console",
                        doc: "Console logging methods (log, warn, error).",
                    },
                    CandidateDef {
                        name: "const",
                        insert_text: "const ",
                        kind: CompletionItemKind::KEYWORD,
                        detail: "keyword",
                        doc: "Declare an immutable block-scoped variable.",
                    },
                    CandidateDef {
                        name: "let",
                        insert_text: "let ",
                        kind: CompletionItemKind::KEYWORD,
                        detail: "keyword",
                        doc: "Declare a mutable block-scoped variable.",
                    },
                    CandidateDef {
                        name: "function",
                        insert_text: "function name() {\n    \n}",
                        kind: CompletionItemKind::KEYWORD,
                        detail: "keyword",
                        doc: "Declare a JavaScript function.",
                    },
                    CandidateDef {
                        name: "JSON.stringify",
                        insert_text: "JSON.stringify()",
                        kind: CompletionItemKind::FUNCTION,
                        detail: "(value: any): string",
                        doc: "Converts a JavaScript value to a JSON string.",
                    },
                    CandidateDef {
                        name: "JSON.parse",
                        insert_text: "JSON.parse()",
                        kind: CompletionItemKind::FUNCTION,
                        detail: "(text: string): any",
                        doc: "Parses a JSON string into a JavaScript object.",
                    },
                    CandidateDef {
                        name: "Date.now",
                        insert_text: "Date.now()",
                        kind: CompletionItemKind::FUNCTION,
                        detail: "(): number",
                        doc: "Returns milliseconds elapsed since Unix epoch.",
                    },
                ];

                if is_tests {
                    list.push(CandidateDef {
                        name: "z.test",
                        insert_text: "z.test(\"Status is 200\", () => {\n    z.assert(z.response.status === 200, \"Expected 200 OK\");\n});\n",
                        kind: CompletionItemKind::SNIPPET,
                        detail: "Snippet: Status code assertion",
                        doc: "Generates a complete test suite verifying HTTP 200 OK status.",
                    });
                    list.push(CandidateDef {
                        name: "z.assert",
                        insert_text: "z.assert(z.response.status === 200, \"Status must be 200\");",
                        kind: CompletionItemKind::SNIPPET,
                        detail: "Snippet: Assertion statement",
                        doc: "Asserts a condition in test scripts.",
                    });
                }

                list
            }

            _ => vec![],
        }
    }
    pub fn should_trigger(new_text: &str) -> bool {
        new_text.contains('{')
            || new_text.contains('"')
            || new_text.contains('\'')
            || new_text
                .chars()
                .any(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '$')
    }
}

impl CompletionProvider for ScriptCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let full_text = rope.to_string();
        let safe_offset = offset.min(full_text.len());
        let slice = &full_text[..safe_offset];

        // 1. Check for unclosed {{ or {{prefix in script
        if let Some((start_pos_byte, _end_pos_byte, q)) = detect_unclosed_env_var(slice) {
            if let Some(env_entity) = &self.env_state {
                let env = env_entity.read(cx);
                let candidates = get_env_completions(env, &q);
                let start_pos = rope.offset_to_position(start_pos_byte);
                let end_pos = rope.offset_to_position(safe_offset);
                let items: Vec<CompletionItem> = candidates
                    .into_iter()
                    .map(|c| CompletionItem {
                        label: format!("{{{{{}}}}}", c.name),
                        filter_text: Some(c.name.clone()),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!("{} • {}", c.detail_text(), c.scope)),
                        documentation: Some(Documentation::String(format!(
                            "Environment Variable: {}\nValue: {}\nScope: {}",
                            c.name,
                            c.detail_text(),
                            c.scope
                        ))),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: LspRange {
                                start: start_pos,
                                end: end_pos,
                            },
                            new_text: format!("{{{{{}}}}}", c.name),
                        })),
                        ..Default::default()
                    })
                    .collect();
                return Task::ready(Ok(CompletionResponse::Array(items)));
            }
        }

        // 2. Check for variable key inside z.environment.get("...", z.variables.get("...", etc.
        if let Some(quote_idx) = slice.rfind(['"', '\'']) {
            let before_quote = slice[..quote_idx].trim_end();
            if before_quote.ends_with(".get(")
                || before_quote.ends_with(".has(")
                || before_quote.ends_with(".unset(")
                || before_quote.ends_with(".set(")
            {
                let var_query = &slice[quote_idx + 1..];
                if !var_query.contains(['"', '\'', '\n']) {
                    if let Some(env_entity) = &self.env_state {
                        let env = env_entity.read(cx);
                        let candidates = get_env_completions(env, var_query);
                        let start_lsp = rope.offset_to_position(quote_idx + 1);
                        let end_lsp = rope.offset_to_position(safe_offset);
                        let items: Vec<CompletionItem> = candidates
                            .into_iter()
                            .map(|c| CompletionItem {
                                label: c.name.clone(),
                                filter_text: Some(c.name.clone()),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("{} • {}", c.detail_text(), c.scope)),
                                documentation: Some(Documentation::String(format!(
                                    "Environment Variable: {}\nValue: {}\nScope: {}",
                                    c.name,
                                    c.detail_text(),
                                    c.scope
                                ))),
                                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                    range: LspRange {
                                        start: start_lsp,
                                        end: end_lsp,
                                    },
                                    new_text: c.name,
                                })),
                                ..Default::default()
                            })
                            .collect();
                        return Task::ready(Ok(CompletionResponse::Array(items)));
                    }
                }
            }
        }

        let query_start = slice
            .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '$' && c != '.')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let expr = &slice[query_start..];

        let (base_path, query, replace_start) = if let Some(dot_idx) = expr.rfind('.') {
            let base = &expr[..dot_idx];
            let q = &expr[dot_idx + 1..];
            (base.to_string(), q.to_string(), query_start + dot_idx + 1)
        } else {
            ("".to_string(), expr.to_string(), query_start)
        };

        let start_pos = rope.offset_to_position(replace_start);
        let end_pos = rope.offset_to_position(safe_offset);

        let candidates = self.get_candidates(&base_path);
        let query_lower = query.to_lowercase();

        let items: Vec<CompletionItem> = candidates
            .into_iter()
            .filter(|c| c.name.to_lowercase().starts_with(&query_lower))
            .map(|c| CompletionItem {
                label: c.name.to_string(),
                filter_text: Some(c.name.to_string()),
                kind: Some(c.kind),
                detail: Some(c.detail.to_string()),
                documentation: Some(Documentation::String(c.doc.to_string())),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: LspRange {
                        start: start_pos,
                        end: end_pos,
                    },
                    new_text: c.insert_text.to_string(),
                })),
                ..Default::default()
            })
            .collect();

        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        Self::should_trigger(new_text)
    }
}

// =========================================================================
// ScriptEditor Component
// =========================================================================

#[allow(clippy::type_complexity)]
#[derive(IntoElement)]
pub struct ScriptEditor {
    editor: Entity<CodeEditorState>,
    kind: ScriptEditorKind,
    error_message: Option<String>,
    on_insert_snippet: Rc<dyn Fn(&str, &mut Window, &mut App)>,
    split_layout_vertical: bool,
}

impl ScriptEditor {
    pub fn new(
        editor: Entity<CodeEditorState>,
        kind: ScriptEditorKind,
        error_message: Option<String>,
        on_insert_snippet: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            editor,
            kind,
            error_message,
            on_insert_snippet: Rc::new(on_insert_snippet),
            split_layout_vertical: false,
        }
    }

    pub fn split_layout_vertical(mut self, vertical: bool) -> Self {
        self.split_layout_vertical = vertical;
        self
    }
}

impl RenderOnce for ScriptEditor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let kind = self.kind;
        let on_insert = self.on_insert_snippet.clone();

        let scroll_handle = window.use_keyed_state(
            SharedString::from(match kind {
                ScriptEditorKind::PreRequest => "snippets-top-scroll-pre",
                ScriptEditorKind::Tests => "snippets-top-scroll-tests",
            }),
            cx,
            |_window, _cx| gpui_kit::ScrollHandle::new(),
        );
        let scr_h = scroll_handle.read(cx).clone();

        let snippets: Vec<(&'static str, &'static str)> = match kind {
            ScriptEditorKind::PreRequest => vec![
                (
                    "Set Timestamp Header",
                    "z.request.headers.add(\"X-Timestamp\", Date.now().toString());\n",
                ),
                (
                    "HMAC-SHA256 Signature",
                    "const sig = z.crypto.hmacSha256(\"my_secret\", Date.now().toString());\nz.request.headers.add(\"X-Signature\", sig);\n",
                ),
                (
                    "Set Env Variable",
                    "z.environment.set(\"custom_key\", \"custom_value\");\n",
                ),
                (
                    "Mutate JSON Body",
                    "const body = z.request.body.json();\nif (body) {\n    body.timestamp = Date.now();\n    z.request.body.setJson(body);\n}\n",
                ),
            ],
            ScriptEditorKind::Tests => vec![
                (
                    "Status code: 200",
                    "z.test(\"Status code is 200\", () => {\n    z.assert(z.response.status === 200, \"Expected 200 OK\");\n});\n",
                ),
                (
                    "Response time < 500ms",
                    "z.test(\"Response time is under 500ms\", () => {\n    z.assert(z.response.time < 500, \"Too slow\");\n});\n",
                ),
                (
                    "JSON value check",
                    "z.test(\"Verify JSON property\", () => {\n    const data = z.response.json();\n    z.assert(data !== null, \"Response should be JSON\");\n});\n",
                ),
                (
                    "Save Token to Env",
                    "// Request Chaining: extract token from response\nconst data = z.response.json();\nif (data && data.token) {\n    z.environment.set(\"access_token\", data.token);\n}\n",
                ),
            ],
        };

        let snippet_buttons: Vec<_> = snippets
            .into_iter()
            .enumerate()
            .map(|(idx, (label, code))| {
                let insert_cb = on_insert.clone();
                let snippet_code = code;
                div()
                    .id(format!(
                        "snippet-btn-{}-{}",
                        match kind {
                            ScriptEditorKind::PreRequest => "pre",
                            ScriptEditorKind::Tests => "test",
                        },
                        idx
                    ))
                    .flex_shrink_0()
                    .px(px(8.))
                    .py(px(if self.split_layout_vertical { 5. } else { 3. }))
                    .rounded(px(4.))
                    .bg(t.bg_editor)
                    .border_1()
                    .border_color(t.border)
                    .text_xs()
                    .text_color(t.text_primary)
                    .hover(|s| s.bg(t.bg_hover).border_color(t.border_focus))
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        insert_cb(snippet_code, window, cx);
                    })
                    .child(label)
            })
            .collect();

        let workspace = if self.split_layout_vertical {
            // Horizontal split (Top: Composer / Bottom: Response): snippets on the RIGHT (vertical column)
            div()
                .flex_1()
                .flex()
                .overflow_hidden()
                .child(
                    // Left: Code Editor Area
                    div().flex_1().h_full().overflow_hidden().p(px(8.)).child(
                        Editor::new(&self.editor)
                            .appearance(false)
                            .bordered(false)
                            .h(gpui_kit::relative(1.)),
                    ),
                )
                // Right: Snippets Toolbar (vertical column)
                .child(
                    div()
                        .w(px(200.))
                        .h_full()
                        .flex_shrink_0()
                        .border_l_1()
                        .border_color(t.border)
                        .bg(t.bg_panel)
                        .p(px(10.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_secondary)
                                .mb(px(4.))
                                .child("SNIPPETS"),
                        )
                        .children(snippet_buttons),
                )
        } else {
            // Vertical split (Left: Composer / Right: Response columns): snippets on TOP in a single horizontal row with scroll
            div()
                .flex_1()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(
                    // Top: Snippets Toolbar (single horizontal row with scroll)
                    div()
                        .w_full()
                        .flex_shrink_0()
                        .border_b_1()
                        .border_color(t.border)
                        .bg(t.bg_panel)
                        .h(px(38.))
                        .px(px(8.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .overflow_hidden()
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_secondary)
                                .child("SNIPPETS:"),
                        )
                        .child(
                            div()
                                .id("snippets-top-scroll-container")
                                .flex_1()
                                .min_w(px(0.))
                                .h_full()
                                .relative()
                                .child(
                                    div()
                                        .id("snippets-top-scroll-track")
                                        .size_full()
                                        .track_scroll(&scr_h)
                                        .overflow_x_scroll()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.))
                                        .pr(px(24.))
                                        .children(snippet_buttons)
                                        .child(div().w(px(24.)).h(px(1.)).flex_shrink_0()),
                                )
                                .child(div().absolute().inset_0().child(
                                    Scrollbar::horizontal(&scr_h).mode(ScrollbarMode::Always),
                                )),
                        ),
                )
                .child(
                    // Bottom: Code Editor Area
                    div().flex_1().w_full().overflow_hidden().p(px(8.)).child(
                        Editor::new(&self.editor)
                            .appearance(false)
                            .bordered(false)
                            .h(gpui_kit::relative(1.)),
                    ),
                )
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg_editor)
            // Error banner if any
            .when_some(self.error_message, |this, err| {
                this.child(
                    div()
                        .px(px(12.))
                        .py(px(6.))
                        .bg(gpui_kit::Hsla {
                            a: 0.15,
                            ..t.method_delete
                        })
                        .border_b_1()
                        .border_color(gpui_kit::Hsla {
                            a: 0.35,
                            ..t.method_delete
                        })
                        .text_xs()
                        .text_color(t.method_delete)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child("! Script Error:")
                        .child(err),
                )
            })
            // Main workspace
            .child(workspace)
    }
}

// =========================================================================
// Tests
// =========================================================================

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
    fn test_js_highlighter_tokenizes_keywords_and_literals() {
        let mut highlighter = JavaScriptHighlighter::new();
        let code = "const token = z.response.json(); // comment\nz.assert(true, 'ok');";
        highlighter.text = Rope::from(code);
        let range = 0..code.len();
        let styles = highlighter.styles(&range, &DummyResolver);

        assert!(!styles.is_empty(), "Styles should not be empty");
        let styled_colored = styles.iter().any(|(_, s)| s.color.is_some());
        assert!(styled_colored, "There should be colored tokens");
    }

    #[test]
    fn test_js_highlighter_folding() {
        let highlighter = JavaScriptHighlighter::new();
        let code = "function test() {\n    if (true) {\n        console.log('nested');\n    }\n}\n";
        let rope = Rope::from(code);
        let folds = highlighter.fold_ranges(&rope);
        assert!(!folds.is_empty(), "Folds should contain ranges for braces");
    }

    #[test]
    fn test_script_completions_candidates_pre_request() {
        let provider = ScriptCompletionProvider::new(ScriptEditorKind::PreRequest);
        let root = provider.get_candidates("z");
        assert!(root.iter().any(|c| c.name == "environment"));
        assert!(root.iter().any(|c| c.name == "request"));
        assert!(root.iter().any(|c| c.name == "crypto"));
        assert!(
            !root.iter().any(|c| c.name == "response"),
            "Pre-request script should not suggest z.response"
        );
        assert!(
            !root.iter().any(|c| c.name == "test"),
            "Pre-request script should not suggest z.test"
        );
    }

    #[test]
    fn test_script_completions_candidates_tests() {
        let provider = ScriptCompletionProvider::new(ScriptEditorKind::Tests);
        let root = provider.get_candidates("z");
        assert!(root.iter().any(|c| c.name == "environment"));
        assert!(
            root.iter().any(|c| c.name == "response"),
            "Tests script must suggest z.response"
        );
        assert!(
            root.iter().any(|c| c.name == "test"),
            "Tests script must suggest z.test"
        );

        let resp = provider.get_candidates("z.response");
        assert!(resp.iter().any(|c| c.name == "status"));
        assert!(resp.iter().any(|c| c.name == "json"));
        assert!(resp.iter().any(|c| c.name == "headers"));
    }

    #[test]
    fn test_script_completions_crypto_methods() {
        let provider = ScriptCompletionProvider::new(ScriptEditorKind::PreRequest);
        let crypto = provider.get_candidates("z.crypto");
        assert!(crypto.iter().any(|c| c.name == "hmacSha256"));
        assert!(crypto.iter().any(|c| c.name == "sha256"));
        assert!(crypto.iter().any(|c| c.name == "uuidv4"));
        assert!(crypto.iter().any(|c| c.name == "base64Encode"));
    }

    #[test]
    fn test_script_completions_query_resolution() {
        let provider = ScriptCompletionProvider::new(ScriptEditorKind::Tests);

        // Test 1: "z."
        let code = "z.";
        let rope = Rope::from(code);
        let full_text = rope.to_string();
        let safe_offset = 2.min(full_text.len());
        let slice = &full_text[..safe_offset];
        let query_start = slice
            .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '$' && c != '.')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let expr = &slice[query_start..];
        let (base_path, query, replace_start) = if let Some(dot_idx) = expr.rfind('.') {
            (
                expr[..dot_idx].to_string(),
                expr[dot_idx + 1..].to_string(),
                query_start + dot_idx + 1,
            )
        } else {
            ("".to_string(), expr.to_string(), query_start)
        };
        assert_eq!(base_path, "z");
        assert_eq!(query, "");
        assert_eq!(replace_start, 2);

        // Test 2: "z.request.headers.a"
        let code2 = "z.request.headers.a";
        let rope2 = Rope::from(code2);
        let full_text2 = rope2.to_string();
        let safe_offset2 = code2.len();
        let slice2 = &full_text2[..safe_offset2];
        let query_start2 = slice2
            .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '$' && c != '.')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let expr2 = &slice2[query_start2..];
        let (base_path2, query2, replace_start2) = if let Some(dot_idx) = expr2.rfind('.') {
            (
                expr2[..dot_idx].to_string(),
                expr2[dot_idx + 1..].to_string(),
                query_start2 + dot_idx + 1,
            )
        } else {
            ("".to_string(), expr2.to_string(), query_start2)
        };
        assert_eq!(base_path2, "z.request.headers");
        assert_eq!(query2, "a");
        assert_eq!(replace_start2, 18);

        let candidates = provider.get_candidates(&base_path2);
        let filtered: Vec<_> = candidates
            .into_iter()
            .filter(|c| c.name.starts_with(&query2))
            .collect();
        assert!(filtered.iter().any(|c| c.name == "add"));
        assert!(filtered.iter().any(|c| c.name == "all"));
    }

    #[test]
    fn test_is_completion_trigger() {
        assert!(ScriptCompletionProvider::should_trigger("."));
        assert!(ScriptCompletionProvider::should_trigger("c"));
        assert!(ScriptCompletionProvider::should_trigger("_"));
        assert!(ScriptCompletionProvider::should_trigger("$"));
        assert!(!ScriptCompletionProvider::should_trigger(" "));
        assert!(!ScriptCompletionProvider::should_trigger("\n"));
    }

    #[gpui_kit::test]
    fn test_script_editor_split_layout_vertical_builder(cx: &mut gpui_kit::TestAppContext) {
        let (editor, _cx) = cx.add_window_view(CodeEditorState::new);
        let pre_editor = ScriptEditor::new(
            editor.clone(),
            ScriptEditorKind::PreRequest,
            None,
            |_, _, _| {},
        )
        .split_layout_vertical(true);
        assert!(pre_editor.split_layout_vertical);

        let test_editor = ScriptEditor::new(editor, ScriptEditorKind::Tests, None, |_, _, _| {})
            .split_layout_vertical(false);
        assert!(!test_editor.split_layout_vertical);
    }
}
