//! In-memory GraphQL schema summary parsed from an introspection response.

use std::collections::HashMap;

/// Object-like kinds expose selectable fields; input kinds expose input
/// fields; enums expose values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphQLTypeKind {
    Scalar,
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
}

#[derive(Debug, Clone)]
pub struct GraphQLFieldDef {
    pub name: String,
    pub description: Option<String>,
    /// Base named type with NON_NULL/LIST wrappers stripped.
    pub type_name: String,
    /// Full type signature, e.g. `[User!]!`.
    pub type_display: String,
    pub args: Vec<GraphQLInputValueDef>,
}

#[derive(Debug, Clone)]
pub struct GraphQLInputValueDef {
    pub name: String,
    pub description: Option<String>,
    pub type_name: String,
    pub type_display: String,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphQLTypeDef {
    pub kind: GraphQLTypeKind,
    pub description: Option<String>,
    pub fields: Vec<GraphQLFieldDef>,
    pub input_fields: Vec<GraphQLInputValueDef>,
    pub enum_values: Vec<String>,
}

/// Parsed introspection result cached per session (never persisted).
#[derive(Debug, Clone, Default)]
pub struct GraphQLSchemaSummary {
    pub query_type: String,
    pub mutation_type: Option<String>,
    pub subscription_type: Option<String>,
    /// Named types by name (built-in `__` meta types excluded).
    pub types: HashMap<String, GraphQLTypeDef>,
}

impl GraphQLSchemaSummary {
    /// Root type name for an operation keyword (`query`, `mutation`,
    /// `subscription`).
    pub fn root_type(&self, operation: &str) -> Option<&str> {
        match operation {
            "query" => Some(self.query_type.as_str()),
            "mutation" => self.mutation_type.as_deref(),
            "subscription" => self.subscription_type.as_deref(),
            _ => None,
        }
    }

    /// Looks up a field of a type, returning its definition.
    pub fn field(&self, type_name: &str, field_name: &str) -> Option<&GraphQLFieldDef> {
        self.types
            .get(type_name)
            .and_then(|t| t.fields.iter().find(|f| f.name == field_name))
    }

    /// Looks up an argument of a field, returning the unwrapped named type.
    pub fn arg_type(&self, type_name: &str, field_name: &str, arg_name: &str) -> Option<&str> {
        self.field(type_name, field_name)
            .and_then(|f| f.args.iter().find(|a| a.name == arg_name))
            .map(|a| a.type_name.as_str())
    }
}
