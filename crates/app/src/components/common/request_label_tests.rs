use super::*;
use zest_core::models::HttpMethod;

#[test]
fn test_default_name_from_url() {
    assert_eq!(
        default_name_from_url("https://speedtest.vn/get-servers?loc=vn"),
        "speedtest.vn"
    );
    assert_eq!(
        default_name_from_url("http://api.github.com/users/octocat"),
        "api.github.com"
    );
    assert_eq!(
        default_name_from_url("https://localhost:8080/api/v1"),
        "localhost"
    );
    assert_eq!(
        default_name_from_url("speedtest.vn/get-servers"),
        "speedtest.vn"
    );
    assert_eq!(default_name_from_url(""), "Untitled Request");
    assert_eq!(default_name_from_url("   "), "Untitled Request");
}

#[test]
fn test_domain_length_truncation() {
    let long_url =
        "https://very-long-subdomain-name-that-exceeds-maximum-allowed-length.example.com/api";
    let name = default_name_from_url(long_url);
    assert!(name.chars().count() <= MAX_DEFAULT_DOMAIN_LEN);
    assert!(name.ends_with('…'));
}

#[test]
fn test_effective_request_name() {
    let mut req = RequestItem::new(
        "test",
        "New Request",
        HttpMethod::Get,
        "https://speedtest.vn/api",
    );
    assert_eq!(effective_request_name(&req), "New Request");

    req.name = "Get Speedtest Servers".to_string();
    assert_eq!(effective_request_name(&req), "Get Speedtest Servers");

    req.name = "".to_string();
    assert_eq!(effective_request_name(&req), "Untitled Request");

    req.name = "   ".to_string();
    assert_eq!(effective_request_name(&req), "Untitled Request");
}
