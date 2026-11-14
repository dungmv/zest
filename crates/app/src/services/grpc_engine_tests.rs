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
