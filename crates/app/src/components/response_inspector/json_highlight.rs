pub fn highlight_json_line(line: &str, t: &crate::theme::AppTheme) -> gpui_kit::StyledText {
    let mut highlights = Vec::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        if b == b'"' {
            let start = i;
            i += 1;
            while i < len {
                let ch = line[i..].chars().next().unwrap();
                let ch_len = ch.len_utf8();
                if ch == '\\' {
                    i += ch_len;
                    if i < len {
                        let next_ch = line[i..].chars().next().unwrap();
                        i += next_ch.len_utf8();
                    }
                } else if ch == '"' {
                    i += ch_len;
                    break;
                } else {
                    i += ch_len;
                }
            }
            let end = i;

            // Check if followed by colon (ignoring spaces) -> JSON object key
            let mut j = end;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let is_key = j < len && bytes[j] == b':';
            let color = if is_key { t.json_key } else { t.json_string };
            highlights.push((
                start..end,
                gpui_kit::HighlightStyle {
                    color: Some(color),
                    ..Default::default()
                },
            ));
        } else if matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',') {
            highlights.push((
                i..i + 1,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_punctuation),
                    ..Default::default()
                },
            ));
            i += 1;
        } else if b == b'-' || b.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < len
                && (bytes[i].is_ascii_digit()
                    || matches!(bytes[i], b'.' | b'e' | b'E' | b'+' | b'-'))
            {
                i += 1;
            }
            highlights.push((
                start..i,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_number),
                    ..Default::default()
                },
            ));
        } else if line[i..].starts_with("true")
            && (i + 4 == len || !bytes[i + 4].is_ascii_alphanumeric())
        {
            highlights.push((
                i..i + 4,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_boolean),
                    font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ));
            i += 4;
        } else if line[i..].starts_with("false")
            && (i + 5 == len || !bytes[i + 5].is_ascii_alphanumeric())
        {
            highlights.push((
                i..i + 5,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_boolean),
                    font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ));
            i += 5;
        } else if line[i..].starts_with("null")
            && (i + 4 == len || !bytes[i + 4].is_ascii_alphanumeric())
        {
            highlights.push((
                i..i + 4,
                gpui_kit::HighlightStyle {
                    color: Some(t.json_null),
                    font_style: Some(gpui_kit::FontStyle::Italic),
                    ..Default::default()
                },
            ));
            i += 4;
        } else {
            let ch = line[i..].chars().next().unwrap();
            i += ch.len_utf8();
        }
    }

    gpui_kit::StyledText::new(line.to_string()).with_highlights(highlights)
}
