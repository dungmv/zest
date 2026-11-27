use super::*;

fn feed_str(p: &mut SseParser, s: &str) -> Vec<SseOutput> {
    p.feed(s.as_bytes())
}

fn event_data(out: Vec<SseOutput>) -> Vec<SseEvent> {
    out.into_iter()
        .filter_map(|o| match o {
            SseOutput::Event(e) => Some(e),
            SseOutput::Comment(_) => None,
        })
        .collect()
}

#[test]
fn test_basic_message_event() {
    let mut p = SseParser::new();
    let events = event_data(feed_str(&mut p, "data: hello\n\n"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hello");
    assert_eq!(events[0].event, None, "default event type is message");
    assert_eq!(events[0].id, None);
}

#[test]
fn test_openai_style_stream_chunks_split_across_feeds() {
    let mut p = SseParser::new();
    let mut events = Vec::new();
    events.extend(event_data(feed_str(&mut p, "data: {\"to")));
    assert!(events.is_empty(), "no dispatch before blank line");
    events.extend(event_data(feed_str(
        &mut p,
        "kens\": 2}\n\ndata: {\"delta\": \"Hi\"}\n\n",
    )));
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].data, "{\"tokens\": 2}");
    assert_eq!(events[1].data, "{\"delta\": \"Hi\"}");
}

#[test]
fn test_multiline_data_joined_with_newline() {
    let mut p = SseParser::new();
    let events = event_data(feed_str(&mut p, "data: line1\ndata: line2\ndata:\n\n"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "line1\nline2\n");
}

#[test]
fn test_named_event_and_id_and_retry() {
    let mut p = SseParser::new();
    let events = event_data(feed_str(
        &mut p,
        "event: add\ndata: 1\nid: 42\nretry: 3000\n\n",
    ));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("add"));
    assert_eq!(events[0].data, "1");
    assert_eq!(events[0].id.as_deref(), Some("42"));
    assert_eq!(events[0].retry, Some(3000));
}

#[test]
fn test_id_and_retry_persist_across_events() {
    let mut p = SseParser::new();
    feed_str(&mut p, "id: a\nretry: 100\ndata: one\n\n");
    let events = event_data(feed_str(&mut p, "data: two\n\n"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id.as_deref(), Some("a"), "last id persists");
    assert_eq!(events[0].retry, Some(100), "last retry persists");
}

#[test]
fn test_crlf_and_bare_cr_terminators() {
    let mut p = SseParser::new();
    let events = event_data(feed_str(&mut p, "data: a\r\ndata: b\rdata: c\n\n"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "a\nb\nc");
}

#[test]
fn test_bom_stripped_once() {
    let mut p = SseParser::new();
    let events = event_data(p.feed(&[0xEF, 0xBB, 0xBF]));
    assert!(events.is_empty());
    let events = event_data(feed_str(&mut p, "data: ok\n\n"));
    assert_eq!(events[0].data, "ok");
}

#[test]
fn test_comments_surfaced_and_ignored_as_fields() {
    let mut p = SseParser::new();
    let out = feed_str(&mut p, ": keep-alive\ndata: x\n: \n\n");
    let mut comments = Vec::new();
    let mut events = Vec::new();
    for o in out {
        match o {
            SseOutput::Comment(c) => comments.push(c),
            SseOutput::Event(e) => events.push(e),
        }
    }
    assert_eq!(comments, vec!["keep-alive".to_string()]);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "x");
}

#[test]
fn test_empty_data_line_does_not_dispatch() {
    let mut p = SseParser::new();
    let out = feed_str(&mut p, "\n\n\nevent: only\n\n");
    assert!(out.is_empty(), "no data means no event");
    assert!(p.finish().is_empty());
}

#[test]
fn test_unknown_and_malformed_fields_ignored() {
    let mut p = SseParser::new();
    let events = event_data(feed_str(
        &mut p,
        "unknown: v\ndata:x\nretry: fast\nid: bad\0id\ndata: y\n\n",
    ));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "x\ny");
    assert_eq!(events[0].retry, None, "non-numeric retry ignored");
    assert_eq!(events[0].id, None, "id with NUL ignored");
}

#[test]
fn test_finish_flushes_trailing_line_and_pending_event() {
    let mut p = SseParser::new();
    feed_str(&mut p, "data: incomplete");
    let events = event_data(p.finish());
    assert_eq!(events.len(), 1, "EOF dispatches a pending event");
    assert_eq!(events[0].data, "incomplete");
}

#[test]
fn test_utf8_multibyte_split_across_chunks() {
    let mut p = SseParser::new();
    let full = "data: tiếng Việt ✓\n\n".as_bytes().to_vec();
    let (a, b) = full.split_at(full.len() - 3);
    let mut events = event_data(p.feed(a));
    events.extend(event_data(p.feed(b)));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "tiếng Việt ✓");
}

#[test]
fn test_runaway_line_is_dropped_not_grown() {
    let mut p = SseParser::new();
    let big = vec![b'x'; 5 * 1024 * 1024];
    let out = p.feed(&big);
    assert!(out.is_empty());
    let events = event_data(feed_str(&mut p, "\ndata: after\n\n"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "after", "parser recovers after the cap");
}

#[test]
fn test_multiline_event_respects_byte_limit() {
    let mut parser = SseParser::new();
    let line = format!("data: {}\n", "x".repeat(1024));
    for _ in 0..MAX_REALTIME_FRAME_BYTES / 1024 + 1 {
        assert!(parser.feed(line.as_bytes()).is_empty());
        if parser.limit_exceeded() {
            break;
        }
    }
    assert!(parser.limit_exceeded());
    assert!(parser.finish().is_empty());
}

#[test]
fn test_crlf_split_across_chunks_does_not_dispatch_early() {
    let mut parser = SseParser::new();
    assert!(feed_str(&mut parser, "data: one\r").is_empty());
    assert!(feed_str(&mut parser, "\ndata: two\r").is_empty());
    let events = event_data(feed_str(&mut parser, "\n\r\n"));
    assert_eq!(events[0].data, "one\ntwo");
}
