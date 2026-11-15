//! Deep GraphQL completion resolver: walks the token stream up to the caret
//! maintaining a type-context stack (selection sets, argument lists, input
//! object literals, fragments) and returns the completions valid at that
//! position from the introspected schema.

use std::collections::HashMap;

use super::lexer::{lex, Tok, Token};
use super::schema::{GraphQLSchemaSummary, GraphQLTypeDef, GraphQLTypeKind};

/// One completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    /// Type signature or short description shown beside the label.
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Field,
    Argument,
    EnumValue,
    InputField,
    TypeName,
    Fragment,
    Directive,
}

/// Upper bound on suggestions so pathological schemas cannot flood the UI.
const MAX_COMPLETIONS: usize = 200;

/// What the walker expects at the caret position.
#[derive(Debug, Clone, PartialEq)]
enum Expect {
    /// Between items of the selection set on `type_name`.
    Field { type_name: String },
    /// After a complete field / fragment condition: `(`, `{`, `@` or a
    /// sibling field may follow. `open_type` is the selection-set type a
    /// following `{` would open; `parent_type`+`field_name` resolve args.
    AfterField {
        open_type: String,
        parent_type: String,
        field_name: Option<String>,
    },
    /// Between arguments of `field_name` on `type_name`.
    ArgumentName {
        type_name: String,
        field_name: String,
    },
    /// Expecting a value of the (unwrapped named) type.
    Value { type_name: Option<String> },
    /// Inside a `[...]` list literal expecting an element value.
    ListValue { type_name: Option<String> },
    /// Between input fields of an input object.
    InputField { type_name: String },
    /// After `@` — a directive name; `resume` is restored afterwards.
    DirectiveName { resume: Box<Expect> },
    /// Inside `@dir(...)`; `resume` is restored on `)`.
    DirectiveArgs { resume: Box<Expect> },
    /// After `on` — a fragment type condition.
    TypeCondition,
    /// After `...` — a fragment spread name (or the `on` keyword).
    FragmentName,
    /// Inside variable definitions — no suggestions.
    VarDefs,
    /// Top level between operations.
    TopLevel,
}

#[derive(Debug, Clone)]
enum Scope {
    Selection {
        type_name: String,
    },
    Args {
        parent_type: String,
        field_name: String,
        open_type: String,
    },
    InputObject {
        type_name: String,
    },
}

#[derive(Debug, Clone)]
enum PendingFragment {
    /// After the `fragment` keyword — expecting the definition name.
    WantName,
    /// `fragment Name` seen — expecting `on` then the type condition.
    SeenName,
    /// `fragment Name on Type` seen — expecting the opening `{`.
    HaveType(String),
}

/// Computes completions at `caret` (byte offset) inside a GraphQL document.
/// Returns an empty vec when the caret is inside a string or the position
/// admits nothing (top level, variable definitions, …).
pub fn completions_at(text: &str, caret: usize, schema: &GraphQLSchemaSummary) -> Vec<Completion> {
    if caret > text.len() || !text.is_char_boundary(caret) {
        return Vec::new();
    }
    let tokens = lex(text);
    let Some((prefix, walk_end)) = prefix_and_walk_end(&tokens, text, caret) else {
        return Vec::new();
    };
    let fragments = collect_fragments(&tokens);
    let mut walker = Walker::new(schema);
    walker.run(&tokens[..walk_end]);
    build_completions(
        walker.expect,
        &prefix,
        schema,
        &fragments,
        walker.last_directive,
    )
}

/// Determines the typed prefix at the caret and how many tokens precede it.
/// `None` means the caret sits inside a string literal.
fn prefix_and_walk_end(tokens: &[Token], text: &str, caret: usize) -> Option<(String, usize)> {
    for (idx, t) in tokens.iter().enumerate() {
        if t.end < caret {
            continue; // fully before the caret — part of the walk
        }
        if t.end == caret {
            // A token ending exactly at the caret: a Name contributes the
            // typed prefix; anything else is complete and part of the walk.
            if let Tok::Name(_) = &t.tok {
                if t.start < caret {
                    return Some((text[t.start..caret].to_string(), idx));
                }
            }
            continue;
        }
        // The token extends past the caret.
        if t.start >= caret {
            return Some((String::new(), idx));
        }
        return match &t.tok {
            Tok::Name(_) => Some((text[t.start..caret].to_string(), idx)),
            Tok::Str => None,
            _ => Some((String::new(), idx)),
        };
    }
    Some((String::new(), tokens.len()))
}

/// Collects `fragment Name on Type` definitions from the whole document so
/// `...Name` spreads and fragment bodies can complete against their types.
fn collect_fragments(tokens: &[Token]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut i = 0;
    while i + 3 < tokens.len() {
        let is_def = matches!(&tokens[i].tok, Tok::Name(n) if n == "fragment")
            && matches!(&tokens[i + 2].tok, Tok::Name(n) if n == "on");
        if is_def {
            if let (Tok::Name(name), Tok::Name(ty)) = (&tokens[i + 1].tok, &tokens[i + 3].tok) {
                out.insert(name.clone(), ty.clone());
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    out
}

struct Walker<'a> {
    schema: &'a GraphQLSchemaSummary,
    scopes: Vec<Scope>,
    /// Element types of the currently open `[...]` literals, innermost last.
    lists: Vec<Option<String>>,
    expect: Expect,
    pending_op: Option<String>,
    pending_fragment: Option<PendingFragment>,
    last_directive: Option<String>,
}

impl<'a> Walker<'a> {
    fn new(schema: &'a GraphQLSchemaSummary) -> Self {
        Self {
            schema,
            scopes: Vec::new(),
            lists: Vec::new(),
            expect: Expect::TopLevel,
            pending_op: None,
            pending_fragment: None,
            last_directive: None,
        }
    }

    fn run(&mut self, tokens: &[Token]) {
        let mut i = 0;
        while i < tokens.len() {
            let tok = tokens[i].tok.clone();
            let next = tokens.get(i + 1).map(|t| t.tok.clone());
            let consumed = match tok {
                Tok::Name(name) => self.step_name(&name, next.as_ref()),
                Tok::Punct(c) => self.step_punct(c),
                Tok::Spread => {
                    self.expect = Expect::FragmentName;
                    1
                }
                Tok::Str | Tok::Num => self.step_literal(),
            };
            i += consumed.max(1);
        }
    }

    /// Expectation once a value has been fully consumed, derived from the
    /// enclosing scope.
    fn resume_after_value(&self) -> Expect {
        match self.scopes.last() {
            Some(Scope::Selection { type_name }) => Expect::Field {
                type_name: type_name.clone(),
            },
            Some(Scope::Args {
                parent_type,
                field_name,
                ..
            }) => Expect::ArgumentName {
                type_name: parent_type.clone(),
                field_name: field_name.clone(),
            },
            Some(Scope::InputObject { type_name }) => Expect::InputField {
                type_name: type_name.clone(),
            },
            None => Expect::TopLevel,
        }
    }

    fn step_literal(&mut self) -> usize {
        self.expect = if matches!(self.expect, Expect::ListValue { .. }) {
            let t = self.lists.last().cloned().flatten();
            Expect::ListValue { type_name: t }
        } else if matches!(self.expect, Expect::Value { .. }) {
            self.resume_after_value()
        } else {
            return 1; // literal out of place — ignore
        };
        1
    }

    fn step_name(&mut self, name: &str, next: Option<&Tok>) -> usize {
        match self.expect.clone() {
            Expect::TopLevel => self.step_name_top_level(name),
            Expect::Field { type_name } => self.step_field_name(name, next, &type_name),
            Expect::AfterField { parent_type, .. } => {
                self.step_field_name(name, next, &parent_type)
            }
            Expect::ArgumentName {
                type_name,
                field_name,
            } => {
                if next == Some(&Tok::Punct(':')) {
                    let arg_type = self
                        .schema
                        .arg_type(&type_name, &field_name, name)
                        .map(str::to_string);
                    self.expect = Expect::Value {
                        type_name: arg_type,
                    };
                    return 2;
                }
                1
            }
            Expect::Value { .. } | Expect::ListValue { .. } => self.step_literal(),
            Expect::InputField { type_name } => {
                if next == Some(&Tok::Punct(':')) {
                    let field_type = self.schema.types.get(&type_name).and_then(|t| {
                        t.input_fields
                            .iter()
                            .find(|f| f.name == name)
                            .map(|f| f.type_name.clone())
                    });
                    self.expect = Expect::Value {
                        type_name: field_type,
                    };
                    return 2;
                }
                1
            }
            Expect::DirectiveName { resume } => {
                self.last_directive = Some(name.to_string());
                if next == Some(&Tok::Punct('(')) {
                    self.expect = Expect::DirectiveArgs { resume };
                } else {
                    self.expect = *resume;
                }
                1
            }
            Expect::DirectiveArgs { .. } => 1,
            Expect::TypeCondition => {
                self.expect = Expect::AfterField {
                    open_type: name.to_string(),
                    parent_type: name.to_string(),
                    field_name: None,
                };
                1
            }
            Expect::FragmentName => {
                if name == "on" {
                    self.expect = Expect::TypeCondition;
                } else {
                    self.expect = self.resume_after_value();
                }
                1
            }
            Expect::VarDefs => 1,
        }
    }

    fn step_name_top_level(&mut self, name: &str) -> usize {
        if let Some(pf) = self.pending_fragment.clone() {
            match pf {
                PendingFragment::WantName => {
                    self.pending_fragment = Some(PendingFragment::SeenName);
                }
                PendingFragment::SeenName => {
                    if name != "on" {
                        self.pending_fragment = Some(PendingFragment::HaveType(name.to_string()));
                    }
                }
                PendingFragment::HaveType(..) => {}
            }
            return 1;
        }
        match name {
            "query" | "mutation" | "subscription" => self.pending_op = Some(name.to_string()),
            "fragment" => self.pending_fragment = Some(PendingFragment::WantName),
            _ => {}
        }
        1
    }

    fn step_field_name(&mut self, name: &str, next: Option<&Tok>, parent_type: &str) -> usize {
        if next == Some(&Tok::Punct(':')) {
            // Alias — skip both tokens; the following name is the field.
            return 2;
        }
        let open_type = self
            .schema
            .field(parent_type, name)
            .map(|f| f.type_name.clone())
            .unwrap_or_default();
        self.expect = Expect::AfterField {
            open_type,
            parent_type: parent_type.to_string(),
            field_name: Some(name.to_string()),
        };
        1
    }

    fn step_punct(&mut self, c: char) -> usize {
        match c {
            '{' => self.open_brace(),
            '}' => self.close_brace(),
            '(' => self.open_paren(),
            ')' => self.close_paren(),
            '[' => {
                match self.expect.clone() {
                    Expect::Value { type_name } | Expect::ListValue { type_name } => {
                        self.lists.push(type_name.clone());
                        self.expect = Expect::ListValue { type_name };
                    }
                    _ => {}
                }
                1
            }
            ']' => {
                self.lists.pop();
                self.expect = match self.lists.last().cloned() {
                    Some(t) => Expect::ListValue { type_name: t },
                    None => self.resume_after_value(),
                };
                1
            }
            '@' => {
                self.expect = Expect::DirectiveName {
                    resume: Box::new(self.expect.clone()),
                };
                1
            }
            '$' => {
                if matches!(self.expect, Expect::Value { .. } | Expect::ListValue { .. }) {
                    self.step_literal()
                } else {
                    1
                }
            }
            _ => 1,
        }
    }

    /// `{` — opens a selection set, an input object literal, or (top level)
    /// an anonymous/root operation or fragment body.
    fn open_brace(&mut self) -> usize {
        match self.expect.clone() {
            Expect::TopLevel => {
                if let Some(PendingFragment::HaveType(ty)) = self.pending_fragment.take() {
                    self.scopes.push(Scope::Selection { type_name: ty });
                    self.expect = self.selection_field_expect();
                    return 1;
                }
                let root = self.root_type();
                self.scopes.push(Scope::Selection { type_name: root });
                self.expect = self.selection_field_expect();
                1
            }
            Expect::AfterField { open_type, .. } => {
                if !open_type.is_empty() {
                    self.scopes.push(Scope::Selection {
                        type_name: open_type,
                    });
                }
                self.expect = self.selection_field_expect();
                1
            }
            Expect::Value { type_name } | Expect::ListValue { type_name } => {
                if let Some(t) = &type_name {
                    if self
                        .schema
                        .types
                        .get(t)
                        .is_some_and(|def| def.kind == GraphQLTypeKind::InputObject)
                    {
                        self.scopes.push(Scope::InputObject {
                            type_name: t.clone(),
                        });
                        self.expect = Expect::InputField {
                            type_name: t.clone(),
                        };
                        return 1;
                    }
                }
                self.step_literal()
            }
            _ => 1,
        }
    }

    /// `}` — closes a selection set or an input object literal.
    fn close_brace(&mut self) -> usize {
        match self.scopes.last() {
            Some(Scope::Selection { .. }) | Some(Scope::InputObject { .. }) => {
                self.scopes.pop();
                self.expect = if self.scopes.is_empty() {
                    Expect::TopLevel
                } else {
                    self.resume_after_value()
                };
            }
            // `}` while inside arguments/variable defs — malformed; ignore.
            _ => {}
        }
        1
    }

    /// `(` — argument list of a field, or variable definitions after an
    /// operation keyword.
    fn open_paren(&mut self) -> usize {
        match self.expect.clone() {
            Expect::AfterField {
                open_type,
                parent_type,
                field_name: Some(field_name),
            } => {
                self.scopes.push(Scope::Args {
                    parent_type: parent_type.clone(),
                    field_name: field_name.clone(),
                    open_type,
                });
                self.expect = Expect::ArgumentName {
                    type_name: parent_type,
                    field_name,
                };
            }
            Expect::TopLevel if self.pending_op.is_some() => {
                self.expect = Expect::VarDefs;
            }
            _ => {}
        }
        1
    }

    /// `)` — closes arguments, variable definitions or directive arguments.
    fn close_paren(&mut self) -> usize {
        match self.expect.clone() {
            Expect::ArgumentName { .. } => {
                if let Some(Scope::Args {
                    parent_type,
                    field_name,
                    open_type,
                }) = self.scopes.pop()
                {
                    self.expect = Expect::AfterField {
                        open_type,
                        parent_type,
                        field_name: Some(field_name),
                    };
                } else {
                    self.expect = self.resume_after_value();
                }
            }
            Expect::VarDefs => {
                let root = self.root_type();
                self.expect = Expect::AfterField {
                    open_type: root.clone(),
                    parent_type: root,
                    field_name: None,
                };
                self.pending_op = None;
            }
            Expect::DirectiveArgs { resume } => {
                self.expect = *resume;
            }
            _ => {}
        }
        1
    }

    /// Root type for the pending operation keyword, empty when unknown.
    fn root_type(&mut self) -> String {
        match self.pending_op.take() {
            Some(op) => self
                .schema
                .root_type(&op)
                .map(str::to_string)
                .unwrap_or_default(),
            None => self.schema.query_type.clone(),
        }
    }

    fn selection_field_expect(&mut self) -> Expect {
        match self.scopes.last() {
            Some(Scope::Selection { type_name }) => Expect::Field {
                type_name: type_name.clone(),
            },
            _ => Expect::TopLevel,
        }
    }
}

/// Standard executable directives offered after `@`.
const BUILTIN_DIRECTIVES: &[(&str, &str)] = &[
    ("skip", "Boolean! — skip this field/fragment"),
    ("include", "Boolean! — include this field/fragment"),
    ("deprecated", "String — on field/enum definitions"),
    ("specifiedBy", "String! — custom scalar URL"),
];

fn build_completions(
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

#[cfg(test)]
#[path = "complete_tests.rs"]
mod tests;
