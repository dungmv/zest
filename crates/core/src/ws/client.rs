use super::types::{WsEvent, WsOutgoing};
use crate::transport::{
    FrameDirection, FrameKind, RequestError, ResolvedRequest, TransportFrame,
    MAX_REALTIME_FRAME_BYTES, REALTIME_QUEUE_CAPACITY,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message, WebSocketConfig};
use tokio_tungstenite::{connect_async_with_config, MaybeTlsStream, WebSocketStream};

/// Handle to one live WebSocket connection.
pub struct WsHandle {
    /// Connection lifecycle + inbound frames.
    pub events: Receiver<WsEvent>,
    /// Send frames / close the connection. Dropping all senders closes it.
    pub outgoing: Sender<WsOutgoing>,
}

/// Spawns WebSocket connections as background task pairs.
pub struct WebSocketClient;

/// Headers that would corrupt the handshake if user-supplied; tungstenite
/// generates them itself when the request is built.
const RESERVED_HEADERS: [&str; 4] = [
    "connection",
    "upgrade",
    "sec-websocket-key",
    "sec-websocket-version",
];

/// Internal writer-loop action.
enum WriteAction {
    Send(Message),
    Close(Option<CloseFrame<'static>>),
}

impl WebSocketClient {
    /// Performs the handshake with the configured timeout and spawns the
    /// reader/writer task pair backing the returned handle. Must be called
    /// inside the shared tokio runtime.
    pub async fn connect(req: &ResolvedRequest) -> Result<WsHandle, RequestError> {
        let request = Self::build_handshake_request(req)?;

        let timeout_ms = req.settings.timeout_ms;
        let started = std::time::Instant::now();
        let config = WebSocketConfig {
            max_message_size: Some(MAX_REALTIME_FRAME_BYTES),
            max_frame_size: Some(MAX_REALTIME_FRAME_BYTES),
            ..Default::default()
        };
        let connect_fut = connect_async_with_config(request, Some(config), false);
        let (stream, response) =
            tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), connect_fut)
                .await
                .map_err(|_| RequestError::Timeout(timeout_ms))?
                .map_err(|e| RequestError::Network(format!("WebSocket handshake failed: {e}")))?;

        let response_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    String::from_utf8_lossy(v.as_bytes()).to_string(),
                )
            })
            .collect();

        let (events_tx, events_rx) = mpsc::channel(REALTIME_QUEUE_CAPACITY);
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<WsOutgoing>(REALTIME_QUEUE_CAPACITY);

        // The handshake result opens the event stream so consumers observe
        // the whole lifecycle on one channel.
        let _ = events_tx
            .send(WsEvent::Connected {
                response_headers,
                duration_ms: started.elapsed().as_millis() as u64,
            })
            .await;

        Self::spawn_tasks(stream, events_tx, outgoing_rx);

        Ok(WsHandle {
            events: events_rx,
            outgoing: outgoing_tx,
        })
    }

    /// Maps the resolved HTTP-style request onto a tungstenite handshake
    /// request: `http(s)://` is accepted and normalized to `ws(s)://`,
    /// query params are already baked into the URL, and reserved handshake
    /// headers are filtered out of the user-supplied set.
    fn build_handshake_request(
        req: &ResolvedRequest,
    ) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, RequestError> {
        let mut request = req
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| RequestError::InvalidUrl(format!("Invalid WebSocket URL: {e}")))?;

        let headers: &mut HeaderMap = request.headers_mut();
        for (name, value) in &req.headers {
            let lower = name.to_lowercase();
            if RESERVED_HEADERS.contains(&lower.as_str()) {
                continue;
            }
            let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                RequestError::InvalidHeader(format!("Invalid header name '{name}': {e}"))
            })?;
            let header_value = HeaderValue::from_str(value).map_err(|e| {
                RequestError::InvalidHeader(format!("Invalid header value for '{name}': {e}"))
            })?;
            headers.insert(header_name, header_value);
        }
        Ok(request)
    }

    fn spawn_tasks(
        stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        events_tx: Sender<WsEvent>,
        outgoing_rx: Receiver<WsOutgoing>,
    ) {
        let (mut sink, mut source) = stream.split();
        // Reader → writer channel for automatic Pong replies.
        let (pong_tx, mut pong_rx) = mpsc::channel::<Message>(REALTIME_QUEUE_CAPACITY);
        let mut outgoing_rx = outgoing_rx;

        // Writer: drains user actions and auto-pong responses.
        tokio::spawn(async move {
            loop {
                let action = tokio::select! {
                    out = outgoing_rx.recv() => match out {
                        Some(WsOutgoing::Text(t)) => WriteAction::Send(Message::Text(t)),
                        Some(WsOutgoing::Binary(b)) => WriteAction::Send(Message::Binary(b)),
                        Some(WsOutgoing::Ping) => WriteAction::Send(Message::Ping(Vec::new())),
                        Some(WsOutgoing::Close) | None => WriteAction::Close(None),
                    },
                    pong = pong_rx.recv() => match pong {
                        Some(msg) => WriteAction::Send(msg),
                        None => WriteAction::Close(None),
                    },
                };
                match action {
                    WriteAction::Send(msg) => {
                        if sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                    WriteAction::Close(frame) => {
                        let _ = sink.send(Message::Close(frame)).await;
                        let _ = sink.close().await;
                        break;
                    }
                }
            }
        });

        // Reader: forwards frames and lifecycle events, auto-pongs pings.
        tokio::spawn(async move {
            let mut next_id: u64 = 0;
            loop {
                let next = tokio::select! {
                    _ = events_tx.closed() => break,
                    next = source.next() => next,
                };
                match next {
                    Some(Ok(msg)) => {
                        let (kind, payload, pong) = match msg {
                            Message::Text(t) => (FrameKind::Text, t.into_bytes(), None),
                            Message::Binary(b) => (FrameKind::Binary, b, None),
                            Message::Ping(p) => {
                                let pong = Message::Pong(p.clone());
                                (FrameKind::Ping, p, Some(pong))
                            }
                            Message::Pong(p) => (FrameKind::Pong, p, None),
                            Message::Close(frame) => {
                                let (code, reason) = frame
                                    .map(|f| (Some(u16::from(f.code)), f.reason.to_string()))
                                    .unwrap_or((None, String::new()));
                                let _ = events_tx.send(WsEvent::Closed { code, reason }).await;
                                // Release the writer so it can flush the
                                // close ack and exit.
                                drop(pong_tx);
                                break;
                            }
                            Message::Frame(_) => continue,
                        };
                        if let Some(pong) = pong {
                            let _ = pong_tx.send(pong).await;
                        }
                        next_id = next_id.wrapping_add(1);
                        let _ = events_tx
                            .send(WsEvent::Message(TransportFrame {
                                id: next_id,
                                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                                direction: FrameDirection::Inbound,
                                kind,
                                payload,
                                label: None,
                            }))
                            .await;
                    }
                    Some(Err(e)) => {
                        let _ = events_tx
                            .send(WsEvent::Error(format!("WebSocket error: {e}")))
                            .await;
                        drop(pong_tx);
                        break;
                    }
                    None => {
                        let _ = events_tx
                            .send(WsEvent::Closed {
                                code: None,
                                reason: "connection ended".to_string(),
                            })
                            .await;
                        drop(pong_tx);
                        break;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
