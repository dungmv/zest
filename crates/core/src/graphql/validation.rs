//! Pure GraphQL variables JSON validation (syntax + root object check).

/// A failed GraphQL variables JSON validation, positioned on the source text.
/// `line`/`column` are zero-based character coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariablesValidationError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Validates the GraphQL variables JSON: syntax plus "must be a JSON object".
/// An empty or whitespace-only editor is accepted as no variables.
pub fn validate_variables(text: &str) -> Result<(), VariablesValidationError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Err(e) => Err(VariablesValidationError {
            line: e.line().saturating_sub(1),
            column: e.column().saturating_sub(1),
            message: format!("Invalid JSON: {e}"),
        }),
        Ok(value) if !value.is_object() => Err(VariablesValidationError {
            line: 0,
            column: 0,
            message: "Variables must be a JSON object.".to_string(),
        }),
        Ok(_) => Ok(()),
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
