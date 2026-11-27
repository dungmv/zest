use super::{extract_host_key, HttpProgress, ReqwestExecutor};
use crate::models::HttpResponseRecord;
use crate::sse::{SseOutput, SseParser};
use crate::transport::{RequestError, ResolvedRequest, MAX_REALTIME_FRAME_BYTES};
use chrono::Utc;
use futures_util::StreamExt;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc::Sender, oneshot};

/// Body retained in the final record after streaming.
const MAX_STREAMED_BODY_CAPTURE: usize = 2 * 1024 * 1024;

impl ReqwestExecutor {
    /// Executes a request as a server-sent-events stream: every parsed
    /// event or keep-alive comment is forwarded over `sse_tx` as it
    /// arrives, and the final record carries the (capped) accumulated body
    /// so the usual response views and cache still apply. `cancel_rx`
    /// aborts both the connect and the streaming phase. `timeout_ms` acts
    /// as an idle timeout between chunks — a stream that goes silent ends
    /// gracefully with whatever was received. Responses whose content type
    /// is not `text/event-stream` fall back to plain accumulation.
    pub async fn send_streaming(
        &self,
        req: ResolvedRequest,
        progress_tx: Option<Sender<HttpProgress>>,
        sse_tx: Sender<SseOutput>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> Result<HttpResponseRecord, RequestError> {
        if let Some(tx) = &progress_tx {
            let _ = tx.try_send(HttpProgress::Connecting);
        }

        let start_time = Instant::now();
        let request_builder = self.build_request_builder(&req, true)?;

        if let Some(tx) = &progress_tx {
            let _ = tx.try_send(HttpProgress::Sending);
        }

        let host_key = extract_host_key(&req.url);
        let is_reused = Self::check_host_reuse(&self.active_hosts, host_key.as_deref());

        let ttfb_start = Instant::now();
        let mut cancel_rx = cancel_rx;
        let response = tokio::select! {
            _ = &mut cancel_rx => return Err(RequestError::Cancelled),
            res = request_builder.send() => res.map_err(|e| {
                if e.is_timeout() {
                    RequestError::Timeout(req.settings.timeout_secs())
                } else {
                    RequestError::Network(e.to_string())
                }
            })?,
        };
        let ttfb_ms = ttfb_start.elapsed().as_millis() as u64;

        if let Some(tx) = &progress_tx {
            let _ = tx.try_send(HttpProgress::WaitingResponse);
        }

        let status = response.status();
        let status_code = status.as_u16();
        let status_text = status.canonical_reason().unwrap_or("").to_string();

        let mut resp_headers = Vec::new();
        for (name, val) in response.headers() {
            if let Ok(v_str) = val.to_str() {
                resp_headers.push((name.as_str().to_string(), v_str.to_string()));
            }
        }
        let is_event_stream = resp_headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("content-type")
                && v.to_ascii_lowercase().contains("text/event-stream")
        });

        Self::update_host_activity(
            &self.active_hosts,
            host_key.as_deref(),
            resp_headers.iter().any(|(k, v)| {
                k.eq_ignore_ascii_case("connection") && v.eq_ignore_ascii_case("close")
            }),
        );

        let idle = Duration::from_millis(req.settings.timeout_ms);
        let mut parser = SseParser::new();
        let mut body_bytes: Vec<u8> = Vec::new();
        let mut total_received = 0usize;
        let mut stream = response.bytes_stream();
        const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(50);
        let mut last_progress_emit: Option<Instant> = None;

        let download_start = Instant::now();
        loop {
            let next = tokio::select! {
                _ = &mut cancel_rx => return Err(RequestError::Cancelled),
                next = tokio::time::timeout(idle, stream.next()) => match next {
                    // Idle timeout on a silent connection ends the stream.
                    Err(_) => break,
                    Ok(None) => break,
                    Ok(Some(chunk_res)) => chunk_res,
                },
            };
            let chunk = next.map_err(|e| RequestError::Network(e.to_string()))?;
            total_received += chunk.len();
            if body_bytes.len() < MAX_STREAMED_BODY_CAPTURE {
                let room = MAX_STREAMED_BODY_CAPTURE - body_bytes.len();
                body_bytes.extend_from_slice(&chunk[..chunk.len().min(room)]);
            }
            if is_event_stream {
                // Bound transient parse output even if a transport chunk
                // contains many complete events.
                for part in chunk.chunks(64 * 1024) {
                    for out in parser.feed(part) {
                        tokio::select! {
                            _ = &mut cancel_rx => return Err(RequestError::Cancelled),
                            sent = sse_tx.send(out) => if sent.is_err() { return Err(RequestError::Cancelled); },
                        }
                    }
                    if parser.limit_exceeded() {
                        return Err(RequestError::ResponseTooLarge(MAX_REALTIME_FRAME_BYTES));
                    }
                }
            }
            if let Some(tx) = &progress_tx {
                if last_progress_emit.is_none_or(|t| t.elapsed() >= PROGRESS_EMIT_INTERVAL) {
                    let _ = tx.try_send(HttpProgress::Downloading {
                        received_bytes: total_received,
                        total_bytes: None,
                    });
                    last_progress_emit = Some(Instant::now());
                }
            }
        }
        let download_ms = download_start.elapsed().as_millis() as u64;

        if is_event_stream {
            for out in parser.finish() {
                tokio::select! {
                    _ = &mut cancel_rx => return Err(RequestError::Cancelled),
                    sent = sse_tx.send(out) => if sent.is_err() { return Err(RequestError::Cancelled); },
                }
            }
            if parser.limit_exceeded() {
                return Err(RequestError::ResponseTooLarge(MAX_REALTIME_FRAME_BYTES));
            }
        }

        let total_ms = start_time.elapsed().as_millis() as u64;
        let is_https = req.url.starts_with("https://");
        let timing = Self::build_timing(total_ms, ttfb_ms, download_ms, is_reused, is_https);
        let cookies = self.get_cookies_for_url(&req.url);

        Ok(HttpResponseRecord {
            id: uuid::Uuid::new_v4().to_string(),
            request_id: req.request_id,
            status_code,
            status_text,
            duration_ms: total_ms,
            size_bytes: total_received,
            headers: resp_headers,
            cookies,
            body: body_bytes,
            timing,
            request_sent: Some(req.sent_info),
            test_results: Vec::new(),
            console_logs: Vec::new(),
            created_at: Utc::now(),
        })
    }
}
