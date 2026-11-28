mod completion;
mod completion_catalog;
mod editor;
mod tab;
#[cfg(test)]
mod tests;

pub(super) use completion::ScriptCompletionProvider;
use editor::ScriptEditor;
pub(super) use tab::render_script_tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScriptEditorKind {
    PreRequest,
    Tests,
}
