pub mod completion;
mod completion_catalog;
pub mod editor;
pub mod highlighter;
#[cfg(test)]
pub mod tests;

pub use completion::*;
pub use editor::*;
pub use highlighter::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptEditorKind {
    PreRequest,
    Tests,
}
