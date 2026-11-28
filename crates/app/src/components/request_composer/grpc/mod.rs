mod completion;
mod message;
mod proto;

pub(super) use completion::ProtoJsonCompletionProvider;
pub(super) use message::{render_grpc_message_tab, GrpcMessageTabContext};
pub(super) use proto::render_proto_tab;
