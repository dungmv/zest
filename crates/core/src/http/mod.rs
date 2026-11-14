//! HTTP transport: connection pooling and request execution over reqwest.
//!
//! Protocol-neutral machinery (request resolution, hierarchy merge, transport
//! errors, frame log) lives in [`crate::transport`].

pub mod executor;

pub use executor::{HttpExecutor, HttpProgress, ReqwestExecutor};
