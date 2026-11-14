use prost::Message as _;
use prost_reflect::{DynamicMessage, MessageDescriptor};
use tonic::codec::{Codec, DecodeBuf, EncodeBuf};
use tonic::Status;

/// tonic codec bridging `prost-reflect` dynamic messages so the response
/// type is resolved at runtime from the schema instead of build-time codegen.
/// Encoding needs no descriptor (the dynamic message carries its own), only
/// decoding does.
#[derive(Clone)]
pub struct DynamicCodec {
    decode_desc: MessageDescriptor,
}

impl DynamicCodec {
    pub fn new(decode_desc: MessageDescriptor) -> Self {
        Self { decode_desc }
    }
}

impl Codec for DynamicCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynamicEncoder;
    type Decoder = DynamicDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        DynamicEncoder
    }

    fn decoder(&mut self) -> Self::Decoder {
        DynamicDecoder(self.decode_desc.clone())
    }
}

pub struct DynamicEncoder;

impl tonic::codec::Encoder for DynamicEncoder {
    type Item = DynamicMessage;
    type Error = Status;

    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        item.encode(dst)
            .map_err(|e| Status::internal(format!("Failed to encode request message: {e}")))
    }
}

pub struct DynamicDecoder(MessageDescriptor);

impl tonic::codec::Decoder for DynamicDecoder {
    type Item = DynamicMessage;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        DynamicMessage::decode(self.0.clone(), src)
            .map(Some)
            .map_err(|e| Status::internal(format!("Failed to decode response message: {e}")))
    }
}
