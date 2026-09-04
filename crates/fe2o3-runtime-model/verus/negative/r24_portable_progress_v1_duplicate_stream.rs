use vstd::prelude::*;
verus! {
pub open spec fn mutated_duplicate_stream_was_admitted_v1() -> bool { true }
pub proof fn mutated_duplicate_stream_registration_is_atomic_v1()
    ensures !mutated_duplicate_stream_was_admitted_v1(), {}
}
