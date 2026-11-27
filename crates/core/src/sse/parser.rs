//! Incremental SSE parser (`SseParser`) implementing the WHATWG
//! server-sent-events stream algorithm: fields `event`, `data`, `id`,
//! `retry`, `:` comment lines, CRLF/CR/LF line terminators and a leading
//! UTF-8 BOM.

/// One parsed SSE event ready for the message log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// Custom event name, or `None` for the default `message` event.
    pub event: Option<String>,
    /// Data payload — multi-line data joined with `\n`.
    pub data: String,
    /// Last seen `id:` field (persists across events per the spec).
    pub id: Option<String>,
    /// Last seen `retry:` field in milliseconds (persists across events).
    pub retry: Option<u64>,
}

/// One parse output: either a dispatched event or a comment line (usually a
/// keep-alive, rendered as a ping in the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseOutput {
    Event(SseEvent),
    Comment(String),
}

use crate::transport::MAX_REALTIME_FRAME_BYTES;

#[derive(Debug, Default)]
pub struct SseParser {
    /// Raw undecoded bytes between line terminators (may split UTF-8 chars).
    raw: Vec<u8>,
    event_type: Option<String>,
    data: String,
    last_id: Option<String>,
    retry: Option<u64>,
    saw_first_byte: bool,
    limit_exceeded: bool,
    trailing_cr: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn limit_exceeded(&self) -> bool {
        self.limit_exceeded
    }

    /// Feeds one network chunk and returns everything the chunk completed.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseOutput> {
        let mut out = Vec::new();
        // Strip a UTF-8 BOM at the very start of the stream.
        self.limit_exceeded = false;
        let mut chunk = chunk;
        if self.trailing_cr && !chunk.is_empty() {
            self.trailing_cr = false;
            if let Some(rest) = chunk.strip_prefix(b"\n") {
                chunk = rest;
            }
        }
        if !self.saw_first_byte {
            self.saw_first_byte = true;
            if chunk.starts_with(&[0xEF, 0xBB, 0xBF]) {
                chunk = &chunk[3..];
            }
        }
        while !chunk.is_empty() && !self.limit_exceeded {
            let (len, separator) = find_line_end(chunk).unwrap_or((chunk.len(), 0));
            if len > MAX_REALTIME_FRAME_BYTES.saturating_sub(self.raw.len()) {
                self.limit_exceeded = true;
                break;
            }
            self.raw.extend_from_slice(&chunk[..len]);
            self.trailing_cr = separator == 1 && chunk.get(len..) == Some(b"\r");
            chunk = &chunk[len + separator..];
            if separator != 0 {
                let mut raw = std::mem::take(&mut self.raw);
                self.process_line(&String::from_utf8_lossy(&raw), &mut out);
                raw.clear();
                self.raw = raw;
            }
        }
        if self.limit_exceeded {
            self.raw.clear();
            self.data.clear();
        }
        out
    }

    /// Flushes the stream end: any trailing partial line is processed and a
    /// pending event is dispatched.
    pub fn finish(&mut self) -> Vec<SseOutput> {
        let mut out = Vec::new();
        if self.limit_exceeded {
            return out;
        }
        if !self.raw.is_empty() {
            let line = String::from_utf8_lossy(&self.raw).into_owned();
            self.raw.clear();
            self.process_line(&line, &mut out);
        }
        if !self.limit_exceeded {
            self.dispatch(&mut out);
        }
        out
    }

    fn process_line(&mut self, line: &str, out: &mut Vec<SseOutput>) {
        if line.len() > MAX_REALTIME_FRAME_BYTES {
            self.limit_exceeded = true;
            return;
        }
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if let Some(comment) = line.strip_prefix(':') {
            if !comment.trim().is_empty() {
                out.push(SseOutput::Comment(comment.trim().to_string()));
            }
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event_type = Some(value.to_string()),
            "data" => {
                if value.len() < MAX_REALTIME_FRAME_BYTES.saturating_sub(self.data.len()) {
                    self.data.push_str(value);
                    self.data.push('\n');
                } else {
                    self.limit_exceeded = true;
                }
            }
            "id" => {
                if !value.contains('\0') {
                    self.last_id = Some(value.to_string());
                }
            }
            "retry" => {
                if let Ok(ms) = value.parse::<u64>() {
                    self.retry = Some(ms);
                }
            }
            _ => {} // unknown fields ignored per spec
        }
    }

    fn dispatch(&mut self, out: &mut Vec<SseOutput>) {
        if self.data.is_empty() {
            return;
        }
        // Per spec the data buffer carries one trailing newline per `data:`
        // field; remove the last one before dispatching.
        if self.data.ends_with('\n') {
            self.data.pop();
        }
        let data = std::mem::take(&mut self.data);
        out.push(SseOutput::Event(SseEvent {
            event: self.event_type.take(),
            data,
            id: self.last_id.clone(),
            retry: self.retry,
        }));
    }
}

/// Finds the earliest line terminator. Returns `(bytes_before_terminator,
/// terminator_length)` so `\r\n` counts as one separator.
fn find_line_end(raw: &[u8]) -> Option<(usize, usize)> {
    for (idx, b) in raw.iter().enumerate() {
        match b {
            b'\n' => return Some((idx, 1)),
            b'\r' => {
                let len = if raw.get(idx + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                return Some((idx, len));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
