#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod model;

pub use codec::{
    ProtocolCodecErrorV1, decode_request_line_v1, decode_response_line_v1, encode_response_line_v1,
    read_request_line_v1,
};
pub use model::*;
