//! Pure domain environment variable resolution, candidate listing, and placeholder lookup.
//!
//! This module is the single source of truth for variable precedence
//! (`Active Environment -> Global -> Dynamic ($...) -> Missing`), first-wins
//! duplicate key semantics, secret masking, and `{{var}}` span lookup.
//! UI layers (tokens, editor decorations, hover/definition LSP, autocomplete)
//! bind to these types without duplicating business rules.

use super::interpolation::{scan_variable_spans, VariableSpan};
use crate::models::Environment;
use crate::scripting::{generate_fake_var_value, FAKE_VAR_CATALOG};
use std::collections::HashSet;

/// Mask string used when displaying secret variable values.
pub const SECRET_MASK: &str = "••••••••";

/// Scope where an environment variable was resolved (or `Missing` if undefined).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableScope {
    /// Defined in the currently active environment (carries the environment name).
    Active(String),
    /// Defined in workspace globals.
    Global,
    /// Built-in dynamic variable (`$guid`, `$timestamp`, etc.).
    Dynamic,
    /// Not defined in any scope.
    Missing,
}

impl VariableScope {
    /// Human-readable scope label matching the UI badges and tooltips.
    pub fn label(&self) -> String {
        match self {
            Self::Active(env_name) => format!("Active: {}", env_name),
            Self::Global => "Global".to_string(),
            Self::Dynamic => "Dynamic".to_string(),
            Self::Missing => "Missing".to_string(),
        }
    }

    pub fn is_resolved(&self) -> bool {
        !matches!(self, Self::Missing)
    }
}

/// Display-safe resolution result for a single variable name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVariable {
    pub name: String,
    /// Display-safe value (`"••••••••"` when `is_secret`, `"Dynamic: <sample>"`
    /// for dynamic variables, `"Unresolved variable"` when missing).
    pub value: String,
    pub scope: VariableScope,
    pub is_secret: bool,
}

impl ResolvedVariable {
    pub fn is_resolved(&self) -> bool {
        self.scope.is_resolved()
    }

    pub fn scope_label(&self) -> String {
        self.scope.label()
    }
}

/// Autocomplete candidate representing an available environment or dynamic variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableCandidate {
    pub name: String,
    /// Display-safe resolved value for env/global variables; generated sample for dynamic ones.
    pub value: String,
    pub scope: VariableScope,
    pub is_secret: bool,
    /// Human-readable description (dynamic variables only).
    pub description: Option<String>,
}

impl VariableCandidate {
    /// Human-readable summary: description for dynamic variables, resolved value otherwise.
    pub fn detail_text(&self) -> &str {
        self.description.as_deref().unwrap_or(&self.value)
    }

    pub fn scope_label(&self) -> String {
        self.scope.label()
    }
}

/// Resolves a variable name against `active_env`, `globals`, and dynamic `$...` variables.
///
/// Precedence: `Active Environment` -> `Global` -> `Dynamic ($...)` -> `Missing`.
/// Duplicate keys within an environment resolve first-wins, matching `VariableInterpolator`.
pub fn resolve_variable(
    active_env: Option<&Environment>,
    globals: &Environment,
    var_name: &str,
) -> ResolvedVariable {
    // 1. Active Environment (highest priority)
    if let Some(active) = active_env {
        if let Some(v) = active.variables.iter().find(|x| x.key == var_name) {
            let value = if v.secret {
                SECRET_MASK.to_string()
            } else {
                v.resolved_value().to_string()
            };
            return ResolvedVariable {
                name: var_name.to_string(),
                value,
                scope: VariableScope::Active(active.name.clone()),
                is_secret: v.secret,
            };
        }
    }

    // 2. Global Environment
    if let Some(v) = globals.variables.iter().find(|x| x.key == var_name) {
        let value = if v.secret {
            SECRET_MASK.to_string()
        } else {
            v.resolved_value().to_string()
        };
        return ResolvedVariable {
            name: var_name.to_string(),
            value,
            scope: VariableScope::Global,
            is_secret: v.secret,
        };
    }

    // 3. Dynamic variables ($guid, $timestamp, ...)
    if var_name.starts_with('$') {
        if let Some(fake) = generate_fake_var_value(var_name) {
            return ResolvedVariable {
                name: var_name.to_string(),
                value: format!("Dynamic: {}", fake),
                scope: VariableScope::Dynamic,
                is_secret: false,
            };
        }
    }

    ResolvedVariable {
        name: var_name.to_string(),
        value: "Unresolved variable".to_string(),
        scope: VariableScope::Missing,
        is_secret: false,
    }
}

/// Lists all available variables matching `query` (case-insensitive substring match),
/// ordered by precedence (`Active` -> `Global` -> `Dynamic`) with shadowing deduplication.
pub fn list_candidates(
    active_env: Option<&Environment>,
    globals: &Environment,
    query: &str,
) -> Vec<VariableCandidate> {
    let q = query.to_lowercase();
    let mut candidates = Vec::new();
    let mut seen_keys = HashSet::new();

    // 1. Active Environment variables (highest priority)
    if let Some(active) = active_env {
        for v in &active.variables {
            if seen_keys.insert(v.key.as_str())
                && (q.is_empty() || v.key.to_lowercase().contains(&q))
            {
                candidates.push(VariableCandidate {
                    name: v.key.clone(),
                    value: if v.secret {
                        SECRET_MASK.to_string()
                    } else {
                        v.resolved_value().to_string()
                    },
                    scope: VariableScope::Active(active.name.clone()),
                    is_secret: v.secret,
                    description: None,
                });
            }
        }
    }

    // 2. Global Environment variables
    for v in &globals.variables {
        if seen_keys.insert(v.key.as_str()) && (q.is_empty() || v.key.to_lowercase().contains(&q)) {
            candidates.push(VariableCandidate {
                name: v.key.clone(),
                value: if v.secret {
                    SECRET_MASK.to_string()
                } else {
                    v.resolved_value().to_string()
                },
                scope: VariableScope::Global,
                is_secret: v.secret,
                description: None,
            });
        }
    }

    // 3. Dynamic variables ($guid, $timestamp, etc.)
    for (d_name, d_desc) in FAKE_VAR_CATALOG {
        if q.is_empty()
            || d_name.to_lowercase().contains(&q)
            || d_name.trim_start_matches('$').to_lowercase().contains(&q)
        {
            candidates.push(VariableCandidate {
                name: (*d_name).to_string(),
                value: generate_fake_var_value(d_name).unwrap_or_default(),
                scope: VariableScope::Dynamic,
                is_secret: false,
                description: Some((*d_desc).to_string()),
            });
        }
    }

    candidates
}

/// Finds the `{{var}}` placeholder span containing UTF-8 byte `offset`, if any.
///
/// Uses half-open bounds `span.start <= offset && offset < span.end` so hovering
/// or clicking immediately outside the closing `}}` does not trigger on the variable.
pub fn find_variable_span_at(text: &str, offset: usize) -> Option<VariableSpan> {
    if offset >= text.len() || !text.contains("{{") {
        return None;
    }
    scan_variable_spans(text)
        .into_iter()
        .find(|span| offset >= span.start && offset < span.end)
}

#[cfg(test)]
mod tests {
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
}
