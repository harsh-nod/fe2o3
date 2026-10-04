use vstd::prelude::*;

verus! {
pub open spec fn mutated_dependency_count_within_bound_v1(count: nat) -> bool { count <= 257 }
pub proof fn mutated_257th_dependency_is_rejected_v1()
    ensures !mutated_dependency_count_within_bound_v1(257),
{}
}
