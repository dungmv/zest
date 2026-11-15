//! Minimal GraphQL tokenizer for the completion resolver. ASCII name and
//! punctuation scanning is byte-based (safe: UTF-8 continuation bytes never
//! collide with ASCII); strings, block strings and comments are skipped so
//! braces inside them never affect the context walk.

/// Token kind produced by [`lex`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// `[_A-Za-z][_0-9A-Za-z]*` — keywords, field/arg/type names.
    Name(String),
    /// A single punctuation character.
    Punct(char),
    /// The `...` spread ellipsis.
    Spread,
    /// A single- or block-string literal (content discarded).
    Str,
    /// An int/float literal.
    Num,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub tok: Tok,
    /// Byte offset of the first byte of the token.
    pub start: usize,
    /// Byte offset one past the last byte of the token.
    pub end: usize,
}

/// Tokenizes a GraphQL document. Never fails: malformed input yields
/// conservative tokens so the resolver can simply stop suggesting.
pub fn lex(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    // Skip a UTF-8 BOM at the very start of the document.
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }
    while i < bytes.len() {
        let start = i;
        match bytes[i] {
            b' ' | b'\t' | b'\r' | b'\n' | b',' => {
                i += 1;
            }
            b'#' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                i = skip_string(bytes, i);
                tokens.push(Token {
                    tok: Tok::Str,
                    start,
                    end: i,
                });
            }
            b'.' => {
                if bytes[i..].starts_with(b"...") {
                    i += 3;
                    tokens.push(Token {
                        tok: Tok::Spread,
                        start,
                        end: i,
                    });
                } else {
                    i += 1;
                    tokens.push(Token {
                        tok: Tok::Punct('.'),
                        start,
                        end: i,
                    });
                }
            }
            c if is_name_start(c) => {
                while i < bytes.len() && is_name_continue(bytes[i]) {
                    i += 1;
                }
                tokens.push(Token {
                    tok: Tok::Name(text[start..i].to_string()),
                    start,
                    end: i,
                });
            }
            c if c.is_ascii_digit() || c == b'-' => {
                i = skip_number(bytes, i);
                tokens.push(Token {
                    tok: Tok::Num,
                    start,
                    end: i,
                });
            }
            _ => {
                // Punctuation is ASCII in GraphQL; step one full char for
                // safety against arbitrary bytes.
                let step = text[start..].chars().next().map_or(1, char::len_utf8);
                i += step;
                tokens.push(Token {
                    tok: Tok::Punct(text[start..i].chars().next().unwrap_or('?')),
                    start,
                    end: i,
                });
            }
        }
    }
    tokens
}

fn is_name_start(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphabetic()
}

fn is_name_continue(c: u8) -> bool {
    is_name_start(c) || c.is_ascii_digit()
}

/// Skips a `"""block"""` or `"single"` string starting at `start == b'"'`.
/// Unterminated strings run to the end of the input.
fn skip_string(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    if bytes[i..].starts_with(b"\"\"\"") {
        i += 3;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"\"\"\"") {
                return i + 3;
            }
            i += 1;
        }
        return i;
    }
    i += 1; // opening quote
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2, // escape consumes the next byte too
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    i
}

fn skip_number(bytes: &[u8], mut i: usize) -> usize {
    if bytes.get(i) == Some(&b'-') {
        i += 1;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if matches!(bytes.get(i), Some(b'e') | Some(b'E')) {
        let mut j = i + 1;
        if matches!(bytes.get(j), Some(b'+') | Some(b'-')) {
            j += 1;
        }
        if bytes.get(j).is_some_and(|c| c.is_ascii_digit()) {
            i = j;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    i
}

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod tests;
