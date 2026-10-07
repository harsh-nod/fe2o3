use vstd::prelude::*;
verus! {
pub open spec fn mutated_event_registration_count_v1() -> nat { 1 }
pub open spec fn mutated_stream_registration_count_v1() -> nat { 0 }
pub proof fn mutated_event_and_stream_registration_is_atomic_v1()
    ensures mutated_event_registration_count_v1()
        == mutated_stream_registration_count_v1(), {}
}
