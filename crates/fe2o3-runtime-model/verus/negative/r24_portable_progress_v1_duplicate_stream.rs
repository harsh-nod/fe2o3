use vstd::prelude::*;
verus! {
// Mutation: registration increments the active count even when the stream
// coordinate duplicates an existing registration.
pub open spec fn mutated_register_stream_v1(active: nat, _stream_duplicate: bool) -> nat {
    active + 1
}
pub proof fn mutated_duplicate_stream_registration_is_atomic_v1()
    ensures mutated_register_stream_v1(11, true) == 11, {}
}
