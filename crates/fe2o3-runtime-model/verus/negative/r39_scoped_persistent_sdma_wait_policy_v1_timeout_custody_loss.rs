// Expected-negative R39 mutation: timeout substitutes the exact R37 stream
// frame while retaining the other modeled custody coordinates.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)]
pub struct FrameV1 { pub predecessor: nat, pub current: nat, pub successor: nat }
pub struct StateV1 { pub stream_frame: FrameV1 }

pub open spec fn mutated_timeout_v1(frame: FrameV1) -> StateV1 {
    StateV1 { stream_frame: FrameV1 {
        predecessor: frame.predecessor,
        current: frame.current + 1,
        successor: frame.successor,
    } }
}

pub proof fn mutated_timeout_retains_exact_stream_frame_v1(frame: FrameV1)
    ensures mutated_timeout_v1(frame).stream_frame == frame,
{}
}
