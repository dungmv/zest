use super::*;
use prost::Message as _;
use zest_core::grpc::DescriptorPool;

const SHOP_PROTO: &str = r#"
syntax = "proto3";
package shop;

message Order {
  string id = 1;
  int32 quantity = 2;
  Customer customer = 3;
  repeated string tags = 4;
  map<string, string> labels = 5;
}

message Customer {
  string id = 1;
  string email = 2;
}
"#;

fn order_message() -> MessageDescriptor {
    let dir =
        std::env::temp_dir().join(format!("zest-completion-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shop.proto");
    std::fs::write(&path, SHOP_PROTO).unwrap();
    let file_set = protox::compile([path], [dir.clone()]).unwrap();
    let pool = DescriptorPool::decode(file_set.encode_to_vec().as_slice()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    pool.get_message_by_name("shop.Order").unwrap()
}

fn names(prefix: &str) -> Vec<String> {
    match json_field_candidates(&order_message(), prefix) {
        Some((_, _, candidates)) => candidates.into_iter().map(|c| c.name).collect(),
        None => Vec::new(),
    }
}

#[test]
fn test_top_level_candidates_and_replace_offset() {
    let (start, _end, candidates) = json_field_candidates(&order_message(), "{\"").unwrap();
    let mut names: Vec<String> = candidates.into_iter().map(|c| c.name).collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "customer".to_string(),
            "id".to_string(),
            "labels".to_string(),
            "quantity".to_string(),
            "tags".to_string()
        ]
    );
    // Suggestions insert at the cursor (right after the opening quote).
    assert_eq!(start, 2);
}

#[test]
fn test_query_filters_candidates() {
    eprintln!("DEBUG i-case: {:?}", names("{\"i\""));
    eprintln!("DEBUG i-open: {:?}", names("{\"i"));
    assert_eq!(names("{\"i\""), vec!["id".to_string()]);
    assert_eq!(names("{\"qu"), vec!["quantity".to_string()]);
    assert!(names("{\"zzz").is_empty());
}

#[test]
fn test_nested_message_candidates() {
    let nested = names("{\"customer\":{\"");
    assert_eq!(nested, vec!["id".to_string(), "email".to_string()]);

    // Back at the parent level after the nested object closes.
    let back = names("{\"customer\":{\"id\":\"c1\"},\"");
    eprintln!("DEBUG back: {:?}", back);
    assert!(back.contains(&"quantity".to_string()));
}

#[test]
fn test_map_literal_has_no_candidates() {
    assert!(names("{\"labels\":{\"").is_empty());
}

#[test]
fn test_manual_trigger_positions() {
    // Ctrl+Space right after `{` (no quote typed): all fields inserted at
    // the caret.
    let (start, end, candidates) = json_field_candidates(&order_message(), "{").unwrap();
    assert_eq!((start, end), (1, 1), "insert at caret, nothing replaced");
    assert_eq!(candidates.len(), 5);

    // After a comma separating members: insertion at the caret.
    let prefix = "{\"id\": \"a\", ";
    let (start, end, candidates) = json_field_candidates(&order_message(), prefix).unwrap();
    assert_eq!((start, end), (prefix.len(), prefix.len()));
    assert_eq!(candidates.len(), 5);

    // Nested: right after the inner `{`.
    let nested = json_field_candidates(&order_message(), "{\"customer\": {").unwrap();
    assert_eq!(nested.2.len(), 2);

    // Value positions yield nothing.
    assert!(json_field_candidates(&order_message(), "{\"id\": ").is_none());
    assert!(json_field_candidates(&order_message(), "{\"tags\": [\"ab").is_none());
    assert!(json_field_candidates(&order_message(), "{\"id\": \"a\"").is_none());
}

#[test]
fn test_repeated_message_element_suggests_element_fields() {
    let dir =
        std::env::temp_dir().join(format!("zest-completion-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let proto = dir.join("orders.proto");
    std::fs::write(
        &proto,
        "syntax = \"proto3\"; package shop2; message Item { string sku = 1; } message Cart { repeated Item items = 1; }",
    )
    .unwrap();
    let file_set = protox::compile([proto], [dir.clone()]).unwrap();
    let pool = DescriptorPool::decode(file_set.encode_to_vec().as_slice()).unwrap();
    let cart = pool.get_message_by_name("shop2.Cart").unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let (start, _end, candidates) = json_field_candidates(&cart, "{\"items\":[{\"").unwrap();
    let names: Vec<String> = candidates.into_iter().map(|c| c.name).collect();
    assert_eq!(names, vec!["sku".to_string()]);
    assert!(start > 0);
}

mod validation {
    use super::*;
    use crate::components::proto_json_completion::validate_message;

    const SHOP_PROTO: &str = r#"
syntax = "proto3";
package shop;

message Order {
  string id = 1;
  int32 quantity = 2;
  map<string, string> labels = 3;
}

service Svc {
  rpc Create(Order) returns (Order);
}
"#;

    fn pool() -> DescriptorPool {
        let dir =
            std::env::temp_dir().join(format!("zest-validate-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shop.proto");
        std::fs::write(&path, SHOP_PROTO).unwrap();
        let file_set = protox::compile([path], [dir.clone()]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        DescriptorPool::decode(file_set.encode_to_vec().as_slice()).unwrap()
    }

    #[test]
    fn test_empty_message_is_valid() {
        let pool = pool();
        assert_eq!(validate_message(&pool, "shop.Svc", "Create", ""), Ok(()));
        assert_eq!(validate_message(&pool, "shop.Svc", "Create", "  "), Ok(()));
    }

    #[test]
    fn test_syntax_error_positions_on_line() {
        // The broken token sits on line 2 (0-based 1).
        let err = validate_message(
            &pool(),
            "shop.Svc",
            "Create",
            "{
  nope",
        )
        .unwrap_err();
        assert!(err.message.starts_with("Invalid JSON"), "got {err:?}");
        assert_eq!(err.line, 1, "error line must be zero-based {err:?}");
    }

    #[test]
    fn test_unknown_field_rejected() {
        let err = validate_message(&pool(), "shop.Svc", "Create", "{\"nme\": \"x\"}").unwrap_err();
        assert!(
            err.message.contains("unrecognized field name"),
            "got {err:?}"
        );
        assert_eq!(err.line, 0);
    }

    #[test]
    fn test_type_mismatch_rejected() {
        let err = validate_message(&pool(), "shop.Svc", "Create", "{\"quantity\": \"lots\"}")
            .unwrap_err();
        assert!(err.message.contains("invalid digit"), "got {err:?}");
    }

    #[test]
    fn test_valid_message_passes() {
        assert_eq!(
            validate_message(
                &pool(),
                "shop.Svc",
                "Create",
                "{\"id\": \"a\", \"quantity\": 2, \"labels\": {\"k\": \"v\"}}"
            ),
            Ok(())
        );
    }

    #[test]
    fn test_without_schema_only_syntax_checked() {
        assert_eq!(
            validate_message(&DescriptorPool::default(), "", "", "{\"anything\": 1}"),
            Ok(())
        );
    }
}
