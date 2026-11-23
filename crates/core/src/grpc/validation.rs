//! Pure gRPC message JSON validation against a resolved protobuf schema.

use prost_reflect::{DescriptorPool, DynamicMessage};

use super::schema::method_descriptors;

/// A failed JSON message validation, positioned on the source text.
/// `line`/`column` are zero-based character coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageValidationError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Validates the gRPC message JSON: syntax first, then schema conformance
/// (strict unknown-field checking, matching the send path) when the
/// selection resolves against the fetched pool. An empty editor is the
/// proto3 zero-value message.
pub fn validate_message(
    pool: &DescriptorPool,
    service: &str,
    method: &str,
    text: &str,
) -> Result<(), MessageValidationError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    if let Err(e) = serde_json::from_str::<serde_json::Value>(text) {
        return Err(MessageValidationError {
            line: e.line().saturating_sub(1),
            column: e.column().saturating_sub(1),
            message: format!("Invalid JSON: {e}"),
        });
    }
    let Some((input, _)) = method_descriptors(pool, service, method) else {
        return Ok(());
    };
    let mut deserializer = serde_json::Deserializer::from_str(text);
    if let Err(e) = DynamicMessage::deserialize(input, &mut deserializer) {
        return Err(MessageValidationError {
            line: e.line().saturating_sub(1),
            column: e.column().saturating_sub(1),
            message: e.to_string(),
        });
    }
    if let Err(e) = deserializer.end() {
        return Err(MessageValidationError {
            line: e.line().saturating_sub(1),
            column: e.column().saturating_sub(1),
            message: e.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
