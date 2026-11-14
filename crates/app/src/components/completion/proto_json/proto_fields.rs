//! Helpers over protobuf field descriptors: field lookup, message descent,
//! and human-readable labels for the completion detail line.

use zest_core::grpc::{FieldDescriptor, Kind, MessageDescriptor};

pub(super) type FieldRef = FieldDescriptor;

pub(super) fn find_field(desc: &MessageDescriptor, name: &str) -> Option<FieldRef> {
    desc.fields().find(|f| f.name() == name)
}

/// The message type a field points at, if any (map entries excluded —
/// key/value suggestions inside a map literal are noise).
pub(super) fn field_as_message(field: &FieldRef) -> Option<MessageDescriptor> {
    if field.is_map() {
        return None;
    }
    match field.kind() {
        Kind::Message(m) => Some(m),
        _ => None,
    }
}

/// Human-readable type of a field for the completion detail line.
pub(super) fn type_label(field: &FieldRef) -> String {
    let cardinality = if field.is_list() {
        "repeated "
    } else if field.is_map() {
        "map "
    } else {
        ""
    };
    let kind = match field.kind() {
        Kind::Message(m) => m.full_name().to_string(),
        Kind::Enum(e) => e.full_name().to_string(),
        other => scalar_label(&other).to_string(),
    };
    format!("{cardinality}{kind}")
}

fn scalar_label(kind: &Kind) -> &'static str {
    match kind {
        Kind::Double => "double",
        Kind::Float => "float",
        Kind::Int32 => "int32",
        Kind::Int64 => "int64",
        Kind::Uint32 => "uint32",
        Kind::Uint64 => "uint64",
        Kind::Sint32 => "sint32",
        Kind::Sint64 => "sint64",
        Kind::Fixed32 => "fixed32",
        Kind::Fixed64 => "fixed64",
        Kind::Sfixed32 => "sfixed32",
        Kind::Sfixed64 => "sfixed64",
        Kind::Bool => "bool",
        Kind::String => "string",
        Kind::Bytes => "bytes",
        Kind::Message(_) | Kind::Enum(_) => "",
    }
}
