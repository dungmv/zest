//! Auto-completion and message-editor support for all editor surfaces.
//!
//! - [`env_autocomplete`]: `{{var}}` detection, candidates, popup rendering,
//!   and inline tokens shared by plain inputs and code editors.
//! - [`proto_json`]: JSON key completion for the gRPC message editor, driven
//!   by the resolved protobuf schema.
//! - [`graphql_query`]: deep query completion for the GraphQL editor,
//!   driven by the introspected schema, plus variables JSON validation.
//! - [`validation`]: gRPC message JSON validation mirrored into editor
//!   gutter marks and hover diagnostics.

pub mod env_autocomplete;
pub mod graphql_query;
pub mod proto_json;
pub mod validation;
