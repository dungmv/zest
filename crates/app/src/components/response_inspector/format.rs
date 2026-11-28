//! Response body formatting, binary sniffing, bounded line slicing, and image preview metadata.

use std::sync::Arc;
use zest_core::models::{
    probe_image_dimensions, HttpResponseRecord, ImagePreviewFormat, Protocol, ResponsePreviewKind,
};

/// Bodies above this size skip JSON/XML pretty-printing entirely and decode
/// only a bounded preview prefix so multi-megabyte downloads never stall the
/// UI thread or balloon memory.
pub const MAX_FORMAT_BODY_BYTES: usize = 2 * 1024 * 1024;
/// Maximum byte prefix decoded into preview text for oversized (> 2 MB) text
/// responses. Full payload remains available via "Save to File".
pub(crate) const MAX_OVERSIZED_PREVIEW_BYTES: usize = 512 * 1024;
/// Number of leading bytes inspected when sniffing for binary payloads.
const BINARY_SNIFF_BYTES: usize = 8 * 1024;
/// Images above this byte size skip in-app decoding to avoid OOM/stalls;
/// users can still download the raw payload via "Save to File".
pub const MAX_PREVIEW_IMAGE_BYTES: usize = 25 * 1024 * 1024;
/// Maximum number of lines materialized for the (virtualized) body view;
/// payloads beyond the cap end with a truncation marker line.
const MAX_FORMAT_LINES: usize = 2000;
/// Maximum characters rendered per line: single-line megabyte bodies (e.g.
/// minified JSON) would otherwise become one giant text node per frame.
const MAX_FORMAT_LINE_CHARS: usize = 1000;

const IMAGE_ZOOM_STEPS: &[u16] = &[25, 50, 75, 100, 150, 200, 300, 400];

/// Zoom mode for the response image preview canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageZoomMode {
    #[default]
    Fit,
    Scale(u16),
}

impl ImageZoomMode {
    pub fn zoom_in(self) -> Self {
        let cur = match self {
            Self::Fit => 100,
            Self::Scale(pct) => pct,
        };
        let next = IMAGE_ZOOM_STEPS
            .iter()
            .copied()
            .find(|&step| step > cur)
            .unwrap_or(*IMAGE_ZOOM_STEPS.last().unwrap_or(&400));
        Self::Scale(next)
    }

    pub fn zoom_out(self) -> Self {
        let cur = match self {
            Self::Fit => 100,
            Self::Scale(pct) => pct,
        };
        let prev = IMAGE_ZOOM_STEPS
            .iter()
            .rev()
            .copied()
            .find(|&step| step < cur)
            .unwrap_or(*IMAGE_ZOOM_STEPS.first().unwrap_or(&25));
        Self::Scale(prev)
    }

    pub fn label(self) -> String {
        match self {
            Self::Fit => "Fit".to_string(),
            Self::Scale(pct) => format!("{pct}%"),
        }
    }
}

fn to_gpui_image_format(fmt: ImagePreviewFormat) -> gpui_kit::ImageFormat {
    match fmt {
        ImagePreviewFormat::Png => gpui_kit::ImageFormat::Png,
        ImagePreviewFormat::Jpeg => gpui_kit::ImageFormat::Jpeg,
        ImagePreviewFormat::Webp => gpui_kit::ImageFormat::Webp,
        ImagePreviewFormat::Gif => gpui_kit::ImageFormat::Gif,
        ImagePreviewFormat::Svg => gpui_kit::ImageFormat::Svg,
        ImagePreviewFormat::Bmp => gpui_kit::ImageFormat::Bmp,
        ImagePreviewFormat::Ico => gpui_kit::ImageFormat::Ico,
        ImagePreviewFormat::Tiff => gpui_kit::ImageFormat::Tiff,
    }
}

/// Returns `true` when the response carries a non-text binary payload
/// (by `Content-Type` header or NUL-byte sniffing in the leading 8 KB).
fn is_likely_binary_response(resp: &HttpResponseRecord) -> bool {
    if resp.body.is_empty() {
        return false;
    }
    if let Some(mime) = resp
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
    {
        let is_binary_mime = mime.starts_with("video/")
            || mime.starts_with("audio/")
            || mime.starts_with("font/")
            || (mime.starts_with("image/") && mime != "image/svg+xml")
            || matches!(
                mime.as_str(),
                "application/octet-stream"
                    | "application/pdf"
                    | "application/zip"
                    | "application/gzip"
                    | "application/x-gzip"
                    | "application/x-tar"
                    | "application/x-bzip2"
                    | "application/x-xz"
                    | "application/x-7z-compressed"
                    | "application/x-rar-compressed"
                    | "application/vnd.rar"
                    | "application/zstd"
                    | "application/wasm"
                    | "application/x-mach-binary"
                    | "application/x-executable"
                    | "application/x-msdownload"
                    | "application/x-apple-diskimage"
                    | "application/msword"
                    | "application/protobuf"
                    | "application/x-protobuf"
            )
            || mime.starts_with("application/vnd.openxmlformats-")
            || mime.starts_with("application/vnd.ms-");
        if is_binary_mime {
            return true;
        }
    }

    let sample = &resp.body[..resp.body.len().min(BINARY_SNIFF_BYTES)];
    sample.contains(&0)
}

/// Decodes at most `max_bytes` from `bytes` into a UTF-8 `String`, trimming
/// any trailing incomplete UTF-8 sequence caused by the byte cap.
pub(crate) fn decode_bounded_utf8_prefix(bytes: &[u8], max_bytes: usize) -> String {
    let mut end = bytes.len().min(max_bytes);
    if bytes.len() > max_bytes {
        let start = end.saturating_sub(3);
        if let Err(err) = std::str::from_utf8(&bytes[start..end]) {
            if err.error_len().is_none() {
                end = start + err.valid_up_to();
            }
        }
    }
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Splits at most [`MAX_FORMAT_LINES`] lines of at most
/// [`MAX_FORMAT_LINE_CHARS`] chars each, appending truncation markers when
/// the payload continues beyond either cap. Runs in bounded time without
/// scanning past `MAX_FORMAT_LINES + 1`.
pub(crate) fn bounded_lines(text: &str, truncated_bytes_note: Option<String>) -> Vec<String> {
    let mut iter = text.lines();
    let mut lines: Vec<String> = iter
        .by_ref()
        .take(MAX_FORMAT_LINES)
        .map(|s| {
            if s.chars().count() > MAX_FORMAT_LINE_CHARS {
                let mut cut: String = s.chars().take(MAX_FORMAT_LINE_CHARS).collect();
                cut.push_str("… (line truncated)");
                cut
            } else {
                s.to_string()
            }
        })
        .collect();
    let has_more_lines = iter.next().is_some();
    if let Some(note) = truncated_bytes_note {
        lines.push(note);
    } else if has_more_lines {
        lines.push(format!(
            "… (truncated, showing first {} lines)",
            MAX_FORMAT_LINES
        ));
    }
    lines
}

#[derive(Clone, Debug)]
pub(crate) struct CachedResponseFormat {
    pub(crate) response_ptr: usize,
    pub(crate) raw_text: Arc<String>,
    pub(crate) raw_lines: Arc<Vec<String>>,
    pub(crate) pretty_text: Arc<String>,
    pub(crate) pretty_lines: Arc<Vec<String>>,
    /// Parsed JSON body, retained for JSONPath query evaluation (kept only
    /// for the active response, so memory stays bounded).
    pub(crate) parsed_json: Option<Arc<serde_json::Value>>,
    pub(crate) is_json: bool,
    pub(crate) is_xml: bool,
    pub(crate) is_binary: bool,
    pub(crate) is_truncated: bool,
    pub(crate) preview_kind: Option<ResponsePreviewKind>,
    pub(crate) preview_image: Option<Arc<gpui_kit::Image>>,
    pub(crate) image_dimensions: Option<(u32, u32)>,
}

#[derive(Clone, Debug, Default)]
pub struct FormattedResponseBody {
    pub display_text: Arc<String>,
    pub lines: Arc<Vec<String>>,
    pub is_json: bool,
    pub is_xml: bool,
    pub is_binary: bool,
    pub is_truncated: bool,
    pub preview_kind: Option<ResponsePreviewKind>,
    pub preview_image: Option<Arc<gpui_kit::Image>>,
    pub image_dimensions: Option<(u32, u32)>,
    /// Number of JSONPath query matches when a query is active, else `None`.
    pub match_count: Option<usize>,
    pub(crate) cached_format: Option<Arc<CachedResponseFormat>>,
}

pub(crate) fn build_cached_format(resp: &Arc<HttpResponseRecord>) -> CachedResponseFormat {
    let preview_kind = resp.preview_kind();
    let (preview_image, image_dimensions, raster_placeholder) = match preview_kind {
        Some(ResponsePreviewKind::Image(fmt)) => {
            let image =
                (!resp.body.is_empty() && resp.body.len() <= MAX_PREVIEW_IMAGE_BYTES).then(|| {
                    Arc::new(gpui_kit::Image::from_bytes(
                        to_gpui_image_format(fmt),
                        resp.body.clone(),
                    ))
                });
            let dims = probe_image_dimensions(fmt, &resp.body);
            let placeholder = fmt.is_raster().then(|| {
                format!(
                    "[Binary {} image — {}. Switch to Preview or Save to File.]",
                    fmt.label(),
                    resp.formatted_size()
                )
            });
            (image, dims, placeholder)
        }
        None => (None, None, None),
    };

    if let Some(placeholder) = raster_placeholder {
        let lines = Arc::new(vec![placeholder.clone()]);
        let text = Arc::new(placeholder);
        return CachedResponseFormat {
            response_ptr: Arc::as_ptr(resp) as usize,
            raw_text: text.clone(),
            raw_lines: lines.clone(),
            pretty_text: text,
            pretty_lines: lines,
            parsed_json: None,
            is_json: false,
            is_xml: false,
            is_binary: true,
            is_truncated: false,
            preview_kind,
            preview_image,
            image_dimensions,
        };
    }

    if is_likely_binary_response(resp) {
        let content_type = resp
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.split(';').next().unwrap_or("").trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("binary");
        let placeholder = format!(
            "[Binary response ({}, {}) — use \"Save to File\" to download.]",
            content_type,
            resp.formatted_size()
        );
        let lines = Arc::new(vec![placeholder.clone()]);
        let text = Arc::new(placeholder);
        return CachedResponseFormat {
            response_ptr: Arc::as_ptr(resp) as usize,
            raw_text: text.clone(),
            raw_lines: lines.clone(),
            pretty_text: text,
            pretty_lines: lines,
            parsed_json: None,
            is_json: false,
            is_xml: false,
            is_binary: true,
            is_truncated: false,
            preview_kind,
            preview_image,
            image_dimensions,
        };
    }

    let oversized = resp.body.len() > MAX_FORMAT_BODY_BYTES;
    if oversized {
        let raw_str = decode_bounded_utf8_prefix(&resp.body, MAX_OVERSIZED_PREVIEW_BYTES);
        let note = format!(
            "… (truncated — showing first 512 KB of {}. Use \"Save to File\" for full payload.)",
            resp.formatted_size()
        );
        let raw_lines: Arc<Vec<String>> = Arc::new(bounded_lines(&raw_str, Some(note)));
        let raw_text = Arc::new(raw_str);
        return CachedResponseFormat {
            response_ptr: Arc::as_ptr(resp) as usize,
            raw_text: raw_text.clone(),
            raw_lines: raw_lines.clone(),
            pretty_text: raw_text,
            pretty_lines: raw_lines,
            parsed_json: None,
            is_json: false,
            is_xml: false,
            is_binary: false,
            is_truncated: true,
            preview_kind,
            preview_image,
            image_dimensions,
        };
    }

    let raw_str = String::from_utf8_lossy(&resp.body).into_owned();
    let parsed_json = serde_json::from_str::<serde_json::Value>(&raw_str).ok();
    let is_json = parsed_json.is_some()
        || resp.is_json()
        || raw_str.trim_start().starts_with('{')
        || raw_str.trim_start().starts_with('[');

    let is_xml = !is_json
        && (resp.is_xml()
            || raw_str.trim_start().starts_with("<?xml")
            || (raw_str.trim_start().starts_with('<') && raw_str.trim_end().ends_with('>')));

    let formatted_pretty = if let Some(parsed) = &parsed_json {
        serde_json::to_string_pretty(parsed).ok()
    } else if is_xml {
        crate::components::body_editor::format_xml(&raw_str).ok()
    } else {
        None
    };

    let raw_lines: Arc<Vec<String>> = Arc::new(bounded_lines(&raw_str, None));
    let raw_text = Arc::new(raw_str);
    let (pretty_text, pretty_lines) = match formatted_pretty {
        Some(pretty) if pretty != *raw_text => {
            let p_lines = Arc::new(bounded_lines(&pretty, None));
            (Arc::new(pretty), p_lines)
        }
        _ => (raw_text.clone(), raw_lines.clone()),
    };

    CachedResponseFormat {
        response_ptr: Arc::as_ptr(resp) as usize,
        raw_text,
        raw_lines,
        pretty_text,
        pretty_lines,
        parsed_json: parsed_json.map(Arc::new),
        is_json,
        is_xml,
        is_binary: false,
        is_truncated: false,
        preview_kind,
        preview_image,
        image_dimensions,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseTabKind {
    #[default]
    Response,
    Request,
    Messages,
    Headers,
    Cookies,
    Console,
}

impl ResponseTabKind {
    /// Response-area tab strip for a request of the given protocol:
    /// - HTTP/GraphQL: Response, Request Sent, Headers, Cookies, Console
    /// - SSE: Messages (live event log), Response (accumulated), Headers,
    ///   Cookies, Console
    /// - WebSocket: Messages (frame log), Headers (handshake), Console
    /// - gRPC: Messages (stream frames), Headers, Console
    pub fn tabs_for(protocol: Protocol) -> &'static [ResponseTabKind] {
        match protocol {
            Protocol::Http | Protocol::GraphQL => &[
                ResponseTabKind::Response,
                ResponseTabKind::Request,
                ResponseTabKind::Headers,
                ResponseTabKind::Cookies,
                ResponseTabKind::Console,
            ],
            Protocol::Sse => &[
                ResponseTabKind::Messages,
                ResponseTabKind::Response,
                ResponseTabKind::Headers,
                ResponseTabKind::Cookies,
                ResponseTabKind::Console,
            ],
            Protocol::WebSocket | Protocol::Grpc => &[
                ResponseTabKind::Messages,
                ResponseTabKind::Headers,
                ResponseTabKind::Console,
            ],
        }
    }

    /// Returns `kind` if it is valid for `protocol`, else the first valid tab.
    pub fn clamp_for(kind: ResponseTabKind, protocol: Protocol) -> ResponseTabKind {
        let tabs = Self::tabs_for(protocol);
        if tabs.contains(&kind) {
            kind
        } else {
            tabs[0]
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ResponseTabKind::Response => "Response",
            ResponseTabKind::Request => "Request Sent",
            ResponseTabKind::Messages => "Messages",
            ResponseTabKind::Headers => "Headers",
            ResponseTabKind::Cookies => "Cookies",
            ResponseTabKind::Console => "Console",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleFilter {
    #[default]
    All,
    Tests,
    Logs,
    Errors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseViewMode {
    #[default]
    Pretty,
    Raw,
    Preview,
    Hex,
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
