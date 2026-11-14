use super::error::GrpcError;
use super::types::GrpcMethodInfo;
use prost::Message as _;
use prost_reflect::{DescriptorPool, MethodDescriptor};
use tonic::transport::Channel;
use tonic_reflection::pb::v1alpha::server_reflection_client::ServerReflectionClient;
use tonic_reflection::pb::v1alpha::server_reflection_request::MessageRequest;
use tonic_reflection::pb::v1alpha::server_reflection_response::MessageResponse;
use tonic_reflection::pb::v1alpha::{ServerReflectionRequest, ServerReflectionResponse};

/// Where the schema for a gRPC request comes from.
#[derive(Debug, Clone)]
pub enum SchemaEndpoint {
    /// Compile a local `.proto` file with protox.
    ProtoFile(String),
    /// Fetch descriptors via gRPC Server Reflection on the given channel.
    Reflection(Channel),
}

/// Loads the descriptor pool for a request:
/// - `.proto` path present → compile it (include dir = file's parent).
/// - otherwise → Server Reflection against the endpoint.
pub async fn resolve_schema(
    proto_path: Option<&str>,
    channel: Option<Channel>,
) -> Result<DescriptorPool, GrpcError> {
    match proto_path {
        Some(path) if !path.trim().is_empty() => load_schema_from_proto_path(path),
        _ => {
            let channel = channel.ok_or_else(|| {
                GrpcError::Schema(
                    "No .proto file configured and no connection available for reflection"
                        .to_string(),
                )
            })?;
            load_schema_from_reflection(channel).await
        }
    }
}

/// Compiles a `.proto` file (plus its imports) into a descriptor pool.
/// The file's own directory is used as the include path so sibling imports
/// resolve.
pub fn load_schema_from_proto_path(path: &str) -> Result<DescriptorPool, GrpcError> {
    let includes = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_path_buf())
        .into_iter();
    let file_set = protox::compile([path], includes)
        .map_err(|e| GrpcError::Schema(format!("Failed to compile {path}: {e}")))?;
    DescriptorPool::decode(file_set.encode_to_vec().as_slice())
        .map_err(|e| GrpcError::Schema(format!("Invalid descriptor set: {e}")))
}

/// Fetches all service descriptors from the server via reflection v1alpha.
pub async fn load_schema_from_reflection(channel: Channel) -> Result<DescriptorPool, GrpcError> {
    let services = list_services_via_reflection(channel.clone()).await?;
    if services.is_empty() {
        return Err(GrpcError::Schema(
            "Server reflection returned no services".to_string(),
        ));
    }

    let mut client = ServerReflectionClient::new(channel);
    let requests: Vec<ServerReflectionRequest> = services
        .iter()
        .map(|svc| ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::FileContainingSymbol(svc.clone())),
        })
        .collect();

    let outgoing = futures_util::stream::iter(requests);
    let mut stream = client
        .server_reflection_info(outgoing)
        .await
        .map_err(|e| GrpcError::Channel(format!("Reflection request failed: {e}")))?
        .into_inner();

    let mut files: Vec<prost_types::FileDescriptorProto> = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    while let Some(response) = stream
        .message()
        .await
        .map_err(|e| GrpcError::Channel(format!("Reflection stream failed: {e}")))?
    {
        collect_file_descriptors(response, &mut files, &mut seen_names);
    }

    let file_set = prost_types::FileDescriptorSet { file: files };
    DescriptorPool::decode(file_set.encode_to_vec().as_slice())
        .map_err(|e| GrpcError::Schema(format!("Invalid reflected descriptor set: {e}")))
}

/// Lists service names exposed by the server via reflection v1alpha.
pub async fn list_services_via_reflection(channel: Channel) -> Result<Vec<String>, GrpcError> {
    let mut client = ServerReflectionClient::new(channel);
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    };
    let outgoing = futures_util::stream::iter(vec![request]);
    let mut stream = client
        .server_reflection_info(outgoing)
        .await
        .map_err(|e| GrpcError::Channel(format!("Reflection list_services failed: {e}")))?
        .into_inner();

    while let Some(response) = stream
        .message()
        .await
        .map_err(|e| GrpcError::Channel(format!("Reflection stream failed: {e}")))?
    {
        if let Some(MessageResponse::ListServicesResponse(list)) = response.message_response {
            return Ok(list
                .service
                .into_iter()
                .map(|s| s.name)
                .filter(|name| !name.starts_with("grpc."))
                .collect());
        }
    }
    Ok(Vec::new())
}

fn collect_file_descriptors(
    response: ServerReflectionResponse,
    files: &mut Vec<prost_types::FileDescriptorProto>,
    seen: &mut std::collections::HashSet<String>,
) {
    if let Some(MessageResponse::FileDescriptorResponse(fd_response)) = response.message_response {
        for bytes in fd_response.file_descriptor_proto {
            if let Ok(file) = prost_types::FileDescriptorProto::decode(bytes.as_slice()) {
                if let Some(name) = &file.name {
                    if seen.insert(name.clone()) {
                        files.push(file);
                    }
                }
            }
        }
    }
}

/// Enumerates the methods of a service from a resolved pool.
pub fn list_methods(
    pool: &DescriptorPool,
    service: &str,
) -> Result<Vec<GrpcMethodInfo>, GrpcError> {
    let svc = pool
        .get_service_by_name(service)
        .ok_or_else(|| GrpcError::Schema(format!("Service '{service}' not found in schema")))?;
    Ok(svc
        .methods()
        .map(|m: MethodDescriptor| GrpcMethodInfo {
            name: m.name().to_string(),
            is_client_streaming: m.is_client_streaming(),
            is_server_streaming: m.is_server_streaming(),
        })
        .collect())
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
