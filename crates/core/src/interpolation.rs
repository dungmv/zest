use crate::models::{Environment, EnvironmentVariable};
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

static VAR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\s*([$a-zA-Z0-9_.-]+)\s*\}\}").unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableSpan {
    pub start: usize,
    pub end: usize,
    pub name: String,
    pub is_resolved: bool,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpolationResult {
    pub text: String,
    pub unresolved_variables: Vec<String>,
    pub spans: Vec<VariableSpan>,
}

/// Pre-indexes environment variables for high-throughput interpolation without re-allocating
/// `HashMap` on every placeholder or field lookup.
pub struct VariableInterpolator<'a> {
    var_map: HashMap<&'a str, &'a str>,
}

impl<'a> VariableInterpolator<'a> {
    pub fn new(env: Option<&'a Environment>) -> Self {
        let mut var_map = HashMap::new();
        if let Some(env) = env {
            for v in &env.variables {
                var_map.insert(v.key.as_str(), v.resolved_value());
            }
        }
        Self { var_map }
    }

    pub fn from_vars(vars: &'a [EnvironmentVariable]) -> Self {
        let mut var_map = HashMap::new();
        for v in vars {
            var_map.insert(v.key.as_str(), v.resolved_value());
        }
        Self { var_map }
    }

    /// Interpolate `text`, returning `Cow::Borrowed` if no `{{` placeholder exists.
    pub fn interpolate_cow<'s>(&self, text: &'s str) -> Cow<'s, str> {
        if !text.contains("{{") {
            return Cow::Borrowed(text);
        }
        Cow::Owned(interpolate_with_map(text, &self.var_map).text)
    }

    pub fn interpolate(&self, text: &str) -> InterpolationResult {
        interpolate_with_map(text, &self.var_map)
    }
}

/// Pure function to interpolate `{{var}}` placeholders using variables from the given environment.
/// Any unresolved variables remain in their `{{var}}` form in the output string,
/// and are reported in `unresolved_variables`.
pub fn interpolate(text: &str, env: Option<&Environment>) -> InterpolationResult {
    if !text.contains("{{") {
        return InterpolationResult {
            text: text.to_string(),
            unresolved_variables: Vec::new(),
            spans: Vec::new(),
        };
    }
    VariableInterpolator::new(env).interpolate(text)
}

/// Returns `Cow::Borrowed` if text does not contain `{{`, avoiding heap allocations.
pub fn interpolate_cow<'a>(text: &'a str, env: Option<&Environment>) -> Cow<'a, str> {
    if !text.contains("{{") {
        return Cow::Borrowed(text);
    }
    Cow::Owned(interpolate(text, env).text)
}

pub fn interpolate_with_vars(text: &str, vars: &[EnvironmentVariable]) -> InterpolationResult {
    if !text.contains("{{") {
        return InterpolationResult {
            text: text.to_string(),
            unresolved_variables: Vec::new(),
            spans: Vec::new(),
        };
    }
    VariableInterpolator::from_vars(vars).interpolate(text)
}

pub fn interpolate_with_map(text: &str, var_map: &HashMap<&str, &str>) -> InterpolationResult {
    if !text.contains("{{") {
        return InterpolationResult {
            text: text.to_string(),
            unresolved_variables: Vec::new(),
            spans: Vec::new(),
        };
    }
    let mut result = String::with_capacity(text.len());
    let mut last_match_end = 0;
    let mut unresolved_variables = Vec::new();
    let mut spans = Vec::new();

    for caps in VAR_REGEX.captures_iter(text) {
        let whole_match = caps.get(0).unwrap();
        let var_name = caps.get(1).unwrap().as_str().trim();

        // Push text preceding the match
        result.push_str(&text[last_match_end..whole_match.start()]);

        let start_pos = result.len();
        if let Some(&val) = var_map.get(var_name) {
            result.push_str(val);
            let end_pos = result.len();
            spans.push(VariableSpan {
                start: start_pos,
                end: end_pos,
                name: var_name.to_string(),
                is_resolved: true,
                value: Some(val.to_string()),
            });
        } else if var_name.starts_with('$') {
            if let Some(resolved_fake) =
                crate::scripting::fake_vars::generate_fake_var_value(var_name)
            {
                result.push_str(&resolved_fake);
                let end_pos = result.len();
                spans.push(VariableSpan {
                    start: start_pos,
                    end: end_pos,
                    name: var_name.to_string(),
                    is_resolved: true,
                    value: Some(resolved_fake),
                });
            } else {
                result.push_str(whole_match.as_str());
                let end_pos = result.len();
                unresolved_variables.push(var_name.to_string());
                spans.push(VariableSpan {
                    start: start_pos,
                    end: end_pos,
                    name: var_name.to_string(),
                    is_resolved: false,
                    value: None,
                });
            }
        } else {
            result.push_str(whole_match.as_str());
            let end_pos = result.len();
            unresolved_variables.push(var_name.to_string());
            spans.push(VariableSpan {
                start: start_pos,
                end: end_pos,
                name: var_name.to_string(),
                is_resolved: false,
                value: None,
            });
        }

        last_match_end = whole_match.end();
    }

    result.push_str(&text[last_match_end..]);

    InterpolationResult {
        text: result,
        unresolved_variables,
        spans,
    }
}

/// Find variable tokens in raw text before substitution (useful for editor syntax highlighting)
pub fn scan_variables(raw_text: &str, env: Option<&Environment>) -> Vec<VariableSpan> {
    if !raw_text.contains("{{") {
        return Vec::new();
    }
    let mut var_map = HashMap::new();
    if let Some(env) = env {
        for v in &env.variables {
            var_map.insert(v.key.as_str(), v.resolved_value());
        }
    }

    let mut spans = Vec::new();
    for caps in VAR_REGEX.captures_iter(raw_text) {
        let whole_match = caps.get(0).unwrap();
        let var_name = caps.get(1).unwrap().as_str().trim();
        let is_in_env = var_map.contains_key(var_name);
        let (is_resolved, value) = if is_in_env {
            (true, var_map.get(var_name).map(|v| v.to_string()))
        } else if var_name.starts_with('$') {
            if let Some(resolved_fake) =
                crate::scripting::fake_vars::generate_fake_var_value(var_name)
            {
                (true, Some(resolved_fake))
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };

        spans.push(VariableSpan {
            start: whole_match.start(),
            end: whole_match.end(),
            name: var_name.to_string(),
            is_resolved,
            value,
        });
    }
    spans
}

/// Find variable placeholder spans without resolving them: no environment
/// lookup and no fake-variable generation.
///
/// Use this on hot paths such as editor retokenization, where only the
/// position and name of each `{{var}}` placeholder matter and running the
/// resolution machinery (per-call env map, RNG-backed fake variables) would
/// be wasted work on every keystroke.
pub fn scan_variable_spans(raw_text: &str) -> Vec<VariableSpan> {
    if !raw_text.contains("{{") {
        return Vec::new();
    }
    VAR_REGEX
        .captures_iter(raw_text)
        .map(|caps| {
            let whole_match = caps.get(0).unwrap();
            VariableSpan {
                start: whole_match.start(),
                end: whole_match.end(),
                name: caps.get(1).unwrap().as_str().trim().to_string(),
                is_resolved: false,
                value: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_scan_variable_spans_positions_and_names() {
        let spans = scan_variable_spans("https://{{host}}/x/{{  user_id  }}?q={{open");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].start, 8);
        assert_eq!(spans[0].end, 16);
        assert_eq!(spans[0].name, "host");
        assert_eq!(spans[1].start, 19);
        assert_eq!(spans[1].name, "user_id");
        // Lean scan never resolves values, so it must not touch the env map
        // or the RNG-backed fake variable catalog.
        assert!(spans.iter().all(|s| !s.is_resolved && s.value.is_none()));

        assert!(scan_variable_spans("no placeholders here").is_empty());
    }
    use super::*;

    #[test]
    fn test_interpolation_basic() {
        let mut env = Environment::new("ws1", "local");
        env.variables.push(EnvironmentVariable {
            key: "host".to_string(),
            initial_value: "localhost:8000".to_string(),
            current_value: "".to_string(),
            secret: false,
        });
        env.variables.push(EnvironmentVariable {
            key: "token".to_string(),
            initial_value: "xyz123".to_string(),
            current_value: "".to_string(),
            secret: true,
        });

        let res = interpolate("http://{{host}}/api/v1?token={{token}}", Some(&env));
        assert_eq!(res.text, "http://localhost:8000/api/v1?token=xyz123");
        assert!(res.unresolved_variables.is_empty());
        assert_eq!(res.spans.len(), 2);
        assert!(res.spans[0].is_resolved);
        assert!(res.spans[1].is_resolved);
    }

    #[test]
    fn test_interpolation_unresolved() {
        let env = Environment::new("ws1", "local");
        let res = interpolate("http://{{missing_host}}/api/{{missing_path}}", Some(&env));
        assert_eq!(res.text, "http://{{missing_host}}/api/{{missing_path}}");
        assert_eq!(
            res.unresolved_variables,
            vec!["missing_host", "missing_path"]
        );
        assert_eq!(res.spans.len(), 2);
        assert!(!res.spans[0].is_resolved);
        assert!(!res.spans[1].is_resolved);
    }

    #[test]
    fn test_interpolation_prefers_current_value() {
        let mut env = Environment::new("ws1", "local");
        env.variables.push(EnvironmentVariable {
            key: "host".to_string(),
            initial_value: "git.example.com".to_string(),
            current_value: "localhost:8000".to_string(),
            secret: false,
        });

        let res = interpolate("http://{{host}}/test", Some(&env));
        assert_eq!(res.text, "http://localhost:8000/test");
        assert!(res.unresolved_variables.is_empty());
    }

    #[test]
    fn test_scan_variables() {
        let mut env = Environment::new("ws1", "local");
        env.variables.push(EnvironmentVariable {
            key: "port".to_string(),
            initial_value: "8080".to_string(),
            current_value: "".to_string(),
            secret: false,
        });

        let raw = "http://localhost:{{port}}/items/{{item_id}}";
        let spans = scan_variables(raw, Some(&env));
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].name, "port");
        assert!(spans[0].is_resolved);
        assert_eq!(spans[1].name, "item_id");
        assert!(!spans[1].is_resolved);
    }

    #[test]
    fn test_interpolation_dynamic_fake_vars_and_trim() {
        let env = Environment::new("ws1", "local");
        let raw = "https://api.com/{{ $guid }}/users/{{ $randomInt }}";
        let res = interpolate(raw, Some(&env));
        assert!(!res.text.contains("{{"));
        assert!(res.text.starts_with("https://api.com/"));
        assert_eq!(res.spans.len(), 2);
        assert!(res.spans[0].is_resolved);
        assert!(res.spans[1].is_resolved);

        let spans = scan_variables(raw, Some(&env));
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].name, "$guid");
        assert!(spans[0].is_resolved);
        assert!(spans[0].value.is_some());
        assert_eq!(spans[1].name, "$randomInt");
        assert!(spans[1].is_resolved);
    }

    #[test]
    fn test_interpolator_cow_no_alloc() {
        let env = Environment::new("ws1", "local");
        let interpolator = VariableInterpolator::new(Some(&env));
        let plain = "https://api.example.com/users";
        let res = interpolator.interpolate_cow(plain);
        assert_eq!(res, plain);
        assert!(matches!(res, Cow::Borrowed(_)));
    }
}
