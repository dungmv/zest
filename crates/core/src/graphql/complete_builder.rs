use std::collections::HashMap;

use super::super::schema::{GraphQLSchemaSummary, GraphQLTypeDef, GraphQLTypeKind};
use super::{Completion, CompletionKind, Expect, MAX_COMPLETIONS};

/// Standard executable directives offered after `@`.
const BUILTIN_DIRECTIVES: &[(&str, &str)] = &[
    ("skip", "Boolean! — skip this field/fragment"),
    ("include", "Boolean! — include this field/fragment"),
    ("deprecated", "String — on field/enum definitions"),
    ("specifiedBy", "String! — custom scalar URL"),
];

pub(super) fn build_completions(
    expect: Expect,
    prefix: &str,
    schema: &GraphQLSchemaSummary,
    fragments: &HashMap<String, String>,
    last_directive: Option<String>,
) -> Vec<Completion> {
    // A directive name may be typed with or without the leading `@`.
    let stripped = prefix.trim_start_matches('@');
    let wants_directive = prefix.starts_with('@');

    let mut out = Vec::new();
    let mut push = |label: &str, kind: CompletionKind, detail: &str| {
        if out.len() >= MAX_COMPLETIONS {
            return;
        }
        if !stripped.is_empty() && !label.starts_with(stripped) {
            return;
        }
        out.push(Completion {
            label: label.to_string(),
            kind,
            detail: detail.to_string(),
        });
    };

    let enum_or_bool_values = |push: &mut dyn FnMut(&str, CompletionKind, &str), t: &str| {
        if let Some(def) = schema.types.get(t) {
            if def.kind == GraphQLTypeKind::Enum {
                for value in &def.enum_values {
                    push(value, CompletionKind::EnumValue, t);
                }
            }
        }
        if t == "Boolean" {
            push("true", CompletionKind::EnumValue, "Boolean");
            push("false", CompletionKind::EnumValue, "Boolean");
        }
    };

    match expect {
        Expect::Field { type_name }
        | Expect::AfterField {
            parent_type: type_name,
            ..
        } => {
            if wants_directive {
                for (name, detail) in BUILTIN_DIRECTIVES {
                    push(name, CompletionKind::Directive, detail);
                }
                return out;
            }
            if let Some(def) = schema.types.get(&type_name) {
                for field in &def.fields {
                    push(&field.name, CompletionKind::Field, &field.type_display);
                }
            }
            push("__typename", CompletionKind::Field, "String");
        }
        Expect::ArgumentName {
            type_name,
            field_name,
        } => {
            if let Some(field) = schema.field(&type_name, &field_name) {
                for arg in &field.args {
                    push(&arg.name, CompletionKind::Argument, &arg.type_display);
                }
            }
        }
        Expect::Value { type_name: Some(t) } => enum_or_bool_values(&mut push, &t),
        Expect::ListValue { type_name: Some(t) } => enum_or_bool_values(&mut push, &t),
        Expect::InputField { type_name } => {
            if let Some(def) = schema.types.get(&type_name) {
                for field in &def.input_fields {
                    push(&field.name, CompletionKind::InputField, &field.type_display);
                }
            }
        }
        Expect::TypeCondition => {
            let mut names: Vec<(&String, &GraphQLTypeDef)> = schema
                .types
                .iter()
                .filter(|(_, def)| {
                    matches!(
                        def.kind,
                        GraphQLTypeKind::Object
                            | GraphQLTypeKind::Interface
                            | GraphQLTypeKind::Union
                    )
                })
                .collect();
            names.sort_by(|a, b| a.0.cmp(b.0));
            for (name, def) in names {
                push(name, CompletionKind::TypeName, &format!("{:?}", def.kind));
            }
        }
        Expect::FragmentName => {
            let mut names: Vec<&String> = fragments.keys().collect();
            names.sort();
            for name in names {
                let ty = fragments
                    .get(name)
                    .map(|t| format!("on {t}"))
                    .unwrap_or_default();
                push(name, CompletionKind::Fragment, &ty);
            }
        }
        Expect::DirectiveName { .. } => {
            for (name, detail) in BUILTIN_DIRECTIVES {
                push(name, CompletionKind::Directive, detail);
            }
        }
        Expect::DirectiveArgs { .. } => {
            if matches!(last_directive.as_deref(), Some("skip") | Some("include")) {
                push("if", CompletionKind::Argument, "Boolean!");
            }
        }
        Expect::Value { type_name: None }
        | Expect::ListValue { type_name: None }
        | Expect::VarDefs
        | Expect::TopLevel => {}
    }
    out
}
