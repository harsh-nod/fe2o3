use vstd::prelude::*;
verus! {
pub open spec fn detached_generation_v1() -> nat { 13 }
pub open spec fn mutated_evicted_generation_v1() -> nat { 0 }
pub proof fn mutated_control_eviction_preserves_detached_generation_v1()
    ensures mutated_evicted_generation_v1() == detached_generation_v1(), {}
}
