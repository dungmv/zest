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
        Some((_, candidates)) => candidates.into_iter().map(|c| c.name).collect(),
        None => Vec::new(),
    }
}

#[test]
fn test_top_level_candidates_and_replace_offset() {
    let (span, candidates) = json_field_candidates(&order_message(), "{\"").unwrap();
    assert!(!span.needs_quote);
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
    // Suggestions replace the partially typed key after the quote.
    assert_eq!((span.replace_start, span.replace_end), (2, 2));
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
    let (span, candidates) = json_field_candidates(&order_message(), "{").unwrap();
    assert!(
        span.needs_quote,
        "no opening quote: insertion must carry quotes"
    );
    assert_eq!((span.replace_start, span.replace_end), (1, 1));
    assert_eq!(candidates.len(), 5);

    // After a comma separating members: insertion at the caret, quoted.
    let prefix = "{\"id\": \"a\", ";
    let (span, candidates) = json_field_candidates(&order_message(), prefix).unwrap();
    assert!(span.needs_quote);
    assert_eq!(
        (span.replace_start, span.replace_end),
        (prefix.len(), prefix.len())
    );
    assert_eq!(candidates.len(), 5);

    // Typing an identifier without a quote still suggests, and the
    // completion span covers the partial identifier (quoted on insert).
    let prefix = "{\"id\": \"a\", qu";
    let (span, candidates) = json_field_candidates(&order_message(), prefix).unwrap();
    assert!(span.needs_quote);
    assert_eq!((span.replace_start, span.replace_end), (12, 14));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].name, "quantity");

    // Nested: right after the inner `{`.
    let nested = json_field_candidates(&order_message(), "{\"customer\": {").unwrap();
    assert_eq!(nested.1.len(), 2);

    // Second field after a NUMBER value on its own line: the comma must
    // reset the expecting-value state (regression: only the first field
    // suggested).
    let prefix = "{
  \"quantity\": 5,
  \"i";
    let (span, candidates) = json_field_candidates(&order_message(), prefix).unwrap();
    assert!(!span.needs_quote);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].name, "id");

    // Unquoted identifier after a number value on a new line.
    let prefix = "{
  \"quantity\": 5,
  i";
    let (span, candidates) = json_field_candidates(&order_message(), prefix).unwrap();
    assert!(span.needs_quote);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].name, "id");

    // Value positions yield nothing.
    assert!(json_field_candidates(&order_message(), "{\"id\": ").is_none());
    assert!(json_field_candidates(&order_message(), "{\"quantity\": 5").is_none());
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

    let (span, candidates) = json_field_candidates(&cart, "{\"items\":[{\"").unwrap();
    assert!(!span.needs_quote);
    let names: Vec<String> = candidates.into_iter().map(|c| c.name).collect();
    assert_eq!(names, vec!["sku".to_string()]);
    assert!(span.replace_start > 0);
}
