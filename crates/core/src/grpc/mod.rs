//! gRPC client engine built on `tonic` + `prost-reflect`.
//!
//! Schema comes from either a local `.proto` file (compiled in-process with
//! `protox`) or gRPC Server Reflection; payloads are authored as JSON and
//! converted to protobuf at runtime via dynamic messages.

mod client;
mod codec;
mod error;
mod schema;
mod types;

pub use client::GrpcClient;
pub use codec::DynamicCodec;
pub use error::GrpcError;
pub use prost_reflect::DescriptorPool;
pub use schema::{list_methods, load_schema_from_proto_path, resolve_schema, SchemaEndpoint};
pub use types::{GrpcCallSummary, GrpcMethodInfo, GrpcStreamEvent, GrpcUnaryResult};
