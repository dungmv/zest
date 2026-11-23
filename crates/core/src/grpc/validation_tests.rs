use std::sync::OnceLock;

use prost::Message as _;
use prost_reflect::DescriptorPool;

use super::*;

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

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("zest-validate-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shop.proto");
        std::fs::write(&path, SHOP_PROTO).unwrap();
        let file_set = protox::compile([path], [dir.clone()]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        DescriptorPool::decode(file_set.encode_to_vec().as_slice()).unwrap()
    })
}

#[test]
fn test_empty_message_is_valid() {
    assert_eq!(validate_message(pool(), "shop.Svc", "Create", ""), Ok(()));
    assert_eq!(validate_message(pool(), "shop.Svc", "Create", "  "), Ok(()));
}

#[test]
fn test_syntax_error_positions_on_line() {
    // The broken token sits on line 2 (0-based 1).
    let err = validate_message(pool(), "shop.Svc", "Create", "{\n  nope").unwrap_err();
    assert!(err.message.starts_with("Invalid JSON"), "got {err:?}");
    assert_eq!(err.line, 1, "error line must be zero-based {err:?}");
}

#[test]
fn test_unknown_field_rejected() {
    let err = validate_message(pool(), "shop.Svc", "Create", "{\"nme\": \"x\"}").unwrap_err();
    assert!(
        err.message.contains("unrecognized field name"),
        "got {err:?}"
    );
    assert_eq!(err.line, 0);
}

#[test]
fn test_type_mismatch_rejected() {
    let err =
        validate_message(pool(), "shop.Svc", "Create", "{\"quantity\": \"lots\"}").unwrap_err();
    assert!(err.message.contains("invalid digit"), "got {err:?}");
}

#[test]
fn test_valid_message_passes() {
    assert_eq!(
        validate_message(
            pool(),
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
