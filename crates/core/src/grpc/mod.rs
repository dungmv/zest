//! gRPC client engine built on `tonic` + `prost-reflect`.
//!
//! Schema comes from either a local `.proto` file (compiled in-process with
//! `protox`) or gRPC Server Reflection; payloads are authored as JSON and
//! converted to protobuf at runtime via dynamic messages.

mod client;
mod codec;
mod completion;
mod error;
mod schema;
mod types;
mod validation;

pub use client::GrpcClient;
pub use codec::DynamicCodec;
pub use completion::{json_field_candidates, KeyCompletionSpan, KeySuggestion};
pub use error::GrpcError;
pub use prost_reflect::{
    DescriptorPool, DeserializeOptions, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor,
};
pub use schema::{
    list_methods, load_schema_from_proto_path, method_descriptors, resolve_schema, SchemaEndpoint,
};
pub use types::{GrpcCallSummary, GrpcMethodInfo, GrpcStreamEvent, GrpcUnaryResult};
pub use validation::{validate_message, MessageValidationError};

#[cfg(test)]
#[path = "reflection_tests.rs"]
mod reflection_tests;
