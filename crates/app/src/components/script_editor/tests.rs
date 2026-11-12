use super::*;
use gpui_kit::component::input::{
    EditorState as CodeEditorState, HighlightStyleResolver, InputHighlighter, Rope,
};
use gpui_kit::HighlightStyle;

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
