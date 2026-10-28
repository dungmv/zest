use crate::models::{
    Auth, Environment, HttpMethod, HttpRequestItem, HttpResponseRecord, KeyValuePair,
    MultipartItem, ParsedCookie, RequestBody, RequestSentInfo, RequestSettings, StoredCookie,
    TimingBreakdown,
};
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
pub fn parse_query_from_url(raw_url: &str) -> (String, Vec<KeyValuePair>) {
    if let Some((base, query)) = raw_url.split_once('?') {
        let mut pairs = Vec::new();
        for part in query.split('&') {
            if part.is_empty() {
                continue;
            }
            if let Some((k, v)) = part.split_once('=') {
                let k_norm = k.replace('+', " ");
                let v_norm = v.replace('+', " ");
                let dec_k = urlencoding::decode(&k_norm)
                    .unwrap_or(std::borrow::Cow::Borrowed(&k_norm))
                    .to_string();
                let dec_v = urlencoding::decode(&v_norm)
                    .unwrap_or(std::borrow::Cow::Borrowed(&v_norm))
                    .to_string();
                pairs.push(KeyValuePair::new(dec_k, dec_v));
            } else {
                let part_norm = part.replace('+', " ");
                let dec_k = urlencoding::decode(&part_norm)
                    .unwrap_or(std::borrow::Cow::Borrowed(&part_norm))
                    .to_string();
                pairs.push(KeyValuePair::new(dec_k, ""));
            }
        }
        (base.to_string(), pairs)
    } else {
        (raw_url.to_string(), Vec::new())
    }
}

/// Rebuilds a URL with the given enabled query parameters
pub fn build_url_with_query(base_url: &str, params: &[KeyValuePair]) -> String {
    let base = if let Some((b, _)) = base_url.split_once('?') {
        b
    } else {
        base_url
    };

    let active_params: Vec<_> = params
        .iter()
        .filter(|p| p.enabled && !p.key.trim().is_empty())
        .map(|p| {
            format!(
                "{}={}",
                urlencoding::encode(&p.key),
                urlencoding::encode(&p.value)
            )
        })
        .collect();

    if active_params.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, active_params.join("&"))
    }
}

#[derive(Error, Debug)]
pub enum HttpError {
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

#[derive(Debug, Clone)]
pub struct ResolvedRequest {
    pub request_id: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub multipart: Option<Vec<MultipartItem>>,
    pub sent_info: RequestSentInfo,
    pub settings: RequestSettings,
}

impl ResolvedRequest {
    pub fn from_item(item: &HttpRequestItem, env: Option<&Environment>) -> Result<Self, HttpError> {
        let interpolator = crate::interpolation::VariableInterpolator::new(env);
        let raw_url = item.url.trim();
        let interpolated_url = interpolator.interpolate_cow(raw_url);

        // Base URL (strip any residual query string from URL if params are provided, or use parse_query_from_url)
        let (base_interpolated, raw_url_params) = parse_query_from_url(&interpolated_url);
        let mut full_url = if base_interpolated.starts_with("http://")
            || base_interpolated.starts_with("https://")
        {
            base_interpolated
        } else {
            format!("http://{}", base_interpolated)
        };

        // Append query params
        let mut enabled_params = Vec::new();
        for p in &item.params {
            if p.enabled && !p.key.trim().is_empty() {
                let k = interpolator.interpolate_cow(&p.key);
                let v = interpolator.interpolate_cow(&p.value);
                enabled_params.push((k, v));
            }
        }

        // If item.params had no entries, use raw_url_params from URL
        if enabled_params.is_empty() {
            for p in &raw_url_params {
                if p.enabled && !p.key.trim().is_empty() {
                    let k = interpolator.interpolate_cow(&p.key);
                    let v = interpolator.interpolate_cow(&p.value);
                    enabled_params.push((k, v));
                }
            }
        }

        // Check if auth is ApiKey in query
        if let Auth::ApiKey { key, value, add_to } = &item.auth {
            if add_to.is_query() && !key.trim().is_empty() {
                let k = interpolator.interpolate_cow(key);
                let v = interpolator.interpolate_cow(value);
                enabled_params.push((k, v));
            }
        }

        if !enabled_params.is_empty() {
            let delimiter = if full_url.contains('?') { "&" } else { "?" };
            let query_str = enabled_params
                .into_iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(&k), urlencoding::encode(&v)))
                .collect::<Vec<_>>()
                .join("&");
            full_url.push_str(delimiter);
            full_url.push_str(&query_str);
        }

        // Headers
        let mut headers = Vec::new();
        let mut content_type_set = false;

        for h in &item.headers {
            if h.enabled && !h.key.trim().is_empty() {
                let k = interpolator.interpolate_cow(&h.key);
                let v = interpolator.interpolate_cow(&h.value);
                if k.eq_ignore_ascii_case("content-type") {
                    content_type_set = true;
                }
                headers.push((k.into_owned(), v.into_owned()));
            }
        }

        // Auth handling
        match &item.auth {
            Auth::Inherit | Auth::None => {}
            Auth::Bearer { token } => {
                let t = interpolator.interpolate_cow(token);
                headers.push(("Authorization".to_string(), format!("Bearer {}", t)));
            }
            Auth::Basic { username, password } => {
                let u = interpolator.interpolate_cow(username);
                let p = interpolator.interpolate_cow(password);
                use base64::Engine as _;
                let creds = format!("{}:{}", u, p);
                let encoded = base64::engine::general_purpose::STANDARD.encode(creds.as_bytes());
                headers.push(("Authorization".to_string(), format!("Basic {}", encoded)));
            }
            Auth::ApiKey { key, value, add_to } => {
                if add_to.is_header() && !key.trim().is_empty() {
                    let k = interpolator.interpolate_cow(key);
                    let v = interpolator.interpolate_cow(value);
                    headers.push((k.into_owned(), v.into_owned()));
                }
            }
        }

        // Body handling
        let mut multipart_data = None;
        let body_bytes = match &item.body {
            RequestBody::None => None,
            RequestBody::Json { content: raw_json } => {
                if !content_type_set {
                    headers.push(("Content-Type".to_string(), "application/json".to_string()));
                }
                let interpolated = interpolator.interpolate_cow(raw_json);
                Some(interpolated.into_owned().into_bytes())
            }
            RequestBody::FormUrlEncoded { params: pairs } => {
                if !content_type_set {
                    headers.push((
                        "Content-Type".to_string(),
                        "application/x-www-form-urlencoded".to_string(),
                    ));
                }
                let form_str = pairs
                    .iter()
                    .filter(|p| p.enabled && !p.key.trim().is_empty())
                    .map(|p| {
                        let k = interpolator.interpolate_cow(&p.key);
                        let v = interpolator.interpolate_cow(&p.value);
                        format!("{}={}", urlencoding::encode(&k), urlencoding::encode(&v))
                    })
                    .collect::<Vec<_>>()
                    .join("&");
                Some(form_str.into_bytes())
            }
            RequestBody::Multipart { items } => {
                let interpolated_items: Vec<MultipartItem> = items
                    .iter()
                    .filter(|p| p.enabled && !p.key.trim().is_empty())
                    .map(|p| {
                        let k = interpolator.interpolate_cow(&p.key);
                        let v = interpolator.interpolate_cow(&p.value);
                        let file_path = p
                            .file_path
                            .as_ref()
                            .map(|path| interpolator.interpolate_cow(path).into_owned());
                        MultipartItem {
                            enabled: true,
                            key: k.into_owned(),
                            value: v.into_owned(),
                            is_file: p.is_file,
                            file_path,
                        }
                    })
                    .collect();
                multipart_data = Some(interpolated_items);
                None
            }
            RequestBody::Raw {
                content,
                content_type,
            } => {
                if !content_type_set && !content_type.trim().is_empty() {
                    headers.push(("Content-Type".to_string(), content_type.clone()));
                }
                let interpolated = interpolator.interpolate_cow(content);
                Some(interpolated.into_owned().into_bytes())
            }
            RequestBody::Binary { file_path } => {
                if !content_type_set {
                    headers.push((
                        "Content-Type".to_string(),
                        "application/octet-stream".to_string(),
                    ));
                }
                let interpolated_path = interpolator.interpolate_cow(file_path);
                std::fs::read(interpolated_path.as_ref()).ok()
            }
        };

        let body_preview = if let Some(b) = &body_bytes {
            if let Ok(s) = std::str::from_utf8(b) {
                Some(s.chars().take(2000).collect())
            } else {
                Some(format!("[Binary data: {} bytes]", b.len()))
            }
        } else if let Some(parts) = &multipart_data {
            let desc = parts
                .iter()
                .map(|p| {
                    if p.is_file {
                        format!("{} = @{}", p.key, p.file_path.as_deref().unwrap_or("file"))
                    } else {
                        format!("{} = {}", p.key, p.value)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!("[Multipart: {}]", desc))
        } else {
            None
        };

        let body_size_bytes = body_bytes.as_ref().map(|b| b.len()).unwrap_or(0);

        let sent_info = RequestSentInfo {
            method: item.method,
            url: full_url.clone(),
            headers: headers.clone(),
            body_size_bytes,
            body_preview,
        };

        Ok(ResolvedRequest {
            request_id: item.id.clone(),
            method: item.method,
            url: full_url,
            headers,
            body: body_bytes,
            multipart: multipart_data,
            sent_info,
            settings: item.settings.clone(),
        })
    }

    /// Apply mutations from Pre-request Script (headers, body JSON override)
    pub fn apply_pre_script_mutations(&mut self, pre_output: &crate::scripting::PreRequestOutput) {
        // Override or append new headers
        for (k, v) in &pre_output.header_mutations {
            self.headers
                .retain(|(name, _)| !name.eq_ignore_ascii_case(k));
            self.headers.push((k.clone(), v.clone()));
            self.sent_info
                .headers
                .retain(|(name, _)| !name.eq_ignore_ascii_case(k));
            self.sent_info.headers.push((k.clone(), v.clone()));
        }

        // Override body if provided
        if let Some(new_body_str) = &pre_output.body_override {
            let bytes = new_body_str.clone().into_bytes();
            self.sent_info.body_size_bytes = bytes.len();
            self.sent_info.body_preview = Some(new_body_str.chars().take(2000).collect());
            self.body = Some(bytes);
        }
    }
}

pub trait HttpExecutor: Send + Sync {
    fn send(
        &self,
        req: ResolvedRequest,
        progress_tx: Option<UnboundedSender<HttpProgress>>,
    ) -> impl std::future::Future<Output = Result<HttpResponseRecord, HttpError>> + Send;
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
    ) -> Result<reqwest::Client, HttpError> {
        let mut builder = reqwest::Client::builder()
            .cookie_provider(Arc::clone(cookie_store))
            .danger_accept_invalid_certs(!validate_tls);

        if !follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        builder
            .build()
            .map_err(|e| HttpError::Network(e.to_string()))
    }

    pub(crate) fn get_client(
        &self,
        follow_redirects: bool,
        validate_tls: bool,
    ) -> Result<reqwest::Client, HttpError> {
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
    ) -> Result<HttpResponseRecord, HttpError> {
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
                HttpError::Timeout(req.settings.timeout_secs())
            } else {
                HttpError::Network(e.to_string())
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
            let chunk = chunk_res.map_err(|e| HttpError::Network(e.to_string()))?;
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
mod tests {
    use super::*;
    use crate::models::{HttpMethod, HttpRequestItem, KeyValuePair, MultipartItem, RequestBody};

    #[test]
    fn test_parse_query_from_url() {
        let url = "https://api.example.com/search?q=rust+lang&page=1&empty=&flag";
        let (base, params) = parse_query_from_url(url);
        assert_eq!(base, "https://api.example.com/search");
        assert_eq!(params.len(), 4);
        assert_eq!(params[0].key, "q");
        assert_eq!(params[0].value, "rust lang");
        assert_eq!(params[1].key, "page");
        assert_eq!(params[1].value, "1");
        assert_eq!(params[2].key, "empty");
        assert_eq!(params[2].value, "");
        assert_eq!(params[3].key, "flag");
        assert_eq!(params[3].value, "");
    }

    #[test]
    fn test_build_url_with_query() {
        let base = "https://api.example.com/items";
        let params = vec![
            KeyValuePair::new("type", "tools"),
            KeyValuePair {
                enabled: false,
                key: "disabled".to_string(),
                value: "val".to_string(),
                description: None,
            },
            KeyValuePair::new("limit", "25"),
        ];
        let built = build_url_with_query(base, &params);
        assert_eq!(built, "https://api.example.com/items?type=tools&limit=25");
    }

    #[test]
    fn test_resolved_request_multipart() {
        let mut item = HttpRequestItem::new(
            "ws-1",
            "Test Multipart",
            HttpMethod::Post,
            "http://localhost:8080/upload",
        );
        item.body = RequestBody::Multipart {
            items: vec![
                MultipartItem::new_text("description", "A test file upload"),
                MultipartItem::new_file("file", "C:/dummy/path.png"),
            ],
        };

        let resolved = ResolvedRequest::from_item(&item, None).unwrap();
        assert!(resolved.multipart.is_some());
        let parts = resolved.multipart.unwrap();
        assert_eq!(parts.len(), 2);
        assert!(!parts[0].is_file);
        assert_eq!(parts[0].key, "description");
        assert!(parts[1].is_file);
        assert_eq!(parts[1].key, "file");
        assert!(resolved.sent_info.body_preview.is_some());
    }

    #[test]
    fn test_resolved_request_no_duplicate_query() {
        // Even if item.url has query params and item.params also has them, they shouldn't duplicate
        let mut item = HttpRequestItem::new(
            "ws-1",
            "Test Dedupe",
            HttpMethod::Get,
            "https://api.example.com/search?q=rust",
        );
        item.params = vec![
            KeyValuePair::new("q", "rust"),
            KeyValuePair::new("page", "2"),
        ];

        let resolved = ResolvedRequest::from_item(&item, None).unwrap();
        assert_eq!(resolved.url, "https://api.example.com/search?q=rust&page=2");

        // When item.url is clean base and params are defined
        let mut item_clean = HttpRequestItem::new(
            "ws-1",
            "Test Clean",
            HttpMethod::Get,
            "https://api.example.com/search",
        );
        item_clean.params = vec![KeyValuePair::new("q", "rust")];
        let resolved_clean = ResolvedRequest::from_item(&item_clean, None).unwrap();
        assert_eq!(resolved_clean.url, "https://api.example.com/search?q=rust");
    }

    #[test]
    fn test_reqwest_executor_client_caching_and_prewarm() {
        let executor = ReqwestExecutor::new();
        // Default client (true, true) is pre-warmed on creation
        assert_eq!(executor.cached_clients_count(), 1);

        // Fetching existing (true, true) does not create a new client
        let _ = executor.get_client(true, true).unwrap();
        assert_eq!(executor.cached_clients_count(), 1);

        // Fetching (false, true) creates and caches client #2
        let _ = executor.get_client(false, true).unwrap();
        assert_eq!(executor.cached_clients_count(), 2);

        // Re-fetching (false, true) uses cached client
        let _ = executor.get_client(false, true).unwrap();
        assert_eq!(executor.cached_clients_count(), 2);

        // Fetching (true, false) and (false, false)
        let _ = executor.get_client(true, false).unwrap();
        assert_eq!(executor.cached_clients_count(), 3);

        let _ = executor.get_client(false, false).unwrap();
        assert_eq!(executor.cached_clients_count(), 4);

        // Re-fetching default client still succeeds
        let _ = executor.get_client(true, true).unwrap();
        assert_eq!(executor.cached_clients_count(), 4);
    }

    #[tokio::test]
    async fn test_reqwest_executor_send_local_http() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server_url = format!("http://127.0.0.1:{}/test", port);

        // Spawn mock HTTP server that handles connections
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    loop {
                        let n = match socket.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        // Simple HTTP response
                        if n > 0 {
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 13\r\nConnection: keep-alive\r\n\r\nHello, World!";
                            if socket.write_all(resp.as_bytes()).await.is_err() {
                                break;
                            }
                        }
                    }
                });
            }
        });

        let executor = ReqwestExecutor::new();

        // Request 1: Cold connection
        let item1 = HttpRequestItem::new("ws-1", "Test Req 1", HttpMethod::Get, &server_url);
        let resolved1 = ResolvedRequest::from_item(&item1, None).unwrap();
        let res1 = executor
            .send(resolved1, None)
            .await
            .expect("send request 1 should succeed");
        assert_eq!(res1.status_code, 200);
        assert_eq!(res1.body, b"Hello, World!");
        assert!(!res1.timing.connection_reused);

        // Request 2: Reused keep-alive connection
        let item2 = HttpRequestItem::new("ws-2", "Test Req 2", HttpMethod::Get, &server_url);
        let resolved2 = ResolvedRequest::from_item(&item2, None).unwrap();
        let res2 = executor
            .send(resolved2, None)
            .await
            .expect("send request 2 should succeed");
        assert_eq!(res2.status_code, 200);
        assert_eq!(res2.body, b"Hello, World!");
        assert!(res2.timing.connection_reused);
        assert_eq!(res2.timing.dns_ms, Some(0));
        assert_eq!(res2.timing.connect_ms, Some(0));
    }

    #[tokio::test]
    async fn test_reqwest_executor_connection_close_not_reused() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server_url = format!("http://127.0.0.1:{}/close", port);

        // Spawn mock server that responds with Connection: close
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let _ = socket.read(&mut buf).await;
                    let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDone";
                    let _ = socket.write_all(resp.as_bytes()).await;
                });
            }
        });

        let executor = ReqwestExecutor::new();

        // Request 1: Connection: close
        let item1 = HttpRequestItem::new("ws-1", "Test Req 1", HttpMethod::Get, &server_url);
        let resolved1 = ResolvedRequest::from_item(&item1, None).unwrap();
        let res1 = executor.send(resolved1, None).await.unwrap();
        assert!(!res1.timing.connection_reused);

        // Request 2: Must not be marked as reused because connection was closed
        let item2 = HttpRequestItem::new("ws-2", "Test Req 2", HttpMethod::Get, &server_url);
        let resolved2 = ResolvedRequest::from_item(&item2, None).unwrap();
        let res2 = executor.send(resolved2, None).await.unwrap();
        assert!(!res2.timing.connection_reused);
    }

    #[test]
    fn test_cookie_store_mutex_poison_recovery() {
        let executor = ReqwestExecutor::new();
        let cookie_store = executor.cookie_store();

        // Intentionally poison the mutex
        let cs_clone = cookie_store.clone();
        let _ = std::panic::catch_unwind(move || {
            let _guard = cs_clone.lock().unwrap();
            panic!("Poisoning cookie store mutex");
        });

        assert!(cookie_store.lock().is_err());

        // Calling get_all_cookies, get_cookies_for_url, or clear_cookies must not panic
        let cookies = executor.get_all_cookies();
        assert!(cookies.is_empty());

        let url_cookies = executor.get_cookies_for_url("https://example.com");
        assert!(url_cookies.is_empty());

        executor.clear_cookies();
    }

    #[test]
    fn test_stored_cookies_roundtrip_and_removal() {
        let executor = ReqwestExecutor::new();
        let sample = vec![
            StoredCookie {
                name: "session_id".to_string(),
                value: "abc123xyz".to_string(),
                domain: "api.example.com".to_string(),
                path: "/".to_string(),
                expires: None,
                secure: true,
                http_only: true,
                same_site: Some("Lax".to_string()),
            },
            StoredCookie {
                name: "theme".to_string(),
                value: "dark".to_string(),
                domain: "app.example.com".to_string(),
                path: "/dashboard".to_string(),
                expires: None,
                secure: false,
                http_only: false,
                same_site: None,
            },
        ];

        executor.restore_stored_cookies(&sample);
        let stored = executor.get_stored_cookies();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].domain, "api.example.com");
        assert_eq!(stored[0].name, "session_id");
        assert_eq!(stored[0].value, "abc123xyz");
        assert!(stored[0].secure);
        assert!(stored[0].http_only);

        executor.remove_cookie("api.example.com", "/", "session_id");
        let after_remove = executor.get_stored_cookies();
        assert_eq!(after_remove.len(), 1);
        assert_eq!(after_remove[0].name, "theme");

        executor.clear_cookies();
        assert!(executor.get_stored_cookies().is_empty());
    }
}
