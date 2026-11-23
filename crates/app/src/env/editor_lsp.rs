//! Multi-line `EditorState` environment variable decorations and LSP bindings
//! (scope-aware pill tint, hover value preview, and Cmd/Ctrl+Click Go to Definition).

use super::style::{open_variable_in_modal, resolve_in_state, variable_style};
use crate::state::{EnvironmentState, ModalState};
use anyhow::Result;
use gpui_kit::component::input::{
    DefinitionProvider, EditorState as CodeEditorState, HoverProvider, RangeDecoration,
    RangeDecorationCollection, RangeDecorationStyle, Rope, ShowDocumentHandler, TextDecoration,
    TextDecorationCollection,
};
use gpui_kit::component::RopeExt;
use gpui_kit::{App, Context, Entity, FontWeight, HighlightStyle, Task, Window};
use lsp_types::{
    Hover, HoverContents, LocationLink, MarkupContent, MarkupKind, Range as LspRange,
    ShowDocumentParams, Uri,
};
use std::rc::Rc;
use std::str::FromStr;
use zest_core::env::{find_variable_span_at, ResolvedVariable, VariableScope};

const ENV_URI_PREFIX: &str = "zest://env/";

/// Holds the `RangeDecorationCollection` (tinted pill fill) and
/// `TextDecorationCollection` (scope-colored semibold text) for one code editor.
///
/// The entire `{{variable}}` span is styled uniformly (braces are not dimmed).
#[derive(Clone)]
pub struct EnvEditorDecorations {
    ranges: RangeDecorationCollection,
    texts: TextDecorationCollection,
}

impl EnvEditorDecorations {
    pub fn new(state: &mut CodeEditorState, cx: &mut Context<CodeEditorState>) -> Self {
        Self {
            ranges: state.create_range_decorations_collection(vec![], cx),
            texts: state.create_decorations_collection(vec![], cx),
        }
    }

    /// Recomputes and applies decorations for all `{{var}}` spans in `text`.
    ///
    /// Computes decoration vectors while immutably borrowing `env_state` and
    /// applies them afterwards, avoiding any `Environment` clone.
    pub fn sync(&self, text: &str, env_state: &Entity<EnvironmentState>, cx: &mut App) {
        if !text.contains("{{") {
            self.ranges.set(vec![], cx);
            self.texts.set(vec![], cx);
            return;
        }

        let spans = zest_core::env::scan_variable_spans(text);
        if spans.is_empty() {
            self.ranges.set(vec![], cx);
            self.texts.set(vec![], cx);
            return;
        }

        let (range_decs, text_decs) = {
            let env = env_state.read(cx);
            let mut range_decs = Vec::with_capacity(spans.len());
            let mut text_decs = Vec::with_capacity(spans.len());

            for span in spans {
                if span.start >= span.end || span.end > text.len() {
                    continue;
                }
                let resolved = resolve_in_state(env, &span.name);
                let style = variable_style(&resolved.scope);

                range_decs.push(
                    RangeDecoration::new(span.start..span.end)
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(style.bg_tint),
                );
                text_decs.push(TextDecoration::new(
                    span.start..span.end,
                    HighlightStyle {
                        color: Some(style.text),
                        font_weight: Some(FontWeight::SEMIBOLD),
                        ..Default::default()
                    },
                ));
            }
            (range_decs, text_decs)
        };

        self.ranges.set(range_decs, cx);
        self.texts.set(text_decs, cx);
    }
}

/// Formats the Markdown body shown inside the editor's native `HoverPopover`.
pub fn format_hover_markdown(resolved: &ResolvedVariable) -> String {
    if resolved.is_resolved() {
        let display_val = if resolved.value.is_empty() {
            "\"\"".to_string()
        } else {
            resolved.value.replace('`', "\\`")
        };
        format!(
            "**`{{{{{}}}}}`** · *{}*\n\n`{}`",
            resolved.name,
            resolved.scope_label(),
            display_val
        )
    } else {
        format!(
            "**`{{{{{}}}}}`** · *Missing*\n\nUnresolved variable. Cmd/Ctrl+Click to configure in Environment.",
            resolved.name
        )
    }
}

/// LSP `HoverProvider` that displays the resolved value and scope of `{{var}}` spans.
pub struct EnvHoverProvider {
    env_state: Entity<EnvironmentState>,
}

impl EnvHoverProvider {
    pub fn new(env_state: Entity<EnvironmentState>) -> Self {
        Self { env_state }
    }
}

impl HoverProvider for EnvHoverProvider {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Option<Hover>>> {
        let full_text = text.to_string();
        let Some(span) = find_variable_span_at(&full_text, offset) else {
            return Task::ready(Ok(None));
        };

        let resolved = resolve_in_state(self.env_state.read(cx), &span.name);
        let range = LspRange {
            start: text.offset_to_position(span.start),
            end: text.offset_to_position(span.end),
        };

        Task::ready(Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format_hover_markdown(&resolved),
            }),
            range: Some(range),
        })))
    }
}

/// LSP `DefinitionProvider` enabling Cmd/Ctrl+Click on `{{var}}` spans to jump to the Environment modal.
pub struct EnvDefinitionProvider {
    env_state: Entity<EnvironmentState>,
}

impl EnvDefinitionProvider {
    pub fn new(env_state: Entity<EnvironmentState>) -> Self {
        Self { env_state }
    }
}

impl DefinitionProvider for EnvDefinitionProvider {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<LocationLink>>> {
        let full_text = text.to_string();
        let Some(span) = find_variable_span_at(&full_text, offset) else {
            return Task::ready(Ok(vec![]));
        };

        let resolved = resolve_in_state(self.env_state.read(cx), &span.name);
        if matches!(resolved.scope, VariableScope::Dynamic) {
            return Task::ready(Ok(vec![]));
        }

        let Some(target_uri) = encode_env_uri(&span.name) else {
            return Task::ready(Ok(vec![]));
        };

        let origin_range = LspRange {
            start: text.offset_to_position(span.start),
            end: text.offset_to_position(span.end),
        };

        Task::ready(Ok(vec![LocationLink {
            origin_selection_range: Some(origin_range),
            target_uri,
            target_range: LspRange::default(),
            target_selection_range: LspRange::default(),
        }]))
    }
}

/// Builds the `window/showDocument` handler that intercepts `zest://env/<name>`
/// URIs emitted by [`EnvDefinitionProvider`] and opens the Environment modal.
pub fn make_env_show_document_handler(
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
) -> ShowDocumentHandler {
    Rc::new(
        move |params: &ShowDocumentParams, _window: &mut Window, cx: &mut App| {
            if let Some(var_name) = decode_env_uri(&params.uri) {
                open_variable_in_modal(&env_state, &modal_state, &var_name, cx);
                true
            } else {
                false
            }
        },
    )
}

/// Attaches `EnvHoverProvider`, `EnvDefinitionProvider`, and `show_document`
/// to an `EditorState` in a single call.
pub fn attach_env_lsp(
    state: &mut CodeEditorState,
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
) {
    let lsp = state.lsp_mut();
    lsp.hover_provider = Some(Rc::new(EnvHoverProvider::new(env_state.clone())));
    lsp.definition_provider = Some(Rc::new(EnvDefinitionProvider::new(env_state.clone())));
    lsp.show_document = Some(make_env_show_document_handler(env_state, modal_state));
}

/// Encodes a variable name into a virtual `zest://env/<percent-encoded>` URI.
pub fn encode_env_uri(var_name: &str) -> Option<Uri> {
    let mut encoded = String::with_capacity(ENV_URI_PREFIX.len() + var_name.len());
    encoded.push_str(ENV_URI_PREFIX);
    for b in var_name.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            encoded.push(*b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    Uri::from_str(&encoded).ok()
}

/// Decodes a `zest://env/<percent-encoded>` URI back into the variable name.
pub fn decode_env_uri(uri: &Uri) -> Option<String> {
    let raw = uri.as_str().strip_prefix(ENV_URI_PREFIX)?;
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            let val = u8::from_str_radix(hex, 16).ok()?;
            out.push(val);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hover_markdown_variants() {
        let active = ResolvedVariable {
            name: "bookingId".to_string(),
            value: "BK-123".to_string(),
            scope: VariableScope::Active("dev".to_string()),
            is_secret: false,
        };
        let md = format_hover_markdown(&active);
        assert!(md.contains("`{{bookingId}}`"));
        assert!(md.contains("*Active: dev*"));
        assert!(md.contains("`BK-123`"));

        let secret = ResolvedVariable {
            name: "apiKey".to_string(),
            value: zest_core::env::SECRET_MASK.to_string(),
            scope: VariableScope::Global,
            is_secret: true,
        };
        let secret_md = format_hover_markdown(&secret);
        assert!(secret_md.contains("*Global*"));
        assert!(secret_md.contains(zest_core::env::SECRET_MASK));

        let dynamic = ResolvedVariable {
            name: "$guid".to_string(),
            value: "Dynamic: 1234-5678".to_string(),
            scope: VariableScope::Dynamic,
            is_secret: false,
        };
        let dyn_md = format_hover_markdown(&dynamic);
        assert!(dyn_md.contains("*Dynamic*"));
        assert!(dyn_md.contains("Dynamic: 1234-5678"));

        let missing = ResolvedVariable {
            name: "notDefined".to_string(),
            value: "Unresolved variable".to_string(),
            scope: VariableScope::Missing,
            is_secret: false,
        };
        let miss_md = format_hover_markdown(&missing);
        assert!(miss_md.contains("`{{notDefined}}`"));
        assert!(miss_md.contains("*Missing*"));
        assert!(miss_md.contains("Cmd/Ctrl+Click"));
    }

    #[test]
    fn test_env_uri_encode_decode_roundtrip() {
        for name in ["bookingId", "api_key-1.0", "$guid", "var with space"] {
            let uri = encode_env_uri(name).expect("valid URI");
            assert!(uri.as_str().starts_with(ENV_URI_PREFIX));
            assert_eq!(decode_env_uri(&uri).as_deref(), Some(name));
        }

        let other = Uri::from_str("https://example.com/foo").unwrap();
        assert_eq!(decode_env_uri(&other), None);
    }

    #[gpui_kit::test]
    async fn test_definition_provider_skips_dynamic_and_resolves_env_vars(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext as _;

        let dir = tempfile::tempdir().unwrap();
        let store: std::sync::Arc<dyn zest_core::storage::WorkspaceStore> =
            std::sync::Arc::new(zest_core::storage::FsWorkspaceStore::new(dir.path()));
        store.init_workspace("DefTestWS").unwrap();

        let env_state =
            cx.new(|_| EnvironmentState::new(store.clone(), "DefTestWS".to_string(), None));
        let provider = EnvDefinitionProvider::new(env_state);

        let (_view, cx) = cx.add_window_view(CodeEditorState::new);
        let text = Rope::from("{\"id\": \"{{bookingId}}\", \"u\": \"{{$guid}}\"}");
        let booking_offset = text.to_string().find("bookingId").unwrap();
        let guid_offset = text.to_string().find("$guid").unwrap();

        let booking_links = cx
            .update(|window, cx| provider.definitions(&text, booking_offset, window, cx))
            .await
            .unwrap();
        assert_eq!(booking_links.len(), 1);
        assert_eq!(
            decode_env_uri(&booking_links[0].target_uri).as_deref(),
            Some("bookingId")
        );

        let guid_links = cx
            .update(|window, cx| provider.definitions(&text, guid_offset, window, cx))
            .await
            .unwrap();
        assert!(
            guid_links.is_empty(),
            "dynamic variables must not emit Go to Definition links"
        );
    }
}
