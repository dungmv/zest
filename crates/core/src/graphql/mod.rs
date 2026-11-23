//! GraphQL client support: introspection schema parsing and the deep
//! completion resolver that suggests fields/arguments/enum values at the
//! caret according to the enclosing type context.
//!
//! Execution itself rides the HTTP pipeline — [`crate::models::request::
//! RequestItem::to_graphql_http_item`] converts a GraphQL request into the
//! equivalent HTTP POST — so this module only carries the schema-aware
//! editing features.

mod complete;
mod introspection;
mod lexer;
mod schema;
mod validation;

pub use complete::{completions_at, Completion, CompletionKind};
pub use introspection::{parse_introspection, INTROSPECTION_QUERY};
pub use schema::{
    GraphQLFieldDef, GraphQLInputValueDef, GraphQLSchemaSummary, GraphQLTypeDef, GraphQLTypeKind,
};
pub use validation::{validate_variables, VariablesValidationError};
