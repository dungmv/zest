//! JSON with comments and trailing commas, as used by the HTTP body editor.

use std::{borrow::Cow, ops::Range};

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Whitespace,
    Comment,
    UnclosedComment,
    Open,
    Close,
    Comma,
    Colon,
    Value,
}

fn tokens(source: &str) -> impl Iterator<Item = (TokenKind, &str)> {
    let mut rest = source;
    std::iter::from_fn(move || {
        let first = *rest.as_bytes().first()?;
        let (kind, len) = match first {
            b' ' | b'\t' | b'\r' | b'\n' => {
                (TokenKind::Whitespace, prefix_len(rest, is_json_whitespace))
            }
            b'/' if rest.starts_with("//") => (
                TokenKind::Comment,
                rest.find(['\r', '\n']).unwrap_or(rest.len()),
            ),
            b'/' if rest.starts_with("/*") => match rest[2..].find("*/") {
                Some(end) => (TokenKind::Comment, end + 4),
                None => (TokenKind::UnclosedComment, rest.len()),
            },
            b'"' => (TokenKind::Value, string_len(rest)),
            b'{' | b'[' => (TokenKind::Open, 1),
            b'}' | b']' => (TokenKind::Close, 1),
            b',' => (TokenKind::Comma, 1),
            b':' => (TokenKind::Colon, 1),
            b'/' => (TokenKind::Value, 1),
            _ => (
                TokenKind::Value,
                prefix_len(rest, |byte| {
                    !is_json_whitespace(byte)
                        && !matches!(byte, b'{' | b'}' | b'[' | b']' | b',' | b':' | b'"' | b'/')
                }),
            ),
        };
        let (token, remaining) = rest.split_at(len);
        rest = remaining;
        Some((kind, token))
    })
}

fn is_json_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn prefix_len(source: &str, predicate: impl Fn(u8) -> bool) -> usize {
    source.bytes().take_while(|byte| predicate(*byte)).count()
}

fn string_len(source: &str) -> usize {
    let mut bytes = source.bytes().enumerate().skip(1);
    while let Some((index, byte)) = bytes.next() {
        match byte {
            b'\\' => {
                bytes.next();
            }
            b'"' => return index + 1,
            _ => {}
        }
    }
    source.len()
}

/// Finds comments for syntax highlighting, including a block still being typed.
pub fn comment_ranges(source: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let source = if source.contains("//") || source.contains("/*") {
        source
    } else {
        ""
    };
    let mut offset = 0;
    tokens(source).filter_map(move |(kind, token)| {
        let start = offset;
        offset += token.len();
        matches!(kind, TokenKind::Comment | TokenKind::UnclosedComment).then_some(start..offset)
    })
}

/// Removes JSONC syntax while preserving strings, key order, and number spellings.
/// Comments become whitespace so adjacent tokens cannot accidentally merge.
pub fn to_json(source: &str) -> Cow<'_, str> {
    let mut offset = 0;
    let mut previous = None;
    let mut pending_comma = None;
    let mut output: Option<String> = None;
    for (kind, token) in tokens(source) {
        let start = offset;
        offset += token.len();
        match kind {
            TokenKind::Comment => {
                let json = output.get_or_insert_with(|| {
                    let mut json = String::with_capacity(source.len());
                    json.push_str(&source[..start]);
                    json
                });
                json.push(' ');
                json.extend(token.chars().filter(|ch| matches!(ch, '\r' | '\n')));
            }
            _ => {
                if kind != TokenKind::Whitespace {
                    if let Some(comma) = pending_comma.take().filter(|_| kind == TokenKind::Close) {
                        let json = output.get_or_insert_with(|| source[..start].to_owned());
                        json.remove(comma);
                    }
                    if kind == TokenKind::Comma
                        && matches!(previous, Some(TokenKind::Value | TokenKind::Close))
                    {
                        pending_comma = Some(output.as_ref().map_or(start, String::len));
                    }
                    previous = Some(kind);
                }
                if let Some(json) = &mut output {
                    json.push_str(token);
                }
            }
        }
    }
    output.map_or(Cow::Borrowed(source), Cow::Owned)
}

/// Formats a JSON body while retaining comments and trailing commas in the editor.
pub fn format(source: &str) -> serde_json::Result<String> {
    let json = to_json(source);
    let value = serde_json::from_str::<serde_json::Value>(&json)?;
    if matches!(json, Cow::Borrowed(_)) {
        return serde_json::to_string_pretty(&value);
    }

    let mut formatted = String::with_capacity(source.len());
    let mut depth = 0;
    let mut previous = None;
    let mut starts_line = true;
    for (kind, token) in tokens(source) {
        if kind == TokenKind::Whitespace {
            starts_line |= token.contains(['\r', '\n']);
            continue;
        }
        if kind == TokenKind::Close {
            depth -= 1;
        }
        match previous {
            Some(TokenKind::Open) if kind == TokenKind::Close => {}
            Some(TokenKind::Comma | TokenKind::Close | TokenKind::Value)
                if kind == TokenKind::Comment && !starts_line =>
            {
                formatted.push(' ');
            }
            Some(TokenKind::Open | TokenKind::Comma | TokenKind::Comment) => {
                newline(&mut formatted, depth);
            }
            Some(_) if matches!(kind, TokenKind::Close | TokenKind::Comment) => {
                newline(&mut formatted, depth);
            }
            Some(TokenKind::Colon) => formatted.push(' '),
            _ => {}
        }
        formatted.push_str(token);
        if kind == TokenKind::Open {
            depth += 1;
        }
        previous = Some(kind);
        starts_line = false;
    }
    Ok(formatted)
}

fn newline(output: &mut String, depth: usize) {
    output.push('\n');
    for _ in 0..depth {
        output.push_str("  ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_jsonc_without_rewriting_strings_or_numbers() {
        let source = r#"{
            // "disabled": true,
            "url": "https://example.com/ñ/*path*/",
            "escaped": "\"//still a string",
            "nested": [1.20e3, /* comment after comma */],
        }"#;
        let json = to_json(source);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "url": "https://example.com/ñ/*path*/",
                "escaped": "\"//still a string",
                "nested": [1200.0],
            })
        );
        assert!(json.contains("1.20e3"));
    }

    #[test]
    fn formats_without_losing_disabled_fields_or_inline_notes() {
        let source = "{\n// \"disabled\": true,\n\"items\":[1,], // keep this note\n}";
        let formatted = format(source).unwrap();
        assert_eq!(
            formatted,
            "{\n  // \"disabled\": true,\n  \"items\": [\n    1,\n  ], // keep this note\n}"
        );
        assert_eq!(format(&formatted).unwrap(), formatted);
    }

    #[test]
    fn leaves_plain_json_untouched_and_rejects_invalid_jsonc_when_formatting() {
        assert!(matches!(
            to_json(r#" { "url": "https://example.com" } "#),
            Cow::Borrowed(_)
        ));
        for source in ["{,}", "[1,,]", "[1/* comment */2]", "{} /* not closed"] {
            assert!(format(source).is_err(), "{source}");
        }
    }
}
