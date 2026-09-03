//! Bounded, collector-neutral semantic execution traces.
//!
//! This crate is an inert data model and canonical codec. It grants no compiler,
//! proof, runtime, KFD, debugger, or profiler authority.

mod codec;
mod model;

pub use codec::{
    TraceDecodeErrorV1, TraceEncodeErrorV1, decode_trace_v1, decode_trace_v2, encode_trace_v1,
    encode_trace_v2, encoded_event_len_v1, encoded_trace_prefix_len_v1,
    encoded_trace_prefix_len_v2,
};
pub use model::*;

/// V2 uses the same bounded canonical-codec error vocabulary as V1.
pub type TraceEncodeErrorV2 = TraceEncodeErrorV1;
/// V2 uses the same hostile-input decoder error vocabulary as V1.
pub type TraceDecodeErrorV2 = TraceDecodeErrorV1;
