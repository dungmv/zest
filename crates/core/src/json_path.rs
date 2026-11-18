//! JSONPath query evaluation over parsed JSON values (RFC 9535).
//!
//! Used by the response inspector's query bar to filter a JSON response body
//! down to the nodes matched by a user-provided JSONPath expression.

use serde_json::Value;

/// The materialized result of a JSONPath query, ready for display.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonQueryResult {
    /// Pretty-printed JSON text of all matches (empty string when nothing matched).
    pub result_text: String,
    /// `result_text` split into lines for line-oriented viewers.
    pub lines: Vec<String>,
    /// Number of nodes matched by the query.
    pub match_count: usize,
}

/// Evaluate a JSONPath query against a parsed JSON value.
///
/// A single match renders as the pretty-printed node itself; multiple matches
/// render as a pretty-printed JSON array of the matched nodes. Returns `Err`
/// with a human-readable message when the query syntax is invalid.
pub fn run_json_path(root: &Value, query: &str) -> Result<JsonQueryResult, String> {
    let path = serde_json_path::JsonPath::parse(query)
        .map_err(|err| format!("Invalid JSONPath: {err}"))?;
    let nodes = path.query(root);

    let values: Vec<&Value> = nodes.iter().copied().collect();
    let result_text = match values.len() {
        0 => String::new(),
        1 => serde_json::to_string_pretty(values[0]).map_err(|err| err.to_string())?,
        _ => serde_json::to_string_pretty(&values).map_err(|err| err.to_string())?,
    };

    let lines: Vec<String> = result_text.lines().map(str::to_string).collect();
    Ok(JsonQueryResult {
        result_text,
        lines,
        match_count: values.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "data": {
                "items": [
                    { "id": "a1", "price": 3 },
                    { "id": "b2", "price": 12 },
                    { "id": "c3", "price": 7 }
                ]
            },
            "store": { "book": [ { "price": 8.5 }, { "price": 9.5 } ] }
        })
    }

    #[test]
    fn wildcard_extraction() {
        let result = run_json_path(&sample(), "$.data.items[*].id").unwrap();
        assert_eq!(result.match_count, 3);
        let parsed: Value = serde_json::from_str(&result.result_text).unwrap();
        assert_eq!(parsed, json!(["a1", "b2", "c3"]));
        assert_eq!(result.lines.len(), 5);
    }

    #[test]
    fn recursive_descent() {
        let result = run_json_path(&sample(), "$..price").unwrap();
        assert_eq!(result.match_count, 5);
    }

    #[test]
    fn filter_expression() {
        let result = run_json_path(&sample(), "$.data.items[?(@.price < 10)].id").unwrap();
        assert_eq!(result.match_count, 2);
    }

    #[test]
    fn single_match_returns_node_not_array() {
        let result = run_json_path(&sample(), "$.data.items[0]").unwrap();
        assert_eq!(result.match_count, 1);
        let parsed: Value = serde_json::from_str(&result.result_text).unwrap();
        assert_eq!(parsed, json!({ "id": "a1", "price": 3 }));
    }

    #[test]
    fn root_query_returns_whole_document() {
        let root = sample();
        let result = run_json_path(&root, "$").unwrap();
        assert_eq!(result.match_count, 1);
        let parsed: Value = serde_json::from_str(&result.result_text).unwrap();
        assert_eq!(parsed, root);
    }

    #[test]
    fn no_matches_gives_empty_text() {
        let result = run_json_path(&sample(), "$.missing").unwrap();
        assert_eq!(result.match_count, 0);
        assert_eq!(result.result_text, "");
        assert!(result.lines.is_empty());
    }

    #[test]
    fn invalid_syntax_is_error() {
        assert!(run_json_path(&sample(), "$.").is_err());
        assert!(run_json_path(&sample(), "data.items").is_err());
        assert!(run_json_path(&sample(), "$[?(@.price<)]").is_err());
    }

    #[test]
    fn scalar_root() {
        let result = run_json_path(&json!(42), "$").unwrap();
        assert_eq!(result.match_count, 1);
        assert_eq!(result.result_text, "42");
    }

    #[test]
    fn array_root_indexing() {
        let root = json!([{ "id": 1 }, { "id": 2 }]);
        let result = run_json_path(&root, "$[*].id").unwrap();
        assert_eq!(result.match_count, 2);
        let parsed: Value = serde_json::from_str(&result.result_text).unwrap();
        assert_eq!(parsed, json!([1, 2]));
    }
}
