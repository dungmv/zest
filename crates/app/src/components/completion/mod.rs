//! Auto-completion and message-editor support for all editor surfaces.
//!
//! - [`proto_json`]: JSON key completion for the gRPC message editor, driven
//!   by the resolved protobuf schema.
//! - [`graphql_query`]: deep query completion for the GraphQL editor,
//!   driven by the introspected schema, plus variables JSON validation.
//! - [`validation`]: gRPC message JSON validation mirrored into editor
//!   gutter marks and hover diagnostics.
//!
//! Environment variable completion, tokens, decorations, and LSP providers
//! live in `crate::env`.

pub mod graphql_query;
pub mod proto_json;
pub mod validation;
