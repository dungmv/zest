//! Centralized GPUI environment variable bindings (`crate::env`).
//!
//! Pure domain resolution lives in `zest_core::env`; this module provides the
//! single UI binding layer shared across `InputState` (URL bar, KV tables, auth,
//! multipart), `EditorState` (JSON, XML, GraphQL, gRPC), and script completion.

pub mod completion;
pub mod editor_lsp;
pub mod input_tokens;
pub mod popup;
pub mod style;

pub use completion::{detect_unclosed_env_var, unclosed_env_span, EnvCompletionProvider};
pub use editor_lsp::{attach_env_lsp, EnvEditorDecorations};
pub use input_tokens::{attach_missing_var_tokens, build_tokenized_content, RetokenizeGuard};
pub use popup::{
    handle_autocomplete_keydown, render_candidate_detail_card, render_candidate_row,
    AutocompleteNavState, CANDIDATE_DETAIL_GAP, CANDIDATE_LIST_WIDTH,
};
pub use style::{
    list_candidates_in_state, open_variable_in_modal, resolve_in_state, variable_style,
    variable_value_tooltip,
};
pub use zest_core::env::{ResolvedVariable, VariableScope};
