//! End-to-end reflection tests against a real in-process gRPC server, so the
//! connect → list_services → descriptor-pool path is exercised over the wire.

use super::client::GrpcClient;
use super::error::GrpcError;
use futures_util::stream::unfold;
use prost::Message as _;

const SAMPLE_PROTO: &str = r#"
syntax = "proto3";
package helloworld;

message HelloRequest {
  string name = 1;
}

message HelloReply {
  string message = 1;
}

service Greeter {
  rpc SayHello(HelloRequest) returns (HelloReply);
}
"#;

// Standard health-check style service: its grpc.* full name must NOT be
// hidden by the internal-services filter.
const HEALTH_PROTO: &str = r#"
syntax = "proto3";
package grpc.health.v1;

message HealthCheckRequest {
  string service = 1;
}

message HealthCheckResponse {
  enum ServingStatus {
    UNKNOWN = 0;
    SERVING = 1;
  }
  ServingStatus status = 1;
}

service Health {
  rpc Check(HealthCheckRequest) returns (HealthCheckResponse);
}
"#;

/// Spawns a reflection-only gRPC server on 127.0.0.1:0 and returns its addr.
async fn spawn_reflection_server() -> std::net::SocketAddr {
    // Compile the sample proto from a temp file so imports resolve.
    let dir = std::env::temp_dir().join(format!("zest-reflect-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let proto_path = dir.join("helloworld.proto");
    std::fs::write(&proto_path, SAMPLE_PROTO).unwrap();
    let health_path = dir.join("health.proto");
    std::fs::write(&health_path, HEALTH_PROTO).unwrap();
    let file_set = protox::compile([proto_path, health_path], [dir.clone()]).unwrap();

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(file_set.encode_to_vec().as_slice())
        .build_v1alpha()
        .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let incoming = unfold(listener, |listener| async move {
        match listener.accept().await {
            Ok((stream, _)) => Some((Ok::<_, std::io::Error>(stream), listener)),
            Err(e) => Some((Err(e), listener)),
        }
    });

    tokio::spawn(async move {
        let _ = tonic::transport::Server::builder()
            .add_service(reflection)
            .serve_with_incoming(incoming)
            .await;
    });

    addr
}

#[test]
fn test_reflection_lists_services_and_builds_pool() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let addr = spawn_reflection_server().await;
        let client = GrpcClient::connect(&format!("http://{addr}"), 5_000).unwrap();

        let services = client.list_services().await.unwrap();
        assert!(
            services.iter().any(|s| s == "helloworld.Greeter"),
            "expected helloworld.Greeter in {services:?}"
        );
        // Standard grpc.* services must survive the filter.
        assert!(
            services.iter().any(|s| s == "grpc.health.v1.Health"),
            "expected grpc.health.v1.Health in {services:?}"
        );
        // Only the reflection services themselves are hidden.
        assert!(
            !services.iter().any(|s| s.starts_with("grpc.reflection.")),
            "reflection services must be filtered: {services:?}"
        );

        let pool = client.reflection_pool().await.unwrap();
        let service = pool.get_service_by_name("helloworld.Greeter").unwrap();
        let methods: Vec<String> = service.methods().map(|m| m.name().to_string()).collect();
        assert_eq!(methods, vec!["SayHello".to_string()]);
        let input = service.methods().next().unwrap().input();
        assert_eq!(input.full_name(), "helloworld.HelloRequest");

        // The health service resolves just like any other.
        let health = pool.get_service_by_name("grpc.health.v1.Health").unwrap();
        let health_methods: Vec<String> = health.methods().map(|m| m.name().to_string()).collect();
        assert_eq!(health_methods, vec!["Check".to_string()]);
    });
}

#[test]
fn test_unary_call_reaches_server_and_maps_status_error() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let addr = spawn_reflection_server().await;
        let client = GrpcClient::connect(&format!("http://{addr}"), 5_000).unwrap();
        let pool = client.reflection_pool().await.unwrap();
        let service = pool.get_service_by_name("helloworld.Greeter").unwrap();
        let method = service.methods().next().unwrap();

        // The reflection-only server has no Greeter implementation, so the
        // fully-formed RPC must come back as a gRPC status error — proving
        // path building, codec and metadata all work over the wire.
        let result = client
            .invoke_unary(
                method.input(),
                method.output(),
                "helloworld.Greeter",
                "SayHello",
                r#"{"name": "Zest"}"#,
                &[],
            )
            .await;

        match result {
            Err(GrpcError::Status { code, message: _ }) => {
                assert_eq!(code, tonic::Code::Unimplemented as i32);
            }
            other => panic!("expected a gRPC status error, got {other:?}"),
        }
    });
}
