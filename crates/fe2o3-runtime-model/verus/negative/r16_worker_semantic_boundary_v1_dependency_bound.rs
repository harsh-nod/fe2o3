use vstd::prelude::*;

verus! {

pub open spec fn mutated_dependencies_within_bound_v1(count: nat) -> bool {
    count <= 257
}

pub proof fn mutated_dependency_count_above_v5_bound_is_rejected_v1()
    ensures !mutated_dependencies_within_bound_v1(257),
{
}

}
