use super::*;
use prost_reflect::DynamicMessage;

const SAMPLE_PROTO: &str = r#"
syntax = "proto3";
package helloworld;

message HelloRequest {
  string name = 1;
  int32 age = 2;
  repeated string tags = 3;
  map<string, string> labels = 4;
}

message HelloReply {
  string message = 1;
}

service Greeter {
  rpc SayHello(HelloRequest) returns (HelloReply);
  rpc StreamReplies(HelloRequest) returns (stream HelloReply);
}
"#;

fn write_sample_proto(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("helloworld.proto");
    std::fs::write(&path, SAMPLE_PROTO).unwrap();
    path
}

#[test]
fn test_proto_path_compiles_and_lists_methods() {
    let dir = std::env::temp_dir().join(format!("zest-grpc-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_sample_proto(&dir);

    let pool = load_schema_from_proto_path(path.to_str().unwrap()).unwrap();
    let methods = list_methods(&pool, "helloworld.Greeter").unwrap();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "SayHello");
    assert!(!methods[0].is_server_streaming);
    assert_eq!(methods[1].name, "StreamReplies");
    assert!(methods[1].is_server_streaming);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_json_message_roundtrip() {
    let dir = std::env::temp_dir().join(format!("zest-grpc-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_sample_proto(&dir);
    let pool = load_schema_from_proto_path(path.to_str().unwrap()).unwrap();

    let input_desc = pool.get_message_by_name("helloworld.HelloRequest").unwrap();
    let json = r#"{"name": "Zest", "age": 3, "tags": ["a", "b"], "labels": {"k": "v"}}"#;
    let mut deserializer = serde_json::Deserializer::from_str(json);
    let options = prost_reflect::DeserializeOptions::new().deny_unknown_fields(false);
    let msg =
        DynamicMessage::deserialize_with_options(input_desc.clone(), &mut deserializer, &options)
            .unwrap();
    deserializer.end().unwrap();

    let output_desc = pool.get_message_by_name("helloworld.HelloReply").unwrap();
    // Canonical JSON serialization then re-parse must preserve values.
    let serialized = serde_json::to_string(&msg).unwrap();
    assert!(serialized.contains("\"name\":\"Zest\""));
    let mut de2 = serde_json::Deserializer::from_str(&serialized);
    let reparsed =
        DynamicMessage::deserialize_with_options(input_desc.clone(), &mut de2, &options).unwrap();
    assert_eq!(msg, reparsed);
    // Empty JSON deserializes to the zero-value message.
    let empty = DynamicMessage::new(output_desc);
    assert_eq!(serde_json::to_string(&empty).unwrap(), "{}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_proto_import_from_sibling_file() {
    let dir = std::env::temp_dir().join(format!("zest-grpc-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("base.proto"),
        "syntax = \"proto3\"; package base; message Base { string id = 1; }",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.proto"),
        "syntax = \"proto3\"; package main; import \"base.proto\"; message Wrapper { base.Base inner = 1; }",
    )
    .unwrap();

    let pool = load_schema_from_proto_path(dir.join("main.proto").to_str().unwrap()).unwrap();
    assert!(pool.get_message_by_name("main.Wrapper").is_some());
    assert!(pool.get_message_by_name("base.Base").is_some());

    let _ = std::fs::remove_dir_all(&dir);
}
