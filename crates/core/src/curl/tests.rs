use super::*;

#[test]
fn test_is_curl_command() {
    assert!(is_curl_command("curl https://example.com"));
    assert!(is_curl_command("$ curl 'https://example.com'"));
    assert!(is_curl_command("> curl -H 'accept: json' https://api.com"));
    assert!(is_curl_command("curl\n -X GET https://api.com"));
    assert!(!is_curl_command("https://example.com"));
    assert!(!is_curl_command("curl://example.com"));
    assert!(!is_curl_command("curling"));
    // `curl.exe` is deliberately not treated as an importable command.
    assert!(!is_curl_command("curl.exe -X POST https://example.com"));
    assert!(!is_curl_command(""));
}

/// Regression: the URL change handler runs `is_curl_command` on every
/// keystroke. A URL typed with a Vietnamese IME starts with multi-byte
/// characters, and byte-slicing the 4/8-char prefixes used to panic ("end
/// byte index 4 is not a char boundary"), which unwound through the
/// window procedure and aborted the whole app (STATUS_STACK_BUFFER_OVERRUN).
#[test]
fn test_is_curl_command_multibyte_prefix_never_panics() {
    // Byte 4 lands inside 'á' (bytes 3..5).
    assert!(!is_curl_command("há á độ"));
    // Byte 4 lands inside 'ộ' (bytes 3..5).
    assert!(!is_curl_command("ộếạ"));
    // "curl.exá" reads as the plain "curl" prefix followed by ".exá",
    // which is not a valid command tail.
    assert!(!is_curl_command("curl.exá"));
    // Multi-byte text everywhere, ASCII "curl" still detected.
    assert!(is_curl_command("curl https://há.ộ/api"));
    assert!(is_curl_command("  curl https://example.com  "));
    assert!(!is_curl_command("ácurl"));
    assert!(!is_curl_command("ế"));
}

#[test]
fn test_parse_curl_post() {
    let curl = r#"curl -X POST "https://api.example.com/v1/users" -H "Content-Type: application/json" -H "Authorization: Bearer mytoken" -d '{"name": "Alice"}'"#;
    let req = curl_to_request("ws1", curl).unwrap();
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.url, "https://api.example.com/v1/users");
    assert_eq!(
        req.auth,
        Auth::Bearer {
            token: "mytoken".to_string()
        }
    );
    assert_eq!(
        req.body,
        RequestBody::Json {
            content: "{\"name\": \"Alice\"}".to_string()
        }
    );
}

#[test]
fn test_parse_curl_devtools_bash_multiline() {
    let curl = "curl 'https://httpbin.org/get?page=1&limit=20' \\\n  -H 'accept: application/json' \\\n  -H 'user-agent: Mozilla/5.0' \\\n  --compressed";
    let req = curl_to_request("ws1", curl).unwrap();
    assert_eq!(req.method, HttpMethod::Get);
    assert_eq!(req.url, "https://httpbin.org/get");
    assert_eq!(req.params.len(), 2);
    assert_eq!(req.params[0].key, "page");
    assert_eq!(req.params[0].value, "1");
    assert_eq!(req.params[1].key, "limit");
    assert_eq!(req.params[1].value, "20");
    assert!(req
        .headers
        .iter()
        .any(|h| h.key == "accept" && h.value == "application/json"));
    assert!(req
        .headers
        .iter()
        .any(|h| h.key == "user-agent" && h.value == "Mozilla/5.0"));
}

#[test]
fn test_parse_curl_windows_cmd() {
    let curl = "curl ^\n  \"https://httpbin.org/post\" ^\n  -H \"Content-Type: application/json\" ^\n  -d \"{\\\"key\\\": \\\"val\\\"}\"";
    let req = curl_to_request("ws1", curl).unwrap();
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.url, "https://httpbin.org/post");
    assert_eq!(
        req.body,
        RequestBody::Json {
            content: "{\"key\": \"val\"}".to_string()
        }
    );
}

#[test]
fn test_parse_curl_powershell_backtick() {
    let curl = "curl --location 'https://httpbin.org/post' `\n  --header 'Content-Type: application/json' `\n  --data '{\"ps\": 1}'";
    let req = curl_to_request("ws1", curl).unwrap();
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.url, "https://httpbin.org/post");
    assert_eq!(
        req.body,
        RequestBody::Json {
            content: "{\"ps\": 1}".to_string()
        }
    );
}

/// `curl.exe` is not an importable command: the parser must reject it,
/// mirroring `is_curl_command`.
#[test]
fn test_parse_curl_rejects_curl_exe() {
    let result = curl_to_request("ws1", "curl.exe -X POST https://example.com");
    assert!(result.is_err(), "curl.exe must not parse as a curl command");
}

#[test]
fn test_parse_curl_basic_auth() {
    let curl = "curl -u admin:secret123 https://httpbin.org/basic-auth/admin/secret123";
    let req = curl_to_request("ws1", curl).unwrap();
    assert_eq!(
        req.auth,
        Auth::Basic {
            username: "admin".to_string(),
            password: "secret123".to_string()
        }
    );
}

#[test]
fn test_parse_curl_flags_before_url() {
    let curl =
        "curl -A 'MyBrowser/1.0' -b 'session=abc' --connect-timeout 30 https://example.com/api";
    let req = curl_to_request("ws1", curl).unwrap();
    assert_eq!(req.url, "https://example.com/api");
    assert!(req
        .headers
        .iter()
        .any(|h| h.key == "User-Agent" && h.value == "MyBrowser/1.0"));
    assert!(req
        .headers
        .iter()
        .any(|h| h.key == "Cookie" && h.value == "session=abc"));
}

#[test]
fn test_export_curl() {
    let mut req = RequestItem::new("ws1", "Test", HttpMethod::Get, "https://httpbin.org/get");
    req.headers
        .push(KeyValuePair::new("Accept", "application/json"));
    let exported = request_to_curl(&req, None);
    assert!(exported.contains("curl -X GET 'https://httpbin.org/get'"));
    assert!(exported.contains("-H 'Accept: application/json'"));
}
