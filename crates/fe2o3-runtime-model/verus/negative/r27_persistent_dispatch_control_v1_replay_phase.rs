use vstd::prelude::*;
verus! {
pub open spec fn attached_phase_v1() -> nat { 1 }
pub open spec fn data_detached_phase_v1() -> nat { 2 }
pub proof fn mutated_replay_requires_data_detached_v1()
    ensures attached_phase_v1() == data_detached_phase_v1(), {}
}
