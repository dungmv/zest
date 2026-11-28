use super::*;
use crate::models::EnvironmentVariable;

fn make_var(key: &str, initial: &str, current: &str, secret: bool) -> EnvironmentVariable {
    EnvironmentVariable {
        key: key.to_string(),
        initial_value: initial.to_string(),
        current_value: current.to_string(),
        secret,
    }
}

#[test]
fn test_resolve_variable_precedence_and_secret_masking() {
    let mut globals = Environment::new("ws", "Globals");
    globals
        .variables
        .push(make_var("host", "global.example.com", "", false));
    globals
        .variables
        .push(make_var("apiKey", "secret-global", "", true));

    let mut active = Environment::new("ws", "Staging");
    active
        .variables
        .push(make_var("host", "staging.example.com", "", false));
    // Duplicate key resolves first-wins
    active
        .variables
        .push(make_var("host", "shadowed.example.com", "", false));
    active
        .variables
        .push(make_var("token", "tok-init", "tok-cur", true));

    // Active overrides Global & first-wins
    let host = resolve_variable(Some(&active), &globals, "host");
    assert_eq!(host.value, "staging.example.com");
    assert_eq!(host.scope, VariableScope::Active("Staging".to_string()));
    assert!(host.is_resolved());
    assert!(!host.is_secret);

    // Active secret is masked
    let token = resolve_variable(Some(&active), &globals, "token");
    assert_eq!(token.value, SECRET_MASK);
    assert!(token.is_secret);
    assert!(token.is_resolved());

    // Global fallback with secret masking
    let api_key = resolve_variable(Some(&active), &globals, "apiKey");
    assert_eq!(api_key.value, SECRET_MASK);
    assert_eq!(api_key.scope, VariableScope::Global);
    assert!(api_key.is_secret);

    // Dynamic variable
    let guid = resolve_variable(Some(&active), &globals, "$guid");
    assert_eq!(guid.scope, VariableScope::Dynamic);
    assert!(guid.value.starts_with("Dynamic: "));
    assert!(guid.is_resolved());

    // Unknown dynamic variable falls back to Missing
    let unknown_dyn = resolve_variable(Some(&active), &globals, "$not_a_real_fake");
    assert_eq!(unknown_dyn.scope, VariableScope::Missing);
    assert!(!unknown_dyn.is_resolved());

    // Missing variable
    let missing = resolve_variable(Some(&active), &globals, "doesNotExist");
    assert_eq!(missing.scope, VariableScope::Missing);
    assert_eq!(missing.value, "Unresolved variable");
    assert!(!missing.is_resolved());
}

#[test]
fn test_list_candidates_shadowing_and_filtering() {
    let mut globals = Environment::new("ws", "Globals");
    globals
        .variables
        .push(make_var("baseUrl", "https://global.test", "", false));
    globals
        .variables
        .push(make_var("globalKey", "gk-123", "", true));

    let mut active = Environment::new("ws", "Dev");
    active
        .variables
        .push(make_var("baseUrl", "https://dev.test", "", false));

    // Active shadows Global for baseUrl
    let base_cands = list_candidates(Some(&active), &globals, "base");
    let base_matches: Vec<_> = base_cands.iter().filter(|c| c.name == "baseUrl").collect();
    assert_eq!(base_matches.len(), 1);
    assert_eq!(base_matches[0].value, "https://dev.test");
    assert_eq!(
        base_matches[0].scope,
        VariableScope::Active("Dev".to_string())
    );

    // Global secret candidate masked
    let g_cands = list_candidates(Some(&active), &globals, "global");
    assert!(g_cands
        .iter()
        .any(|c| c.name == "globalKey" && c.value == SECRET_MASK && c.is_secret));

    // Dynamic variable match without leading '$'
    let d_cands = list_candidates(Some(&active), &globals, "guid");
    assert!(d_cands.iter().any(|c| c.name == "$guid"
        && c.scope == VariableScope::Dynamic
        && c.description.as_deref() == Some("Generates random UUID v4")));
}

#[test]
fn test_find_variable_span_at_boundaries_and_utf8() {
    // Multi-byte UTF-8 prefix ("xin chào ") before {{bookingId}}
    let text = "{\"msg\": \"xin chào {{bookingId}}\", \"n\": {{limit}}}";
    let start = text.find("{{bookingId}}").unwrap();
    let end = start + "{{bookingId}}".len();

    // Before span
    assert_eq!(find_variable_span_at(text, start - 1), None);
    // At opening brace
    assert_eq!(
        find_variable_span_at(text, start).map(|s| s.name),
        Some("bookingId".to_string())
    );
    // Inside span
    assert_eq!(
        find_variable_span_at(text, start + 5).map(|s| s.name),
        Some("bookingId".to_string())
    );
    // At last byte of closing brace
    assert_eq!(
        find_variable_span_at(text, end - 1).map(|s| s.name),
        Some("bookingId".to_string())
    );
    // Immediately after span
    assert_eq!(find_variable_span_at(text, end), None);

    // Second variable on same line
    let limit_start = text.find("{{limit}}").unwrap();
    assert_eq!(
        find_variable_span_at(text, limit_start + 2).map(|s| s.name),
        Some("limit".to_string())
    );

    // Unclosed variable returns None
    assert_eq!(find_variable_span_at("{\"a\": \"{{open\"}", 9), None);
    // Out-of-bounds offset returns None
    assert_eq!(find_variable_span_at(text, text.len() + 10), None);
}

#[test]
fn effective_environment_matches_resolver_precedence() {
    let variable = |key: &str, value: &str| crate::models::EnvironmentVariable {
        key: key.into(),
        initial_value: value.into(),
        current_value: String::new(),
        secret: false,
    };
    let mut globals = Environment::new("workspace", "Globals");
    globals.variables = vec![
        variable("HOST", "global"),
        variable("HOST", "duplicate"),
        variable("PORT", "8080"),
    ];
    let mut active = Environment::new("workspace", "Active");
    active.variables = vec![
        variable("HOST", "active"),
        variable("HOST", "second active"),
        variable("PATH", "/api"),
    ];

    let effective = merge_effective_environment("workspace", Some(&active), &globals);
    assert_eq!(
        effective
            .variables
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        ["HOST", "PORT", "PATH"]
    );
    for entry in &effective.variables {
        let resolved = resolve_variable(Some(&active), &globals, &entry.key);
        assert_eq!(entry.resolved_value(), resolved.value);
    }
}
