use chrono::Utc;
use rand::Rng;
use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;
use uuid::Uuid;

use crate::models::{Auth, HttpRequestItem, RequestBody};

/// Catalog of dynamic variables: `(name with $ prefix, description)` pairs.
/// Single source of truth shared by the regex matcher and the UI autocomplete.
pub const FAKE_VAR_CATALOG: &[(&str, &str)] = &[
    ("$guid", "Generates random UUID v4"),
    ("$uuid", "Generates random UUID v4"),
    ("$timestamp", "Current UNIX timestamp in seconds"),
    ("$isoTimestamp", "Current UTC timestamp in ISO 8601"),
    ("$randomInt", "Random integer between 1 and 1000"),
    ("$randomEmail", "Random email e.g. user1234@example.com"),
    ("$randomName", "Random sample full name"),
    ("$randomPhoneNumber", "Random phone number"),
];

static FAKE_VAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    let alternation = FAKE_VAR_CATALOG
        .iter()
        .map(|(name, _)| name.trim_start_matches('$'))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"\{{\{{\$({alternation})\}}\}}")).unwrap()
});

/// Generate dynamic values for fake/mock variables by name (with or without `$` prefix).
pub fn generate_fake_var_value(name: &str) -> Option<String> {
    let stripped = name.strip_prefix('$').unwrap_or(name);
    let mut rng = rand::thread_rng();
    match stripped {
        "uuid" | "guid" => Some(Uuid::new_v4().to_string()),
        "timestamp" => Some(Utc::now().timestamp().to_string()),
        "isoTimestamp" => Some(Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        "randomInt" => Some(rng.gen_range(1u32..=1000).to_string()),
        "randomEmail" => Some(format!("user{}@example.com", rng.gen_range(1000u32..9999))),
        "randomName" => {
            const NAMES: &[&str] = &[
                "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry", "Jack",
                "Kelly", "Leo", "Mia", "Noah", "Olivia", "Peter", "Quinn", "Rose", "Sam",
            ];
            Some(NAMES[rng.gen_range(0..NAMES.len())].to_string())
        }
        "randomPhoneNumber" => Some(format!("+849{:08}", rng.gen_range(0u32..99_999_999))),
        _ => None,
    }
}

/// Replace dynamic mock variables like {{$uuid}}, {{$timestamp}}, etc. in `text`.
/// Must be called BEFORE `interpolate()` to prevent collisions with normal environment variables.
/// Returns `Cow::Borrowed` if text does not contain `{{$`, avoiding heap allocations.
pub fn resolve_fake_vars<'a>(text: &'a str) -> Cow<'a, str> {
    if !text.contains("{{$") {
        return Cow::Borrowed(text);
    }

    FAKE_VAR_RE.replace_all(text, |caps: &regex::Captures| {
        generate_fake_var_value(&caps[1]).unwrap_or_else(|| caps[0].to_string())
    })
}

/// Resolve fake variables in-place on `&mut String`, avoiding re-allocation if `{{$` is absent.
pub fn resolve_fake_vars_in_place(s: &mut String) {
    if !s.contains("{{$") {
        return;
    }
    if let Cow::Owned(new_s) = resolve_fake_vars(s) {
        *s = new_s;
    }
}

/// Resolve all fake variables in an HttpRequestItem, allocating only for fields that contain `{{$`.
pub fn resolve_fake_vars_in_request(req_item: &mut HttpRequestItem) {
    resolve_fake_vars_in_place(&mut req_item.url);
    for p in &mut req_item.params {
        resolve_fake_vars_in_place(&mut p.key);
        resolve_fake_vars_in_place(&mut p.value);
    }
    for h in &mut req_item.headers {
        resolve_fake_vars_in_place(&mut h.key);
        resolve_fake_vars_in_place(&mut h.value);
    }
    match &mut req_item.body {
        RequestBody::Json { content } => resolve_fake_vars_in_place(content),
        RequestBody::Raw { content, .. } => resolve_fake_vars_in_place(content),
        RequestBody::FormUrlEncoded { params } => {
            for p in params {
                resolve_fake_vars_in_place(&mut p.key);
                resolve_fake_vars_in_place(&mut p.value);
            }
        }
        RequestBody::Multipart { items } => {
            for item in items {
                resolve_fake_vars_in_place(&mut item.key);
                resolve_fake_vars_in_place(&mut item.value);
            }
        }
        _ => {}
    }
    match &mut req_item.auth {
        Auth::Bearer { token } => resolve_fake_vars_in_place(token),
        Auth::Basic { username, password } => {
            resolve_fake_vars_in_place(username);
            resolve_fake_vars_in_place(password);
        }
        Auth::ApiKey { key, value, .. } => {
            resolve_fake_vars_in_place(key);
            resolve_fake_vars_in_place(value);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_is_valid_v4() {
        let out = resolve_fake_vars("id={{$uuid}}&guid={{$guid}}");
        assert!(out.starts_with("id="));
        let parts: Vec<&str> = out.split('&').collect();
        let u1 = parts[0].trim_start_matches("id=");
        let u2 = parts[1].trim_start_matches("guid=");
        assert!(Uuid::parse_str(u1).is_ok());
        assert!(Uuid::parse_str(u2).is_ok());
        assert_ne!(u1, u2);
    }

    #[test]
    fn test_timestamp_is_integer() {
        let out = resolve_fake_vars("ts={{$timestamp}}");
        let ts_str = out.trim_start_matches("ts=");
        let val = ts_str.parse::<i64>();
        assert!(val.is_ok());
        assert!(val.unwrap() > 1_600_000_000);
    }

    #[test]
    fn test_iso_timestamp_format() {
        let out = resolve_fake_vars("{{$isoTimestamp}}");
        assert!(chrono::DateTime::parse_from_rfc3339(&out).is_ok());
    }

    #[test]
    fn test_no_fake_var_unchanged() {
        let original = "https://api.example.com/{{base_url}}/items?q={{search}}";
        let resolved = resolve_fake_vars(original);
        assert_eq!(resolved, original);
        // Verify zero heap allocation (Cow::Borrowed)
        assert!(matches!(resolved, Cow::Borrowed(_)));
    }

    #[test]
    fn test_random_int_and_email() {
        let out = resolve_fake_vars("num={{$randomInt}}&email={{$randomEmail}}");
        assert!(out.contains("@example.com"));
    }

    #[test]
    fn test_fake_var_catalog_names_generate() {
        for (name, desc) in FAKE_VAR_CATALOG {
            assert!(!desc.is_empty(), "empty description for {name}");
            assert!(
                generate_fake_var_value(name).is_some(),
                "no generator for {name}"
            );
        }
    }

    #[test]
    fn test_resolve_fake_vars_in_request() {
        use crate::models::{HttpMethod, KeyValuePair};
        let mut item = HttpRequestItem::new(
            "ws1",
            "Test",
            HttpMethod::Get,
            "https://api.example.com/users?uid={{$uuid}}",
        );
        item.headers
            .push(KeyValuePair::new("X-Request-Id", "{{$guid}}"));
        item.headers
            .push(KeyValuePair::new("Content-Type", "application/json"));
        resolve_fake_vars_in_request(&mut item);
        assert!(!item.url.contains("{{$uuid}}"));
        assert!(!item.headers[0].value.contains("{{$guid}}"));
        assert_eq!(item.headers[1].value, "application/json");
    }
}
