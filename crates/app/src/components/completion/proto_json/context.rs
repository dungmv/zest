//! Pure JSON-prefix walking: given the input message descriptor and the text
//! before the cursor, locate which message the cursor is inside and which
//! fields are suggested there. No editor or GPUI state involved.

use zest_core::grpc::MessageDescriptor;

use super::proto_fields::{field_as_message, find_field, type_label};

/// One suggested JSON key (owned so suggestions survive the context walk).
pub struct KeySuggestion {
    pub name: String,
    pub type_label: String,
}

/// Where a key completion applies and how the inserted text must look.
pub struct KeyCompletionSpan {
    pub replace_start: usize,
    pub replace_end: usize,
    /// True when no opening quote was typed: accepting a suggestion inserts
    /// the name wrapped in quotes.
    pub needs_quote: bool,
}

/// Suggests the JSON keys available at the cursor position of a partially
/// typed message, tracking nested objects through the typed JSON. Returns
/// the span the completion replaces (the partially typed key — with or
/// without its opening quote) plus the matching fields.
pub(super) fn json_field_candidates(
    root: &MessageDescriptor,
    prefix: &str,
) -> Option<(KeyCompletionSpan, Vec<KeySuggestion>)> {
    let (current, span) = locate_message_context(root, prefix)?;
    let query: String = prefix[span.replace_start..span.replace_end]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let mut candidates: Vec<KeySuggestion> = current
        .fields()
        .map(|field| KeySuggestion {
            name: field.name().to_string(),
            type_label: type_label(&field),
        })
        .collect();
    if !query.is_empty() {
        let lower = query.to_lowercase();
        candidates.retain(|s| s.name.to_lowercase().starts_with(&lower));
    }
    if candidates.is_empty() {
        None
    } else {
        Some((span, candidates))
    }
}

/// Walks the typed JSON prefix to find which message the cursor is inside
/// and where the current (partial) key begins.
fn locate_message_context(
    root: &MessageDescriptor,
    prefix: &str,
) -> Option<(MessageDescriptor, KeyCompletionSpan)> {
    let bytes = prefix.as_bytes();
    let mut stack: Vec<Option<MessageDescriptor>> = Vec::new();
    // `None` = inside an object whose schema is unknown (e.g. a map literal).
    let mut current: Option<MessageDescriptor> = Some(root.clone());
    let mut last_key: Option<String> = None;
    // True right after a key's `:` — the next string is a value, not a key.
    let mut expecting_value = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                // Capture the string span, then decide if it is a key.
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        break;
                    }
                    i += 1;
                }
                if i >= bytes.len() {
                    // Unterminated string at the cursor. Right after a key's
                    // `:` it is a value being typed — no key suggestions.
                    if expecting_value {
                        return None;
                    }
                    let current = current?;
                    return Some((
                        current,
                        KeyCompletionSpan {
                            replace_start: start + 1,
                            replace_end: prefix.len(),
                            needs_quote: false,
                        },
                    ));
                }
                let content = &prefix[start + 1..i];
                i += 1;
                let mut j = i;
                while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b':' {
                    last_key = Some(content.to_string());
                    expecting_value = true;
                } else if i == bytes.len() && !expecting_value {
                    // String closed exactly at the cursor (e.g. auto-paired
                    // quotes) while a key is expected here.
                    let current = current?;
                    return Some((
                        current,
                        KeyCompletionSpan {
                            replace_start: start + 1,
                            replace_end: i,
                            needs_quote: false,
                        },
                    ));
                } else {
                    expecting_value = false;
                }
                // The capture loop already positioned `i` past the closing
                // quote; skip the loop's trailing increment.
                continue;
            }
            b'{' => {
                // A keyed `{` descends into that field's message (None for
                // maps/scalars = unknown scope); an unkeyed `{` (the root
                // object, or a bare object in an array) keeps the scope.
                let new_current = match &last_key {
                    Some(key) => current
                        .as_ref()
                        .and_then(|c| find_field(c, key))
                        .and_then(|f| field_as_message(&f)),
                    None => current.clone(),
                };
                stack.push(current.take());
                current = new_current;
                last_key = None;
                // Whatever precedes the brace, the next token is a key.
                expecting_value = false;
            }
            b'}' => {
                current = stack.pop().flatten();
                last_key = None;
                expecting_value = false;
            }
            b',' => {
                // A comma ends the current member/element: the next string
                // is a key again inside the enclosing object — including
                // after scalar (number/bool/null) values, which the string
                // branch above cannot reset.
                expecting_value = false;
            }
            _ => {}
        }
        i += 1;
    }
    let current = current?;
    // No string under the cursor: suggestions still apply while an
    // identifier is being typed (or the cursor sits where a key may start)
    // right after `{` or `,` — the inserted name is then wrapped in quotes.
    // Value positions (right after `":` or a completed value) yield nothing.
    let ident_len: usize = prefix
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .map(char::len_utf8)
        .sum();
    let before_ident = &prefix[..prefix.len() - ident_len];
    let opener = before_ident
        .bytes()
        .rev()
        .find(|b| !b.is_ascii_whitespace());
    let key_position = matches!(opener, Some(b'{') | Some(b','));
    if !key_position {
        return None;
    }
    Some((
        current,
        KeyCompletionSpan {
            replace_start: prefix.len() - ident_len,
            replace_end: prefix.len(),
            needs_quote: true,
        },
    ))
}
