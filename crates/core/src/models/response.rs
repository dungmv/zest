use super::request::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TimingBreakdown {
    pub total_ms: u64,
    pub dns_ms: Option<u64>,
    pub connect_ms: Option<u64>,
    pub tls_ms: Option<u64>,
    pub ttfb_ms: Option<u64>,
    pub download_ms: Option<u64>,
    #[serde(default)]
    pub connection_reused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RequestSentInfo {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body_size_bytes: usize,
    pub body_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedCookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<String>,
    pub max_age: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
}

impl ParsedCookie {
    pub fn parse(raw: &str) -> Self {
        let mut parts = raw.split(';').map(|s| s.trim());
        let first = parts.next().unwrap_or("");
        let (name, value) = if let Some((n, v)) = first.split_once('=') {
            (n.trim().to_string(), v.trim().to_string())
        } else {
            (first.to_string(), String::new())
        };

        let mut domain = None;
        let mut path = None;
        let mut expires = None;
        let mut max_age = None;
        let mut secure = false;
        let mut http_only = false;
        let mut same_site = None;

        for part in parts {
            if part.eq_ignore_ascii_case("secure") {
                secure = true;
            } else if part.eq_ignore_ascii_case("httponly") {
                http_only = true;
            } else if let Some((k, v)) = part.split_once('=') {
                let k_trim = k.trim();
                let v_trim = v.trim();
                if k_trim.eq_ignore_ascii_case("domain") {
                    domain = Some(v_trim.to_string());
                } else if k_trim.eq_ignore_ascii_case("path") {
                    path = Some(v_trim.to_string());
                } else if k_trim.eq_ignore_ascii_case("expires") {
                    expires = Some(v_trim.to_string());
                } else if k_trim.eq_ignore_ascii_case("max-age") {
                    max_age = v_trim.parse::<i64>().ok();
                } else if k_trim.eq_ignore_ascii_case("samesite") {
                    same_site = Some(v_trim.to_string());
                }
            }
        }

        Self {
            name,
            value,
            domain,
            path,
            expires,
            max_age,
            secure,
            http_only,
            same_site,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleLogSource {
    PreRequest,
    PostResponse,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleLogLevel {
    Log,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleLogEntry {
    pub source: ConsoleLogSource,
    pub level: ConsoleLogLevel,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HttpResponseRecord {
    pub id: String,
    pub request_id: String,
    pub status_code: u16,
    pub status_text: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<String>,
    pub body: Vec<u8>,
    pub timing: TimingBreakdown,
    #[serde(default)]
    pub request_sent: Option<RequestSentInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_results: Vec<TestResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub console_logs: Vec<ConsoleLogEntry>,
    pub created_at: DateTime<Utc>,
}

pub fn status_code_to_reason(code: u16) -> &'static str {
    reqwest::StatusCode::from_u16(code)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("")
}

impl HttpResponseRecord {
    pub fn body_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.body)
    }

    pub fn formatted_size(&self) -> String {
        let bytes = self.size_bytes;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }

    pub fn formatted_duration(&self) -> String {
        if self.duration_ms < 1000 {
            format!("{} ms", self.duration_ms)
        } else {
            format!("{:.2} s", self.duration_ms as f64 / 1000.0)
        }
    }

    pub fn parsed_cookies(&self) -> Vec<ParsedCookie> {
        self.cookies
            .iter()
            .map(|c| ParsedCookie::parse(c))
            .collect()
    }

    pub fn is_json(&self) -> bool {
        self.headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("content-type") && v.to_lowercase().contains("json")
        })
    }

    pub fn is_xml(&self) -> bool {
        self.headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("content-type")
                && (v.to_lowercase().contains("xml")
                    || v.to_lowercase().contains("application/xml")
                    || v.to_lowercase().contains("text/xml"))
        })
    }

    /// Extracts the filename from the `Content-Disposition` response header, if present.
    pub fn content_disposition_filename(&self) -> Option<String> {
        self.headers.iter().find_map(|(k, v)| {
            if k.eq_ignore_ascii_case("content-disposition") {
                parse_content_disposition_filename(v)
            } else {
                None
            }
        })
    }

    /// Returns the suggested filename when saving this response body to disk.
    /// Uses the `Content-Disposition` header filename when available, otherwise defaults to `"response.json"`.
    pub fn suggested_filename(&self) -> String {
        self.content_disposition_filename()
            .unwrap_or_else(|| "response.json".to_string())
    }
}

/// Parses a `Content-Disposition` header value and extracts the filename,
/// following RFC 6266 and RFC 5987 / RFC 8187.
///
/// Prefers `filename*` (extended parameter with charset and percent-encoding)
/// over `filename` when both are present. Sanitizes the extracted filename to
/// prevent path traversal or control characters.
pub fn parse_content_disposition_filename(header_value: &str) -> Option<String> {
    let mut filename_ext: Option<String> = None;
    let mut filename_plain: Option<String> = None;

    for param in split_disposition_params(header_value) {
        let Some((raw_key, raw_val)) = param.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        let val = raw_val.trim();

        if key.eq_ignore_ascii_case("filename*") {
            if let Some(decoded) = decode_rfc5987_value(val) {
                if let Some(clean) = sanitize_filename(&decoded) {
                    filename_ext = Some(clean);
                }
            }
        } else if key.eq_ignore_ascii_case("filename") {
            let unquoted = unquote_header_value(val);
            if let Some(clean) = sanitize_filename(&unquoted) {
                filename_plain = Some(clean);
            }
        }
    }

    filename_ext.or(filename_plain)
}

fn split_disposition_params(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut escaped = false;

    for (idx, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => {
                parts.push(&input[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    if start <= input.len() {
        parts.push(&input[start..]);
    }
    parts
}

fn unquote_header_value(val: &str) -> String {
    let trimmed = val.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' {
                        out.push(next);
                        chars.next();
                        continue;
                    }
                }
            }
            out.push(ch);
        }
        out
    } else {
        trimmed.to_string()
    }
}

fn decode_rfc5987_value(val: &str) -> Option<String> {
    let unquoted = val.trim().trim_matches('"');
    let (charset, rest) = unquoted.split_once('\'')?;
    let (_lang, encoded) = rest.split_once('\'')?;
    let bytes = urlencoding::decode_binary(encoded.as_bytes());

    if charset.eq_ignore_ascii_case("iso-8859-1")
        || charset.eq_ignore_ascii_case("latin1")
        || charset.eq_ignore_ascii_case("latin-1")
    {
        Some(bytes.iter().map(|&b| b as char).collect())
    } else {
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

fn sanitize_filename(raw: &str) -> Option<String> {
    let basename = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = basename
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string();

    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        None
    } else {
        Some(cleaned)
    }
}
