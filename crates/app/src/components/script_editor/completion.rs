use super::ScriptEditorKind;
use crate::components::env_autocomplete::{detect_unclosed_env_var, get_env_completions};
use crate::state::EnvironmentState;
use anyhow::Result;
use gpui_kit::component::input::{CompletionProvider, Rope};
use gpui_kit::component::RopeExt;
use gpui_kit::{App, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};

pub(crate) struct CandidateDef {
    pub(crate) name: &'static str,
    pub(crate) insert_text: &'static str,
    pub(crate) kind: CompletionItemKind,
    pub(crate) detail: &'static str,
    pub(crate) doc: &'static str,
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

    pub(crate) fn get_candidates(&self, base_path: &str) -> Vec<CandidateDef> {
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
