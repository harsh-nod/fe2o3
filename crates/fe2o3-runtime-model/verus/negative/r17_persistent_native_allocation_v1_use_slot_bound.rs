use vstd::prelude::*;

verus! {
pub open spec fn mutated_use_count_within_bound_v1(count: nat) -> bool { count <= 65 }
pub proof fn mutated_65th_use_is_rejected_v1()
    ensures !mutated_use_count_within_bound_v1(65),
{}
}
