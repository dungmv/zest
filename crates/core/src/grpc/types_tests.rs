use super::map_endpoint;
use crate::grpc::error::GrpcError;
use crate::grpc::types::GrpcMethodInfo;

#[test]
fn test_map_endpoint_schemes() {
    assert_eq!(
        map_endpoint("grpc://localhost:50051").unwrap(),
        ("http://localhost:50051".to_string(), false)
    );
    assert_eq!(
        map_endpoint("grpcs://api.example.com:443").unwrap(),
        ("https://api.example.com:443".to_string(), true)
    );
    assert_eq!(
        map_endpoint("https://api.example.com").unwrap(),
        ("https://api.example.com".to_string(), true)
    );
    // Bare host defaults to plaintext (Postman-style).
    assert_eq!(
        map_endpoint("localhost:50051").unwrap(),
        ("http://localhost:50051".to_string(), false)
    );
    assert!(matches!(
        map_endpoint("grpc://"),
        Err(GrpcError::InvalidEndpoint(_))
    ));
}

#[test]
fn test_method_info_label() {
    let unary = GrpcMethodInfo {
        name: "SayHello".to_string(),
        is_client_streaming: false,
        is_server_streaming: false,
    };
    assert_eq!(unary.label(), "SayHello()");
    let server = GrpcMethodInfo {
        is_server_streaming: true,
        ..unary.clone()
    };
    assert_eq!(server.label(), "SayHello(server-stream)");
    let bidi = GrpcMethodInfo {
        is_client_streaming: true,
        ..server
    };
    assert_eq!(bidi.label(), "SayHello(bistream)");
}
