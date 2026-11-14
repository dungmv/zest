//! Protocol-neutral transport vocabulary shared by HTTP, WebSocket and gRPC.
//!
//! The HTTP executor lives in [`crate::http`], the WebSocket client in
//! [`crate::ws`] and the gRPC client in [`crate::grpc`]. Everything those
//! three share — request resolution, hierarchy header/auth merging, the
//! transport error type and the unified realtime frame log — lives here so
//! no protocol has to borrow another one's module.

pub mod error;
pub mod frame;
pub mod hierarchy;
pub mod resolved;

pub use error::RequestError;
pub use frame::{FrameDirection, FrameKind, TransportFrame};
pub use hierarchy::{
    resolve_hierarchical_headers_and_auth, resolve_inherited_headers, InheritedHeader,
};
pub use resolved::{build_url_with_query, parse_query_from_url, ResolvedRequest};

#[cfg(test)]
#[path = "resolved_tests.rs"]
mod tests;
