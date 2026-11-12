use crate::models::{
    Auth, Environment, HttpMethod, HttpRequestItem, KeyValuePair, MultipartItem, RequestBody,
};
use anyhow::{anyhow, Result};
use base64::Engine as _;
use std::str::FromStr;

/// Check if a string looks like a cURL command
pub fn is_curl_command(s: &str) -> bool {
    let mut trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Strip leading terminal prompts like '$ ' or '> ' or 'ps> '
    if trimmed.starts_with('$') || trimmed.starts_with('>') {
        trimmed = trimmed[1..].trim_start();
    }
    // The prefix check uses `get` instead of byte slicing: with an IME-typed
    // URL (e.g. Vietnamese) byte 4 can fall inside a multi-byte character,
    // and slicing there panics — which, fired from the per-keystroke URL
    // change handler, unwinds through the window procedure and aborts the
    // app. A non-boundary prefix simply means "not a curl command".
    if trimmed
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("curl"))
    {
        let rest = &trimmed[4..];
        return rest.is_empty()
            || rest.starts_with(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '-');
    }
    false
}

/// Sanitize cURL command by removing shell line continuations (\, ^, `)
/// and cmd-specific quote escapes (^").
fn sanitize_curl_command(curl_str: &str) -> String {
    let mut s = curl_str.trim();
    if s.starts_with('$') || s.starts_with('>') {
        s = s[1..].trim_start();
    }
    let mut result = String::new();
    for line in s.lines() {
        let trimmed_end = line.trim_end();
        if let Some(without_slash) = trimmed_end.strip_suffix('\\') {
            result.push_str(without_slash.trim_end());
            result.push(' ');
        } else if let Some(without_caret) = trimmed_end.strip_suffix('^') {
            result.push_str(without_caret.trim_end());
            result.push(' ');
        } else if let Some(without_backtick) = trimmed_end.strip_suffix('`') {
            result.push_str(without_backtick.trim_end());
            result.push(' ');
        } else {
            result.push_str(trimmed_end);
            result.push('\n');
        }
    }
    result.replace("^\"", "\"")
}

/// Parse a cURL command into an HttpRequestItem
pub fn curl_to_request(workspace_id: &str, curl_str: &str) -> Result<HttpRequestItem> {
    let clean = sanitize_curl_command(curl_str);
    let tokens = tokenize_command(&clean)?;

    if tokens.is_empty() {
        return Err(anyhow!("Empty command"));
    }

    // Only the plain `curl` command is importable; `curl.exe` is rejected so
    // the parser stays consistent with `is_curl_command`.
    let start_idx = if tokens[0].eq_ignore_ascii_case("curl") {
        1
    } else {
        return Err(anyhow!(
            "Not a curl command (expected it to start with 'curl')"
        ));
    };

    let mut method = HttpMethod::Get;
    let mut method_explicit = false;
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut body_data: Option<String> = None;
    let mut multipart_items = Vec::new();
    let mut auth = Auth::None;

    let mut idx = start_idx;
    while idx < tokens.len() {
        let token = &tokens[idx];

        // 1. HTTP Method
        if token == "-X" || token == "--request" {
            idx += 1;
            if idx < tokens.len() {
                if let Ok(m) = HttpMethod::from_str(&tokens[idx]) {
                    method = m;
                    method_explicit = true;
                }
            }
        } else if token == "-I" || token == "--head" {
            method = HttpMethod::Head;
            method_explicit = true;
        }
        // 2. Headers
        else if token == "-H" || token == "--header" {
            idx += 1;
            if idx < tokens.len() {
                let h_str = &tokens[idx];
                if let Some((k, v)) = h_str.split_once(':') {
                    let k = k.trim().to_string();
                    let v = v.trim().to_string();
                    if k.eq_ignore_ascii_case("authorization") {
                        if v.starts_with("Bearer ") {
                            auth = Auth::Bearer {
                                token: v.trim_start_matches("Bearer ").trim().to_string(),
                            };
                        } else if v.starts_with("Basic ") {
                            let encoded = v.trim_start_matches("Basic ").trim();
                            if let Ok(decoded_bytes) =
                                base64::engine::general_purpose::STANDARD.decode(encoded)
                            {
                                if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                                    if let Some((user, pass)) = decoded_str.split_once(':') {
                                        auth = Auth::Basic {
                                            username: user.to_string(),
                                            password: pass.to_string(),
                                        };
                                    } else {
                                        headers.push(KeyValuePair::new(k, v));
                                    }
                                } else {
                                    headers.push(KeyValuePair::new(k, v));
                                }
                            } else {
                                headers.push(KeyValuePair::new(k, v));
                            }
                        } else {
                            headers.push(KeyValuePair::new(k, v));
                        }
                    } else {
                        headers.push(KeyValuePair::new(k, v));
                    }
                }
            }
        }
        // 3. User-Agent
        else if token == "-A" || token == "--user-agent" {
            idx += 1;
            if idx < tokens.len() {
                headers.push(KeyValuePair::new("User-Agent", &tokens[idx]));
            }
        }
        // 4. Cookie
        else if token == "-b" || token == "--cookie" {
            idx += 1;
            if idx < tokens.len() {
                headers.push(KeyValuePair::new("Cookie", &tokens[idx]));
            }
        }
        // 5. Referer
        else if token == "-e" || token == "--referer" {
            idx += 1;
            if idx < tokens.len() {
                headers.push(KeyValuePair::new("Referer", &tokens[idx]));
            }
        }
        // 6. Explicit URL
        else if token == "--url" {
            idx += 1;
            if idx < tokens.len() {
                url = tokens[idx].clone();
            }
        }
        // 7. Data / Body
        else if token == "-d"
            || token == "--data"
            || token == "--data-raw"
            || token == "--data-binary"
            || token == "--data-ascii"
            || token == "--data-urlencode"
        {
            idx += 1;
            if idx < tokens.len() {
                let data = &tokens[idx];
                if let Some(existing) = &mut body_data {
                    existing.push('&');
                    existing.push_str(data);
                } else {
                    body_data = Some(data.clone());
                }
                if !method_explicit {
                    method = HttpMethod::Post;
                }
            }
        }
        // 8. Form multipart
        else if token == "-F" || token == "--form" {
            idx += 1;
            if idx < tokens.len() {
                let form_str = &tokens[idx];
                if let Some((field, val)) = form_str.split_once('=') {
                    if let Some(file_path) = val.strip_prefix('@') {
                        multipart_items
                            .push(MultipartItem::new_file(field.trim(), file_path.trim()));
                    } else {
                        multipart_items.push(MultipartItem::new_text(field.trim(), val.trim()));
                    }
                }
                if !method_explicit {
                    method = HttpMethod::Post;
                }
            }
        }
        // 9. Auth basic
        else if token == "-u" || token == "--user" {
            idx += 1;
            if idx < tokens.len() {
                let u_str = &tokens[idx];
                let (user, pass) = u_str.split_once(':').unwrap_or((u_str, ""));
                auth = Auth::Basic {
                    username: user.to_string(),
                    password: pass.to_string(),
                };
            }
        }
        // 10. Known flags with an argument to skip
        else if token == "--connect-timeout"
            || token == "--max-time"
            || token == "-m"
            || token == "-o"
            || token == "--output"
            || token == "--proxy"
            || token == "-x"
            || token == "--cacert"
            || token == "--cert"
            || token == "--key"
            || token == "--resolve"
            || token == "--proto"
            || token == "--interface"
            || token == "--retry"
            || token == "--limit-rate"
        {
            idx += 1; // skip argument
        }
        // 11. Known boolean flags without arguments (skip)
        else if token == "-k"
            || token == "--insecure"
            || token == "-L"
            || token == "--location"
            || token == "-s"
            || token == "--silent"
            || token == "-S"
            || token == "--show-error"
            || token == "-v"
            || token == "--verbose"
            || token == "-i"
            || token == "--include"
            || token == "--compressed"
            || token == "--globoff"
            || token == "-g"
            || token == "-f"
            || token == "--fail"
            || token == "-N"
            || token == "--no-buffer"
            || token == "-O"
            || token == "--remote-name"
        {
            // do nothing
        }
        // 12. Positional URL
        else if !token.starts_with('-') && url.is_empty() {
            url = token.clone();
        }

        idx += 1;
    }

    if url.is_empty() {
        return Err(anyhow!("No URL found in cURL command"));
    }

    // Determine request body
    let body = if !multipart_items.is_empty() {
        RequestBody::Multipart {
            items: multipart_items,
        }
    } else if let Some(data) = body_data {
        let is_json = (data.trim().starts_with('{') && data.trim().ends_with('}'))
            || (data.trim().starts_with('[') && data.trim().ends_with(']'));
        let is_form = headers.iter().any(|h| {
            h.key.eq_ignore_ascii_case("content-type")
                && h.value.contains("application/x-www-form-urlencoded")
        });

        if is_json {
            RequestBody::Json { content: data }
        } else if is_form {
            let mut pairs = Vec::new();
            for part in data.split('&') {
                if let Some((k, v)) = part.split_once('=') {
                    let dec_k = urlencoding::decode(k).unwrap_or(k.into()).into_owned();
                    let dec_v = urlencoding::decode(v).unwrap_or(v.into()).into_owned();
                    pairs.push(KeyValuePair::new(dec_k, dec_v));
                }
            }
            RequestBody::FormUrlEncoded { params: pairs }
        } else {
            let ct = headers
                .iter()
                .find(|h| h.key.eq_ignore_ascii_case("content-type"))
                .map(|h| h.value.clone())
                .unwrap_or_else(|| "text/plain".to_string());
            RequestBody::Raw {
                content: data,
                content_type: ct,
            }
        }
    } else {
        RequestBody::None
    };

    let (clean_url, parsed_params) = crate::http::parse_query_from_url(&url);

    let name = clean_url
        .trim_end_matches('/')
        .split('/')
        .rfind(|s| !s.is_empty() && !s.contains(':'))
        .unwrap_or("Imported Request")
        .to_string();

    let mut req = HttpRequestItem::new(workspace_id, name, method, clean_url);
    req.params = parsed_params;
    req.headers = headers;
    req.auth = auth;
    req.body = body;

    Ok(req)
}

/// Export an HttpRequestItem to a standard cURL command string
pub fn request_to_curl(item: &HttpRequestItem, env: Option<&Environment>) -> String {
    let mut parts = vec!["curl".to_string()];

    // Method
    parts.push("-X".to_string());
    parts.push(item.method.as_str().to_string());

    // URL & Interpolator
    let interpolator = crate::interpolation::VariableInterpolator::new(env);
    let full_url = crate::http::build_url_with_query(&item.url, &item.params);
    let url = interpolator.interpolate_cow(&full_url);
    parts.push(format!("'{}'", url.replace('\'', "'\\''")));

    // Headers
    for h in &item.headers {
        if h.enabled && !h.key.trim().is_empty() {
            let k = interpolator.interpolate_cow(&h.key);
            let v = interpolator.interpolate_cow(&h.value);
            parts.push("-H".to_string());
            parts.push(format!(
                "'{}: {}'",
                k.replace('\'', "'\\''"),
                v.replace('\'', "'\\''")
            ));
        }
    }

    // Auth
    match &item.auth {
        Auth::Inherit | Auth::None => {}
        Auth::Bearer { token } => {
            let t = interpolator.interpolate_cow(token);
            parts.push("-H".to_string());
            parts.push(format!(
                "'Authorization: Bearer {}'",
                t.replace('\'', "'\\''")
            ));
        }
        Auth::Basic { username, password } => {
            let u = interpolator.interpolate_cow(username);
            let p = interpolator.interpolate_cow(password);
            parts.push("-u".to_string());
            parts.push(format!(
                "'{}:{}'",
                u.replace('\'', "'\\''"),
                p.replace('\'', "'\\''")
            ));
        }
        Auth::ApiKey { key, value, add_to } => {
            if add_to.is_header() && !key.trim().is_empty() {
                let k = interpolator.interpolate_cow(key);
                let v = interpolator.interpolate_cow(value);
                parts.push("-H".to_string());
                parts.push(format!(
                    "'{}: {}'",
                    k.replace('\'', "'\\''"),
                    v.replace('\'', "'\\''")
                ));
            }
        }
    }

    // Body
    match &item.body {
        RequestBody::None => {}
        RequestBody::Json { content: raw_json } => {
            let interpolated = interpolator.interpolate_cow(raw_json);
            parts.push("-H".to_string());
            parts.push("'Content-Type: application/json'".to_string());
            parts.push("-d".to_string());
            parts.push(format!("'{}'", interpolated.replace('\'', "'\\''")));
        }
        RequestBody::Raw {
            content,
            content_type,
        } => {
            let interpolated = interpolator.interpolate_cow(content);
            if !content_type.trim().is_empty() {
                parts.push("-H".to_string());
                parts.push(format!(
                    "'Content-Type: {}'",
                    content_type.replace('\'', "'\\''")
                ));
            }
            parts.push("-d".to_string());
            parts.push(format!("'{}'", interpolated.replace('\'', "'\\''")));
        }
        RequestBody::FormUrlEncoded { params: pairs } => {
            for p in pairs {
                if p.enabled && !p.key.trim().is_empty() {
                    let k = interpolator.interpolate_cow(&p.key);
                    let v = interpolator.interpolate_cow(&p.value);
                    parts.push("--data-urlencode".to_string());
                    parts.push(format!(
                        "'{}={}'",
                        k.replace('\'', "'\\''"),
                        v.replace('\'', "'\\''")
                    ));
                }
            }
        }
        _ => {}
    }

    parts.join(" ")
}

fn tokenize_command(cmd: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = cmd.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' if !in_single_quote => {
                if let Some(next) = chars.next() {
                    match next {
                        'n' if in_double_quote => current.push('\n'),
                        'r' if in_double_quote => current.push('\r'),
                        't' if in_double_quote => current.push('\t'),
                        _ => current.push(next),
                    }
                }
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '$' if !in_single_quote && !in_double_quote && chars.peek() == Some(&'\'') => {
                // Bash ANSI-C quoting: $'...'
                // Skip the '$', the next char '\'' will toggle in_single_quote!
            }
            ' ' | '\t' | '\n' | '\r' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests;
