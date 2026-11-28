use zest_core::models::RequestItem;

#[cfg(test)]
pub const MAX_DEFAULT_DOMAIN_LEN: usize = 28;
pub const MAX_DISPLAY_TITLE_LEN: usize = 36;

pub fn truncate_text(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() > max_chars {
        let truncated: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        trimmed.to_string()
    }
}

/// Extracts the domain (host) from a URL and limits its length.
#[cfg(test)]
pub fn default_name_from_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "Untitled Request".to_string();
    }

    let domain = if let Ok(parsed) = url::Url::parse(trimmed) {
        parsed.host_str().unwrap_or("").to_string()
    } else {
        let without_scheme = trimmed
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        without_scheme
            .split(['/', '?', '#', ':'])
            .next()
            .unwrap_or("")
            .to_string()
    };

    let domain = domain.trim();
    if domain.is_empty() {
        return "Untitled Request".to_string();
    }

    truncate_text(domain, MAX_DEFAULT_DOMAIN_LEN)
}

/// Returns the display name of a request (custom name if set, otherwise "Untitled Request"),
/// capped at `MAX_DISPLAY_TITLE_LEN` characters.
pub fn effective_request_name(req: &RequestItem) -> String {
    effective_request_name_str(&req.name)
}

pub fn effective_request_name_str(raw_name: &str) -> String {
    let raw = raw_name.trim();
    if raw.is_empty() {
        "Untitled Request".to_string()
    } else {
        truncate_text(raw, MAX_DISPLAY_TITLE_LEN)
    }
}

#[cfg(test)]
#[path = "request_label_tests.rs"]
mod tests;
