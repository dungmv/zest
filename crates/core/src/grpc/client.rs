use std::str::FromStr;
use std::time::Instant;

use prost_reflect::{DeserializeOptions, DynamicMessage, MessageDescriptor};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tonic::client::Grpc;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Request, Status};

use super::codec::DynamicCodec;
use super::error::GrpcError;
use super::types::{map_endpoint, GrpcStreamEvent, GrpcUnaryResult};

/// Metadata headers that would corrupt the gRPC framing; tonic sets them.
const RESERVED_METADATA: [&str; 6] = [
    "host",
    "content-type",
    "content-length",
    "te",
    "connection",
    "transfer-encoding",
];

/// Thin dynamic gRPC client over one tonic channel.
#[derive(Clone)]
pub struct GrpcClient {
    channel: Channel,
    timeout_ms: u64,
}

impl GrpcClient {
    /// Builds a channel from a request URL. `grpc://`/bare host → plaintext,
    /// `grpcs://`/`https://` → TLS with native roots.
    pub fn connect(endpoint_url: &str, timeout_ms: u64) -> Result<Self, GrpcError> {
        let (uri, tls) = map_endpoint(endpoint_url)
            .map_err(|_| GrpcError::InvalidEndpoint(endpoint_url.to_string()))?;
        let mut endpoint = Endpoint::from_shared(uri.clone())
            .map_err(|e| GrpcError::InvalidEndpoint(format!("{uri}: {e}")))?
            .connect_timeout(std::time::Duration::from_millis(timeout_ms.min(10_000)));
        if tls {
            endpoint = endpoint
                .tls_config(
                    ClientTlsConfig::new()
                        .with_enabled_roots()
                        .assume_http2(true),
                )
                .map_err(|e| GrpcError::Channel(format!("TLS config failed: {e}")))?;
        }
        let channel = endpoint.connect_lazy();
        Ok(Self {
            channel,
            timeout_ms,
        })
    }

    /// Channel handle for schema resolution (reflection).
    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }

    /// Performs one unary RPC. `metadata` comes from the resolved headers.
    pub async fn invoke_unary(
        &self,
        input_desc: MessageDescriptor,
        output_desc: MessageDescriptor,
        service: &str,
        method: &str,
        message_json: &str,
        metadata: &[(String, String)],
    ) -> Result<GrpcUnaryResult, GrpcError> {
        let started = Instant::now();
        let path = Self::path(service, method)?;
        let request = self.build_request(&input_desc, message_json, metadata)?;

        let mut grpc = Grpc::new(self.channel.clone());
        grpc.ready()
            .await
            .map_err(|e| GrpcError::Channel(format!("Channel not ready: {e}")))?;

        let codec = DynamicCodec::new(output_desc);
        let call = grpc.unary(request, path, codec);
        let response =
            tokio::time::timeout(std::time::Duration::from_millis(self.timeout_ms), call)
                .await
                .map_err(|_| GrpcError::Timeout(self.timeout_ms))?
                .map_err(Self::status_to_error)?;

        let headers = Self::metadata_to_vec(response.metadata());
        let message = response.into_inner();
        let response_json = serde_json::to_string_pretty(&message).map_err(|e| {
            GrpcError::InvalidMessage(format!("Failed to render response JSON: {e}"))
        })?;
        Ok(GrpcUnaryResult {
            response_json,
            headers,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Performs one server-streaming RPC and pumps decoded messages into the
    /// returned channel. The request stream carries a single message.
    pub async fn invoke_server_streaming(
        &self,
        input_desc: MessageDescriptor,
        output_desc: MessageDescriptor,
        service: &str,
        method: &str,
        message_json: &str,
        metadata: &[(String, String)],
    ) -> Result<UnboundedReceiver<GrpcStreamEvent>, GrpcError> {
        let path = Self::path(service, method)?;
        let request = self.build_request(&input_desc, message_json, metadata)?;

        let mut grpc = Grpc::new(self.channel.clone());
        grpc.ready()
            .await
            .map_err(|e| GrpcError::Channel(format!("Channel not ready: {e}")))?;

        let codec = DynamicCodec::new(output_desc);
        let response = grpc
            .server_streaming(request, path, codec)
            .await
            .map_err(Self::status_to_error)?;

        let (tx, rx) = mpsc::unbounded_channel();
        let mut stream = response.into_inner();
        let timeout_ms = self.timeout_ms;
        tokio::spawn(async move {
            let started = Instant::now();
            let mut index: u64 = 0;
            loop {
                let next = tokio::time::timeout(
                    std::time::Duration::from_millis(timeout_ms),
                    stream.message(),
                )
                .await;
                match next {
                    Err(_) => {
                        let _ = tx.send(GrpcStreamEvent::TransportError(format!(
                            "Stream idle beyond {} ms",
                            timeout_ms
                        )));
                        break;
                    }
                    Ok(Err(status)) => {
                        let duration_ms = started.elapsed().as_millis() as u64;
                        if status.code() == tonic::Code::Ok {
                            let _ = tx.send(GrpcStreamEvent::Completed { duration_ms });
                        } else {
                            let _ = tx.send(GrpcStreamEvent::Failed {
                                code: status.code() as i32,
                                message: status.message().to_string(),
                                duration_ms,
                            });
                        }
                        break;
                    }
                    Ok(Ok(Some(message))) => match serde_json::to_string_pretty(&message) {
                        Ok(json) => {
                            index += 1;
                            if tx.send(GrpcStreamEvent::Message { index, json }).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(GrpcStreamEvent::TransportError(format!(
                                "Failed to render response JSON: {e}"
                            )));
                            break;
                        }
                    },
                    Ok(Ok(None)) => {
                        let _ = tx.send(GrpcStreamEvent::Completed {
                            duration_ms: started.elapsed().as_millis() as u64,
                        });
                        break;
                    }
                }
            }
        });
        Ok(rx)
    }

    /// Fetches the service list from the server via reflection (v1alpha with
    /// v1 fallback handled by the server side itself).
    pub async fn list_services(&self) -> Result<Vec<String>, GrpcError> {
        super::schema::list_services_via_reflection(self.channel.clone()).await
    }

    /// Reflection descriptor fetch for all exposed services.
    pub async fn reflection_pool(&self) -> Result<prost_reflect::DescriptorPool, GrpcError> {
        super::schema::load_schema_from_reflection(self.channel.clone()).await
    }

    fn path(
        service: &str,
        method: &str,
    ) -> Result<tonic::codegen::http::uri::PathAndQuery, GrpcError> {
        tonic::codegen::http::uri::PathAndQuery::from_str(&format!("/{service}/{method}"))
            .map_err(|e| GrpcError::InvalidEndpoint(format!("/{service}/{method}: {e}")))
    }

    fn build_request(
        &self,
        input_desc: &MessageDescriptor,
        message_json: &str,
        metadata: &[(String, String)],
    ) -> Result<Request<DynamicMessage>, GrpcError> {
        let json = message_json.trim();
        let message: DynamicMessage = if json.is_empty() {
            DynamicMessage::new(input_desc.clone())
        } else {
            let mut deserializer = serde_json::Deserializer::from_str(json);
            let options = DeserializeOptions::new().deny_unknown_fields(false);
            let msg = DynamicMessage::deserialize_with_options(
                input_desc.clone(),
                &mut deserializer,
                &options,
            )
            .map_err(|e| GrpcError::InvalidMessage(e.to_string()))?;
            deserializer
                .end()
                .map_err(|e| GrpcError::InvalidMessage(e.to_string()))?;
            msg
        };

        let mut request = Request::new(message);
        for (name, value) in metadata {
            let lower = name.to_lowercase();
            if RESERVED_METADATA.contains(&lower.as_str()) || lower.starts_with(':') {
                continue;
            }
            if let (Ok(key), Ok(val)) = (
                lower.parse::<tonic::metadata::MetadataKey<tonic::metadata::Ascii>>(),
                value.parse::<tonic::metadata::MetadataValue<_>>(),
            ) {
                request.metadata_mut().insert(key, val);
            }
        }
        Ok(request)
    }

    fn status_to_error(status: Status) -> GrpcError {
        GrpcError::Status {
            code: status.code() as i32,
            message: status.message().to_string(),
        }
    }

    fn metadata_to_vec(metadata: &tonic::metadata::MetadataMap) -> Vec<(String, String)> {
        metadata
            .iter()
            .filter_map(|kv| match kv {
                tonic::metadata::KeyAndValueRef::Ascii(key, value) => value
                    .to_str()
                    .ok()
                    .map(|v| (key.as_str().to_string(), v.to_string())),
                tonic::metadata::KeyAndValueRef::Binary(..) => None,
            })
            .collect()
    }
}
