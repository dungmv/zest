use super::*;
use crate::models::{EnvironmentVariable, KeyValuePair};

#[test]
fn test_pre_request_env_and_headers() {
    let env_vars = vec![EnvironmentVariable {
        key: "token".to_string(),
        initial_value: "old_token".to_string(),
        current_value: "".to_string(),
        secret: false,
    }];

    let ctx_data = PreRequestContext {
        env_vars,
        request_headers: vec![KeyValuePair::new("User-Agent", "Zest")],
        request_body_json: Some(r#"{"count": 1}"#.to_string()),
    };

    let script = r#"
        // Read env
        const t = z.environment.get("token");
        z.assert(t === "old_token", "Token should match");

        // Mutate env
        z.environment.set("token", "new_token_123");
        z.environment.set("computed", 42);

        // Add dynamic header
        z.request.headers.add("X-Custom-Header", "Value123");

        // Mutate body
        const body = z.request.body.json();
        body.count = body.count + 5;
        body.injected = true;
        z.request.body.setJson(body);

        console.log("Pre-request finished with token:", z.environment.get("token"));
    "#;

    let output = ScriptSandbox::run_pre_request(script, ctx_data);
    assert!(output.error.is_none(), "Error: {:?}", output.error);

    // Verify env mutations
    assert_eq!(output.env_mutations.len(), 2);
    assert_eq!(
        output.env_mutations[0],
        ("token".to_string(), Some("new_token_123".to_string()))
    );
    assert_eq!(
        output.env_mutations[1],
        ("computed".to_string(), Some("42".to_string()))
    );

    // Verify header mutations
    assert_eq!(output.header_mutations.len(), 1);
    assert_eq!(
        output.header_mutations[0],
        ("X-Custom-Header".to_string(), "Value123".to_string())
    );

    // Verify body override
    assert!(output.body_override.is_some());
    let new_body = output.body_override.unwrap();
    assert!(new_body.contains(r#""count":6"#));
    assert!(new_body.contains(r#""injected":true"#));

    // Verify console logs
    assert_eq!(output.console_logs.len(), 1);
    assert!(output.console_logs[0].contains("Pre-request finished with token: new_token_123"));
}

#[test]
fn test_post_response_test_assertions_and_chaining() {
    let resp_body =
        r#"{"id": 100, "status": "active", "access_token": "jwt_secret_token"}"#.to_string();
    let ctx_data = PostResponseContext {
        env_vars: vec![],
        status_code: 200,
        response_time_ms: 125,
        response_headers: vec![("content-type".to_string(), "application/json".to_string())],
        response_body: resp_body,
    };

    let script = r#"
        // Test assertions
        z.test("Status code is 200", () => {
            z.assert(z.response.status === 200);
        });

        z.test("Response time is under 500ms", () => {
            z.assert(z.response.time < 500);
        });

        z.test("Status is active", () => {
            const j = z.response.json();
            z.assert(j.status === "active", "Status must be active");
        });

        // Request chaining: extract access token to environment
        const data = z.response.json();
        z.environment.set("access_token", data.access_token);
    "#;

    let output = ScriptSandbox::run_post_response(script, ctx_data);
    assert!(output.error.is_none(), "Error: {:?}", output.error);

    // All 3 tests must pass
    assert_eq!(output.test_results.len(), 3);
    assert!(output.test_results[0].passed);
    assert_eq!(output.test_results[0].name, "Status code is 200");
    assert!(output.test_results[1].passed);
    assert!(output.test_results[2].passed);

    // Env mutation for request chaining
    assert_eq!(output.env_mutations.len(), 1);
    assert_eq!(
        output.env_mutations[0],
        (
            "access_token".to_string(),
            Some("jwt_secret_token".to_string())
        )
    );
}

#[test]
fn test_post_response_failing_assertion() {
    let ctx_data = PostResponseContext {
        env_vars: vec![],
        status_code: 404,
        response_time_ms: 80,
        response_headers: vec![],
        response_body: "Not Found".to_string(),
    };

    let script = r#"
        z.test("Should fail assertion", () => {
            z.assert(z.response.status === 200, "Expected 200 OK");
        });
    "#;

    let output = ScriptSandbox::run_post_response(script, ctx_data);
    assert_eq!(output.test_results.len(), 1);
    assert!(!output.test_results[0].passed);
    assert!(output.test_results[0]
        .error_message
        .as_ref()
        .unwrap()
        .contains("Expected 200 OK"));
}

#[test]
fn test_crypto_utilities() {
    let ctx_data = PreRequestContext::default();
    let script = r#"
        const sig = z.crypto.hmacSha256("my_secret", "payload_data");
        z.environment.set("hmac", sig);

        const md5 = z.crypto.md5("hello world");
        z.environment.set("md5", md5);

        const b64 = z.crypto.base64Encode("Antigravity Client");
        z.environment.set("b64", b64);
        const dec = z.crypto.base64Decode(b64);
        z.assert(dec === "Antigravity Client");
    "#;

    let output = ScriptSandbox::run_pre_request(script, ctx_data);
    assert!(output.error.is_none(), "Error: {:?}", output.error);

    let hmac = output
        .env_mutations
        .iter()
        .find(|(k, _)| k == "hmac")
        .unwrap()
        .1
        .as_ref()
        .unwrap();
    assert_eq!(hmac.len(), 64); // 256-bit hex

    let md5_val = output
        .env_mutations
        .iter()
        .find(|(k, _)| k == "md5")
        .unwrap()
        .1
        .as_ref()
        .unwrap();
    assert_eq!(md5_val, "5eb63bbbe01eeed093cb22bb8f5acdc3"); // known md5 of "hello world"
}

#[test]
fn test_z_primary() {
    let ctx_data = PreRequestContext::default();
    let script = r#"
        z.environment.set("from_z", "success");
    "#;

    let output = ScriptSandbox::run_pre_request(script, ctx_data);
    assert!(output.error.is_none());
    assert_eq!(
        output.env_mutations[0],
        ("from_z".to_string(), Some("success".to_string()))
    );
}

#[test]
fn test_console_logging_levels() {
    let ctx_data = PreRequestContext::default();
    let script = r#"
        console.log("standard log", 123);
        console.info("info log");
        console.warn("warning log");
        console.error("error log");
    "#;

    let output = ScriptSandbox::run_pre_request(script, ctx_data);
    assert!(output.error.is_none());
    assert_eq!(output.console_entries.len(), 4);
    assert_eq!(output.console_entries[0].level, ConsoleLogLevel::Log);
    assert_eq!(output.console_entries[0].message, "standard log 123");
    assert_eq!(
        output.console_entries[0].source,
        ConsoleLogSource::PreRequest
    );

    assert_eq!(output.console_entries[1].level, ConsoleLogLevel::Info);
    assert_eq!(output.console_entries[1].message, "info log");

    assert_eq!(output.console_entries[2].level, ConsoleLogLevel::Warn);
    assert_eq!(output.console_entries[2].message, "warning log");

    assert_eq!(output.console_entries[3].level, ConsoleLogLevel::Error);
    assert_eq!(output.console_entries[3].message, "error log");
}
