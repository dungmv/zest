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
use std::collections::{HashMap, HashSet};

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

/// Merges the active variables and globals for request execution.
/// The first occurrence within each scope wins, and active values override globals.
pub fn merge_effective_environment(
    workspace_name: &str,
    active: Option<&Environment>,
    globals: &Environment,
) -> Environment {
    let mut effective = Environment::new(workspace_name, "Effective");
    let mut key_map = HashMap::<&str, usize>::new();

    // Duplicate keys resolve first-wins within a layer (matching the
    // variable tooltip, autocomplete and interpolator), while the active
    // environment still overrides globals.
    for v in &globals.variables {
        if key_map.contains_key(v.key.as_str()) {
            continue;
        }
        key_map.insert(v.key.as_str(), effective.variables.len());
        effective.variables.push(v.clone());
    }

    if let Some(active_env) = active {
        let mut seen_active = HashSet::new();
        for v in &active_env.variables {
            if !seen_active.insert(v.key.as_str()) {
                continue;
            }
            if let Some(&idx) = key_map.get(v.key.as_str()) {
                effective.variables[idx] = v.clone();
            } else {
                key_map.insert(v.key.as_str(), effective.variables.len());
                effective.variables.push(v.clone());
            }
        }
    }
    effective
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
