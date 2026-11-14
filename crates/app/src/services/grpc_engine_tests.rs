use super::*;
use zest_core::grpc::DescriptorPool;

#[test]
fn test_auto_selection_single_service_single_method() {
    let summary = GrpcSchemaSummary {
        pool: DescriptorPool::default(),
        services: vec![(
            "helloworld.Greeter".to_string(),
            vec![GrpcMethodInfo {
                name: "SayHello".to_string(),
                is_client_streaming: false,
                is_server_streaming: false,
            }],
        )],
    };
    assert_eq!(
        auto_selection(&summary),
        Some(("helloworld.Greeter".to_string(), "SayHello".to_string()))
    );
}

#[test]
fn test_auto_selection_rejects_ambiguous_schemas() {
    let two_methods = GrpcSchemaSummary {
        pool: DescriptorPool::default(),
        services: vec![(
            "svc.S".to_string(),
            vec![
                GrpcMethodInfo {
                    name: "A".to_string(),
                    is_client_streaming: false,
                    is_server_streaming: false,
                },
                GrpcMethodInfo {
                    name: "B".to_string(),
                    is_client_streaming: false,
                    is_server_streaming: false,
                },
            ],
        )],
    };
    assert_eq!(auto_selection(&two_methods), None);

    let two_services = GrpcSchemaSummary {
        pool: DescriptorPool::default(),
        services: vec![
            (
                "svc.A".to_string(),
                vec![GrpcMethodInfo {
                    name: "M".to_string(),
                    is_client_streaming: false,
                    is_server_streaming: false,
                }],
            ),
            (
                "svc.B".to_string(),
                vec![GrpcMethodInfo {
                    name: "M".to_string(),
                    is_client_streaming: false,
                    is_server_streaming: false,
                }],
            ),
        ],
    };
    assert_eq!(auto_selection(&two_services), None);

    assert_eq!(auto_selection(&GrpcSchemaSummary::default()), None);
}

#[test]
fn test_grpc_frame_log_survives_and_caps() {
    let engine = GrpcEngine::new();
    for i in 0..(MAX_GRPC_LOG_FRAMES + 50) {
        engine.register_frame(
            "r1",
            TransportFrame {
                id: 0,
                timestamp_ms: 0,
                direction: FrameDirection::Inbound,
                kind: FrameKind::Text,
                payload: format!("m{i}").into_bytes(),
            },
        );
    }
    let frames = engine.snapshot_frames("r1").unwrap();
    assert_eq!(frames.len(), MAX_GRPC_LOG_FRAMES);
    assert_eq!(frames[0].payload, b"m50".to_vec(), "oldest frames dropped");

    // Per-request isolation.
    assert!(engine.snapshot_frames("r2").is_none());

    engine.register_last_call(
        "r1",
        GrpcCallSummary {
            status_code: 0,
            status_message: None,
            headers: Vec::new(),
            duration_ms: 5,
        },
    );
    assert_eq!(engine.snapshot_last_call("r1").unwrap().duration_ms, 5);
}
