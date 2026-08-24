//! Bounded, collector-neutral semantic execution traces.
//!
//! This crate is an inert data model and canonical codec. It grants no compiler,
//! proof, runtime, KFD, debugger, or profiler authority.

mod codec;
mod model;

pub use codec::{
    TraceDecodeErrorV1, TraceEncodeErrorV1, decode_trace_v1, encode_trace_v1, encoded_event_len_v1,
    encoded_trace_prefix_len_v1,
};
pub use model::*;
