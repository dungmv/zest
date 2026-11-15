use super::*;

#[test]
fn test_lex_names_punct_spread() {
    let tokens = lex("query { user(... on Type) }");
    let kinds: Vec<String> = tokens
        .iter()
        .map(|t| match &t.tok {
            Tok::Name(n) => format!("N:{n}"),
            Tok::Punct(c) => format!("P:{c}"),
            Tok::Spread => "S".to_string(),
            Tok::Str => "STR".to_string(),
            Tok::Num => "#".to_string(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["N:query", "P:{", "N:user", "P:(", "S", "N:on", "N:Type", "P:)", "P:}",]
    );
}

#[test]
fn test_strings_and_comments_swallow_braces() {
    let tokens = lex("query { f(x: \"}\") # comment { with brace\n g }");
    let names: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match &t.tok {
            Tok::Name(n) => Some(n.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["query", "f", "x", "g"]);
    // Exactly one string token spanning the quoted brace.
    assert_eq!(tokens.iter().filter(|t| t.tok == Tok::Str).count(), 1);
}

#[test]
fn test_block_string_spanning_lines() {
    let tokens = lex("query { doc(md: \"\"\"a\n\"brace\" here\"\"\") }");
    assert_eq!(tokens.iter().filter(|t| t.tok == Tok::Str).count(), 1);
}

#[test]
fn test_unterminated_string_runs_to_end() {
    let tokens = lex("query { f(x: \"open brace { {");
    assert_eq!(tokens.iter().filter(|t| t.tok == Tok::Str).count(), 1);
}

#[test]
fn test_offsets_are_byte_accurate_with_utf8() {
    let text = "query { f(n: \"tiếng\") }";
    let tokens = lex(text);
    let str_tok = tokens.iter().find(|t| t.tok == Tok::Str).unwrap();
    assert_eq!(&text[str_tok.start..str_tok.end], "\"tiếng\"");
}

#[test]
fn test_numbers_and_escapes() {
    let tokens = lex("f(a: -1.5e3, b: \"a\\\"b\", c: 42)");
    assert_eq!(tokens.iter().filter(|t| t.tok == Tok::Num).count(), 2);
    // Escaped quote inside the string must not terminate it early.
    let str_tok = tokens.iter().find(|t| t.tok == Tok::Str).unwrap();
    assert_eq!(str_tok.end - str_tok.start, 6, "\"a\\\"b\" is 6 bytes");
}
