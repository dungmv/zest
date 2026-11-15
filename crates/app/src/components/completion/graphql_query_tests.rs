use super::*;

#[test]
fn test_validate_variables_accepts_object_and_empty() {
    assert!(validate_variables("").is_ok());
    assert!(validate_variables("   \n").is_ok());
    assert!(validate_variables("{}").is_ok());
    assert!(validate_variables("{\"id\": \"1\"}").is_ok());
}

#[test]
fn test_validate_variables_rejects_non_object() {
    let err = validate_variables("[1, 2]").unwrap_err();
    assert!(err.2.contains("JSON object"));
    let err = validate_variables("\"str\"").unwrap_err();
    assert!(err.2.contains("JSON object"));
}

#[test]
fn test_validate_variables_positions_json_errors() {
    let err = validate_variables("{\n  \"id\": }").unwrap_err();
    assert_eq!(err.0, 1, "error on second line (zero-based)");
    assert!(err.2.contains("Invalid JSON"));
}
