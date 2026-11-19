//! Format exporters: serialize Zest collections into external interchange
//! formats. The export direction mirrors the `crate::importers` architecture:
//! pure functions over shared domain/DTO types, no I/O (the caller decides
//! where to write the serialized text).

pub mod openapi;

pub use openapi::{requests_to_openapi, serialize_openapi, OpenApiExport, OpenApiFileFormat};
