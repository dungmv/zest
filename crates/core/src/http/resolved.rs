use super::executor::HttpError;
use crate::models::{
    Auth, Environment, HttpMethod, HttpRequestItem, KeyValuePair, MultipartItem, RequestBody,
    RequestSentInfo, RequestSettings,
};

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
        let known_scheme = ["http://", "https://", "ws://", "wss://", "grpc://", "grpcs://"]
            .iter()
            .any(|s| base_interpolated.starts_with(s));
        let mut full_url = if known_scheme {
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
