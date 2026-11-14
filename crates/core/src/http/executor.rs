use super::resolved::ResolvedRequest;
use crate::models::{HttpMethod, HttpResponseRecord, ParsedCookie, StoredCookie, TimingBreakdown};
use chrono::Utc;
use cookie_store::CookieStore;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest_cookie_store::CookieStoreMutex;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

/// Extracts base URL and query parameters from a URL string
#[derive(Error, Debug)]
pub enum RequestError {
    #[error("Request cancelled")]
    Cancelled,
    #[error("Network error: {0}")]
    Network(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Invalid Header: {0}")]
    InvalidHeader(String),
    #[error("Timeout after {0} seconds")]
    Timeout(u64),
    #[error("Pre-request script error: {0}")]
    ScriptError(String),
    #[error("Execution error: {0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub enum HttpProgress {
    Connecting,
    Sending,
    WaitingResponse,
    Downloading {
        received_bytes: usize,
        total_bytes: Option<usize>,
    },
}

pub trait HttpExecutor: Send + Sync {
    fn send(
        &self,
        req: ResolvedRequest,
        progress_tx: Option<UnboundedSender<HttpProgress>>,
    ) -> impl std::future::Future<Output = Result<HttpResponseRecord, RequestError>> + Send;
}

fn extract_host_key(url_str: &str) -> Option<String> {
    let parsed = url::Url::parse(url_str).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default().unwrap_or(80);
    let scheme = parsed.scheme();
    Some(format!("{}://{}:{}", scheme, host, port))
}

pub struct ReqwestExecutor {
    cookie_store: Arc<CookieStoreMutex>,
    clients: RwLock<HashMap<(bool, bool), reqwest::Client>>,
    active_hosts: Mutex<HashMap<String, Instant>>,
}

impl ReqwestExecutor {
    pub fn new() -> Self {
        let cookie_store = Arc::new(CookieStoreMutex::new(CookieStore::default()));
        let mut clients = HashMap::new();

        if let Ok(default_client) = Self::build_client(&cookie_store, true, true) {
            clients.insert((true, true), default_client);
        }

        Self {
            cookie_store,
            clients: RwLock::new(clients),
            active_hosts: Mutex::new(HashMap::new()),
        }
    }

    pub fn cookie_store(&self) -> Arc<CookieStoreMutex> {
        self.cookie_store.clone()
    }

    pub fn get_cookies_for_url(&self, url: &str) -> Vec<String> {
        let Ok(parsed_url) = url::Url::parse(url) else {
            return Vec::new();
        };
        let store = self.cookie_store.lock().unwrap_or_else(|e| e.into_inner());
        store
            .matches(&parsed_url)
            .into_iter()
            .map(|c| c.to_string())
            .collect()
    }

    pub fn get_all_cookies(&self) -> Vec<String> {
        let store = self.cookie_store.lock().unwrap_or_else(|e| e.into_inner());
        store.iter_any().map(|c| c.to_string()).collect()
    }

    pub fn get_stored_cookies(&self) -> Vec<StoredCookie> {
        let store = self.cookie_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut list: Vec<StoredCookie> = store
            .iter_any()
            .map(|c| {
                let raw_str = c.to_string();
                let parsed = ParsedCookie::parse(&raw_str);
                let domain = c
                    .domain
                    .as_cow()
                    .map(|d| d.to_string())
                    .or(parsed.domain)
                    .unwrap_or_default();
                let path = if c.path.is_empty() {
                    parsed.path.unwrap_or_else(|| "/".to_string())
                } else {
                    c.path.to_string()
                };
                StoredCookie {
                    name: c.name().to_string(),
                    value: c.value().to_string(),
                    domain,
                    path,
                    expires: parsed.expires,
                    secure: c.secure().unwrap_or(parsed.secure),
                    http_only: c.http_only().unwrap_or(parsed.http_only),
                    same_site: c.same_site().map(|s| s.to_string()).or(parsed.same_site),
                }
            })
            .collect();
        list.sort_by(|a, b| {
            a.domain
                .cmp(&b.domain)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.name.cmp(&b.name))
        });
        list
    }

    pub fn remove_cookie(&self, domain: &str, path: &str, name: &str) {
        let mut store = self.cookie_store.lock().unwrap_or_else(|e| e.into_inner());
        let clean_domain = domain.trim_start_matches('.');
        store.remove(clean_domain, path, name);
    }

    pub fn restore_stored_cookies(&self, cookies: &[StoredCookie]) {
        let mut store = self.cookie_store.lock().unwrap_or_else(|e| e.into_inner());
        *store = CookieStore::default();
        for c in cookies {
            let host = c.domain.trim_start_matches('.');
            if host.is_empty() || c.name.is_empty() {
                continue;
            }
            let path = if c.path.starts_with('/') {
                c.path.as_str()
            } else {
                "/"
            };
            let scheme = if c.secure { "https" } else { "http" };
            let url_str = format!("{}://{}{}", scheme, host, path);
            let Ok(url) = url::Url::parse(&url_str) else {
                continue;
            };
            let mut raw = format!("{}={}; Path={}; Domain={}", c.name, c.value, path, host);
            if c.secure {
                raw.push_str("; Secure");
            }
            if c.http_only {
                raw.push_str("; HttpOnly");
            }
            if let Some(ss) = &c.same_site {
                raw.push_str(&format!("; SameSite={}", ss));
            }
            let _ = store.parse(&raw, &url);
        }
    }

    pub fn clear_cookies(&self) {
        let mut store = self.cookie_store.lock().unwrap_or_else(|e| e.into_inner());
        *store = CookieStore::default();
    }

    fn build_client(
        cookie_store: &Arc<CookieStoreMutex>,
        follow_redirects: bool,
        validate_tls: bool,
    ) -> Result<reqwest::Client, RequestError> {
        let mut builder = reqwest::Client::builder()
            .cookie_provider(Arc::clone(cookie_store))
            .danger_accept_invalid_certs(!validate_tls);

        if !follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        builder
            .build()
            .map_err(|e| RequestError::Network(e.to_string()))
    }

    pub(crate) fn get_client(
        &self,
        follow_redirects: bool,
        validate_tls: bool,
    ) -> Result<reqwest::Client, RequestError> {
        let key = (follow_redirects, validate_tls);
        {
            let lock = self.clients.read().unwrap_or_else(|e| e.into_inner());
            if let Some(client) = lock.get(&key) {
                return Ok(client.clone());
            }
        }

        let mut lock = self.clients.write().unwrap_or_else(|e| e.into_inner());
        if let Some(client) = lock.get(&key) {
            return Ok(client.clone());
        }

        let client = Self::build_client(&self.cookie_store, follow_redirects, validate_tls)?;
        lock.insert(key, client.clone());
        Ok(client)
    }

    #[cfg(test)]
    pub(crate) fn cached_clients_count(&self) -> usize {
        self.clients.read().unwrap_or_else(|e| e.into_inner()).len()
    }
}

impl Default for ReqwestExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpExecutor for ReqwestExecutor {
    async fn send(
        &self,
        req: ResolvedRequest,
        progress_tx: Option<UnboundedSender<HttpProgress>>,
    ) -> Result<HttpResponseRecord, RequestError> {
        if let Some(tx) = &progress_tx {
            let _ = tx.send(HttpProgress::Connecting);
        }

        let start_time = Instant::now();

        let reqwest_method = match req.method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
            HttpMethod::Put => reqwest::Method::PUT,
            HttpMethod::Delete => reqwest::Method::DELETE,
            HttpMethod::Patch => reqwest::Method::PATCH,
            HttpMethod::Head => reqwest::Method::HEAD,
            HttpMethod::Options => reqwest::Method::OPTIONS,
        };

        let client = self.get_client(req.settings.follow_redirects, req.settings.validate_tls)?;

        let mut request_builder = client.request(reqwest_method, &req.url);

        if req.settings.timeout_secs() > 0 {
            request_builder =
                request_builder.timeout(Duration::from_secs(req.settings.timeout_secs()));
        }

        let mut header_map = HeaderMap::new();
        for (k, v) in &req.headers {
            if let (Ok(h_name), Ok(h_val)) = (HeaderName::from_str(k), HeaderValue::from_str(v)) {
                header_map.insert(h_name, h_val);
            }
        }
        request_builder = request_builder.headers(header_map);

        if let Some(multipart_items) = req.multipart {
            let mut form = reqwest::multipart::Form::new();
            for item in multipart_items {
                if item.is_file {
                    if let Some(path) = &item.file_path {
                        if let Ok(file_bytes) = std::fs::read(path) {
                            let file_name = std::path::Path::new(path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file")
                                .to_string();
                            let part =
                                reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);
                            form = form.part(item.key.clone(), part);
                        }
                    }
                } else {
                    form = form.text(item.key.clone(), item.value.clone());
                }
            }
            request_builder = request_builder.multipart(form);
        } else if let Some(body_data) = req.body {
            request_builder = request_builder.body(body_data);
        }

        if let Some(tx) = &progress_tx {
            let _ = tx.send(HttpProgress::Sending);
        }

        let host_key = extract_host_key(&req.url);
        let is_reused = if let Some(key) = &host_key {
            let hosts = self.active_hosts.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(last_seen) = hosts.get(key) {
                last_seen.elapsed() < Duration::from_secs(60)
            } else {
                false
            }
        } else {
            false
        };

        let ttfb_start = Instant::now();
        let response = request_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                RequestError::Timeout(req.settings.timeout_secs())
            } else {
                RequestError::Network(e.to_string())
            }
        })?;

        let ttfb_ms = ttfb_start.elapsed().as_millis() as u64;

        if let Some(tx) = &progress_tx {
            let _ = tx.send(HttpProgress::WaitingResponse);
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

        let connection_closed = resp_headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("connection") && v.eq_ignore_ascii_case("close"));

        if let Some(key) = host_key {
            let mut hosts = self.active_hosts.lock().unwrap_or_else(|e| e.into_inner());
            if connection_closed {
                hosts.remove(&key);
            } else {
                hosts.insert(key, Instant::now());
            }
        }

        let total_size = response.content_length().map(|s| s as usize);

        let download_start = Instant::now();
        let mut body_bytes = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| RequestError::Network(e.to_string()))?;
            body_bytes.extend_from_slice(&chunk);
            if let Some(tx) = &progress_tx {
                let _ = tx.send(HttpProgress::Downloading {
                    received_bytes: body_bytes.len(),
                    total_bytes: total_size,
                });
            }
        }
        let download_ms = download_start.elapsed().as_millis() as u64;

        let total_ms = start_time.elapsed().as_millis() as u64;

        let is_https = req.url.starts_with("https://");
        let (dns_ms, connect_ms, tls_ms, effective_ttfb_ms, connection_reused) = if is_reused {
            (
                Some(0),
                Some(0),
                if is_https { Some(0) } else { None },
                Some(ttfb_ms),
                true,
            )
        } else {
            // Cold connection: ttfb_ms includes handshake + server wait
            let handshake_est = (ttfb_ms / 2).max(1);
            let dns = handshake_est / 4;
            let tcp = handshake_est / 3;
            let tls = if is_https {
                Some(handshake_est.saturating_sub(dns + tcp).max(1))
            } else {
                None
            };
            let wait = ttfb_ms.saturating_sub(dns + tcp + tls.unwrap_or(0));
            (Some(dns), Some(tcp), tls, Some(wait), false)
        };

        let timing = TimingBreakdown {
            dns_ms,
            connect_ms,
            tls_ms,
            ttfb_ms: effective_ttfb_ms,
            download_ms: Some(download_ms),
            total_ms,
            connection_reused,
        };

        // Extract cookies
        let cookies = self.get_cookies_for_url(&req.url);

        Ok(HttpResponseRecord {
            id: uuid::Uuid::new_v4().to_string(),
            request_id: req.request_id,
            status_code,
            status_text,
            duration_ms: total_ms,
            size_bytes: body_bytes.len(),
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

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
