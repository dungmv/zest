use super::{clamp_char_boundary_left, clamp_char_boundary_right};
use crate::editor_scheme::editor_scheme;
use gpui_kit::component::input::{
    EditorState as CodeEditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    Rope,
};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use std::ops::Range;

pub struct XmlHighlighter {
    pub text: Rope,
    pub cached_str: String,
}

impl XmlHighlighter {
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            cached_str: String::new(),
        }
    }
}

impl Default for XmlHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl InputHighlighter for XmlHighlighter {
    fn language(&self) -> SharedString {
        "xml".into()
    }

    fn update(
        &mut self,
        _edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut Window,
        _cx: &mut Context<CodeEditorState>,
    ) {
        self.text = text.clone();
        self.cached_str = text.to_string();
    }

    fn styles(
        &self,
        range: &Range<usize>,
        _resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let temp_str;
        let full_str: &str = if !self.cached_str.is_empty() {
            &self.cached_str
        } else if self.text.len() != 0 {
            temp_str = self.text.to_string();
            &temp_str
        } else {
            ""
        };
        if full_str.is_empty() || range.is_empty() {
            return vec![(range.clone(), HighlightStyle::default())];
        }

        let start = clamp_char_boundary_left(full_str, range.start);
        let end = clamp_char_boundary_right(full_str, range.end.min(full_str.len()));

        if start >= end {
            return vec![(range.clone(), HighlightStyle::default())];
        }

        let slice_str = &full_str[start..end];
        let slice_bytes = slice_str.as_bytes();
        let len = slice_bytes.len();
        let mut i = 0;
        let mut raw_tokens = Vec::new();
        let scheme = editor_scheme();

        while i < len {
            // Check comment <!-- ... -->
            if i + 3 < len && &slice_bytes[i..i + 4] == b"<!--" {
                let c_start = i;
                i += 4;
                while i + 2 < len && &slice_bytes[i..i + 3] != b"-->" {
                    i += 1;
                }
                if i + 2 < len {
                    i += 3;
                }
                raw_tokens.push((
                    start + c_start..start + i,
                    HighlightStyle {
                        color: Some(scheme.comment),
                        font_style: Some(gpui_kit::FontStyle::Italic),
                        ..Default::default()
                    },
                ));
            }
            // Check CDATA <![CDATA[ ... ]]>
            else if i + 8 < len && &slice_bytes[i..i + 9] == b"<![CDATA[" {
                let cd_start = i;
                i += 9;
                while i + 2 < len && &slice_bytes[i..i + 3] != b"]]>" {
                    i += 1;
                }
                if i + 2 < len {
                    i += 3;
                }
                raw_tokens.push((
                    start + cd_start..start + i,
                    HighlightStyle {
                        color: Some(scheme.number),
                        ..Default::default()
                    },
                ));
            }
            // Check XML Declaration / Processing instruction <? ... ?>
            else if i + 1 < len && &slice_bytes[i..i + 2] == b"<?" {
                let d_start = i;
                i += 2;
                while i + 1 < len && &slice_bytes[i..i + 2] != b"?>" {
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
                raw_tokens.push((
                    start + d_start..start + i,
                    HighlightStyle {
                        color: Some(scheme.null),
                        font_style: Some(gpui_kit::FontStyle::Italic),
                        ..Default::default()
                    },
                ));
            }
            // Check Tags: <tag ...> or </tag>
            else if slice_bytes[i] == b'<' {
                let tag_start = i;
                let is_closing = i + 1 < len && slice_bytes[i + 1] == b'/';
                let tag_bracket_len = if is_closing { 2 } else { 1 };
                raw_tokens.push((
                    start + tag_start..start + tag_start + tag_bracket_len,
                    HighlightStyle {
                        color: Some(scheme.punctuation),
                        ..Default::default()
                    },
                ));
                i += tag_bracket_len;

                // Tag name
                let name_start = i;
                while i < len
                    && (slice_bytes[i].is_ascii_alphanumeric()
                        || slice_bytes[i] == b':'
                        || slice_bytes[i] == b'_'
                        || slice_bytes[i] == b'-')
                {
                    i += 1;
                }
                if i > name_start {
                    raw_tokens.push((
                        start + name_start..start + i,
                        HighlightStyle {
                            color: Some(scheme.accent),
                            font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                            ..Default::default()
                        },
                    ));
                }

                // Attributes until '>' or '/>'
                while i < len && slice_bytes[i] != b'>' {
                    if slice_bytes[i] == b'/' && i + 1 < len && slice_bytes[i + 1] == b'>' {
                        raw_tokens.push((
                            start + i..start + i + 2,
                            HighlightStyle {
                                color: Some(scheme.punctuation),
                                ..Default::default()
                            },
                        ));
                        i += 2;
                        break;
                    }

                    // Attribute name
                    if slice_bytes[i].is_ascii_alphabetic()
                        || slice_bytes[i] == b'_'
                        || slice_bytes[i] == b':'
                    {
                        let attr_start = i;
                        while i < len
                            && (slice_bytes[i].is_ascii_alphanumeric()
                                || slice_bytes[i] == b'_'
                                || slice_bytes[i] == b':'
                                || slice_bytes[i] == b'-')
                        {
                            i += 1;
                        }
                        raw_tokens.push((
                            start + attr_start..start + i,
                            HighlightStyle {
                                color: Some(scheme.key),
                                ..Default::default()
                            },
                        ));
                        continue;
                    }

                    // '='
                    if slice_bytes[i] == b'=' {
                        raw_tokens.push((
                            start + i..start + i + 1,
                            HighlightStyle {
                                color: Some(scheme.punctuation),
                                ..Default::default()
                            },
                        ));
                        i += 1;
                        continue;
                    }

                    // Attribute string value: "..." or '...'
                    if slice_bytes[i] == b'"' || slice_bytes[i] == b'\'' {
                        let quote = slice_bytes[i];
                        let val_start = i;
                        i += 1;
                        while i < len && slice_bytes[i] != quote && slice_bytes[i] != b'\n' {
                            i += 1;
                        }
                        if i < len && slice_bytes[i] == quote {
                            i += 1;
                        }
                        raw_tokens.push((
                            start + val_start..start + i,
                            HighlightStyle {
                                color: Some(scheme.string),
                                ..Default::default()
                            },
                        ));
                        continue;
                    }

                    i += 1;
                }

                if i < len && slice_bytes[i] == b'>' {
                    raw_tokens.push((
                        start + i..start + i + 1,
                        HighlightStyle {
                            color: Some(scheme.punctuation),
                            ..Default::default()
                        },
                    ));
                    i += 1;
                }
            } else {
                let ch = slice_str[i..].chars().next().unwrap();
                i += ch.len_utf8();
            }
        }

        let mut full_styles = Vec::new();
        let mut cursor = range.start;

        for (token_range, style) in raw_tokens {
            if token_range.start > cursor {
                full_styles.push((cursor..token_range.start, HighlightStyle::default()));
            }
            cursor = token_range.end;
            full_styles.push((token_range, style));
        }

        if cursor < range.end {
            full_styles.push((cursor..range.end, HighlightStyle::default()));
        }

        full_styles
    }

    fn fold_ranges(&self, text: &Rope) -> Vec<FoldRange> {
        let mut folds = Vec::new();
        let mut tag_stack: Vec<(String, usize)> = Vec::new();
        let s = text.to_string();
        let bytes = s.as_bytes();
        let len = bytes.len();
        let mut current_line = 0;
        let mut i = 0;

        while i < len {
            if bytes[i] == b'\n' {
                current_line += 1;
                i += 1;
                continue;
            }

            // Check comment <!-- ... -->
            if i + 3 < len && &bytes[i..i + 4] == b"<!--" {
                let comment_start = current_line;
                i += 4;
                while i + 2 < len && &bytes[i..i + 3] != b"-->" {
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    i += 1;
                }
                if i + 2 < len {
                    i += 3;
                }
                if current_line > comment_start {
                    folds.push(FoldRange::new(comment_start, current_line));
                }
                continue;
            }

            // Check CDATA <![CDATA[ ... ]]>
            if i + 8 < len && &bytes[i..i + 9] == b"<![CDATA[" {
                let cdata_start = current_line;
                i += 9;
                while i + 2 < len && &bytes[i..i + 3] != b"]]>" {
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    i += 1;
                }
                if i + 2 < len {
                    i += 3;
                }
                if current_line > cdata_start {
                    folds.push(FoldRange::new(cdata_start, current_line));
                }
                continue;
            }

            // Check XML Declaration / Processing Instruction <? ... ?> or <!DOCTYPE ... >
            if i + 1 < len && (bytes[i] == b'<' && (bytes[i + 1] == b'?' || bytes[i + 1] == b'!')) {
                let decl_start = current_line;
                let is_pi = bytes[i + 1] == b'?';
                i += 2;
                while i < len {
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    if is_pi && i + 1 < len && bytes[i] == b'?' && bytes[i + 1] == b'>' {
                        i += 2;
                        break;
                    } else if !is_pi && bytes[i] == b'>' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                if current_line > decl_start {
                    folds.push(FoldRange::new(decl_start, current_line));
                }
                continue;
            }

            // Check Closing tag: </tag_name>
            if i + 1 < len && bytes[i] == b'<' && bytes[i + 1] == b'/' {
                i += 2;
                let name_start = i;
                while i < len
                    && (bytes[i].is_ascii_alphanumeric()
                        || bytes[i] == b':'
                        || bytes[i] == b'_'
                        || bytes[i] == b'-')
                {
                    i += 1;
                }
                let tag_name = &s[name_start..i];
                while i < len && bytes[i] != b'>' {
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    i += 1;
                }
                if i < len && bytes[i] == b'>' {
                    i += 1;
                }

                if let Some(pos) = tag_stack.iter().rposition(|(name, _)| name == tag_name) {
                    let (_, start_line) = tag_stack.remove(pos);
                    if current_line > start_line {
                        folds.push(FoldRange::new(start_line, current_line));
                    }
                }
                continue;
            }

            // Check Opening tag: <tag_name ...> or <tag_name ... />
            if bytes[i] == b'<'
                && i + 1 < len
                && (bytes[i + 1].is_ascii_alphabetic()
                    || bytes[i + 1] == b'_'
                    || bytes[i + 1] == b':')
            {
                let tag_start_line = current_line;
                i += 1;
                let name_start = i;
                while i < len
                    && (bytes[i].is_ascii_alphanumeric()
                        || bytes[i] == b':'
                        || bytes[i] == b'_'
                        || bytes[i] == b'-')
                {
                    i += 1;
                }
                let tag_name = s[name_start..i].to_string();

                let mut self_closing = false;
                let mut in_quote: Option<u8> = None;

                while i < len {
                    let b = bytes[i];
                    if b == b'\n' {
                        current_line += 1;
                    }
                    if let Some(q) = in_quote {
                        if b == q {
                            in_quote = None;
                        }
                    } else if b == b'"' || b == b'\'' {
                        in_quote = Some(b);
                    } else if b == b'/' && i + 1 < len && bytes[i + 1] == b'>' {
                        self_closing = true;
                        i += 2;
                        break;
                    } else if b == b'>' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }

                if !self_closing {
                    tag_stack.push((tag_name, tag_start_line));
                }
                continue;
            }

            i += 1;
        }

        folds.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| b.end_line.cmp(&a.end_line))
        });
        folds.dedup_by_key(|f| f.start_line);
        folds
    }
}

pub fn format_xml(xml: &str) -> Result<String, String> {
    use quick_xml::reader::Reader;
    use quick_xml::writer::Writer;
    use std::io::Cursor;

    let trimmed = xml.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let mut reader = Reader::from_str(trimmed);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(event) => {
                writer
                    .write_event(event)
                    .map_err(|e| format!("XML write error: {}", e))?;
            }
            Err(e) => return Err(format!("XML syntax error: {}", e)),
        }
    }

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).map_err(|e| format!("UTF-8 conversion error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyResolver;
    impl HighlightStyleResolver for DummyResolver {
        fn style(&self, _: &str) -> Option<HighlightStyle> {
            None
        }
    }

    #[test]
    fn test_format_xml_valid() {
        let raw = "<root><item id=\"1\"><name>Alice</name></item></root>";
        let formatted = format_xml(raw).expect("should format valid XML");
        assert!(formatted.contains("<root>\n"));
        assert!(formatted.contains("  <item id=\"1\">\n"));
        assert!(formatted.contains("    <name>Alice</name>\n"));
        assert!(formatted.contains("  </item>\n"));
        assert!(formatted.contains("</root>"));
    }

    #[test]
    fn test_format_xml_with_declaration() {
        let raw = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><root><val>123</val></root>";
        let formatted = format_xml(raw).expect("should format XML with declaration");
        assert!(formatted.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(formatted.contains("<root>\n"));
        assert!(formatted.contains("  <val>123</val>\n"));
    }

    #[test]
    fn test_format_xml_invalid_syntax() {
        let raw = "<root><unclosed>test</root>";
        let res = format_xml(raw);
        assert!(res.is_err(), "Invalid unclosed tag should return error");
    }

    #[test]
    fn test_xml_highlighter_folding() {
        let highlighter = XmlHighlighter::new();
        let xml_text = "<?xml version=\"1.0\"?>\n<root>\n  <item id=\"1\">\n    <name>Widget</name>\n  </item>\n  <item id=\"2\" />\n  <!--\n    Multi line\n    comment\n  -->\n</root>";
        let rope = Rope::from_str(xml_text);
        let folds = highlighter.fold_ranges(&rope);
        assert!(!folds.is_empty(), "Should detect foldable ranges in XML");

        assert!(folds.iter().any(|f| f.start_line == 1 && f.end_line == 10));
        assert!(folds.iter().any(|f| f.start_line == 2 && f.end_line == 4));
        assert!(folds.iter().any(|f| f.start_line == 6 && f.end_line == 9));
        assert!(!folds.iter().any(|f| f.start_line == 3));
        assert!(!folds.iter().any(|f| f.start_line == 5));
    }

    #[test]
    fn test_xml_highlighter_styles() {
        let mut highlighter = XmlHighlighter::new();
        let xml_text = "<root id=\"101\">{{custom_var}}</root>";
        highlighter.text = Rope::from_str(xml_text);
        highlighter.cached_str = xml_text.to_string();

        let range = 0..xml_text.len();
        let styles = highlighter.styles(&range, &DummyResolver);
        assert!(!styles.is_empty(), "Styles should tokenize XML tags");

        let scheme = editor_scheme();
        let has_tag_highlight = styles
            .iter()
            .any(|(r, s)| &xml_text[r.clone()] == "root" && s.color == Some(scheme.accent));
        let has_attr_value_highlight = styles
            .iter()
            .any(|(r, s)| &xml_text[r.clone()] == "\"101\"" && s.color == Some(scheme.string));
        assert!(has_tag_highlight, "Should highlight XML tag name");
        assert!(
            has_attr_value_highlight,
            "Should highlight XML attribute value"
        );
    }
}
