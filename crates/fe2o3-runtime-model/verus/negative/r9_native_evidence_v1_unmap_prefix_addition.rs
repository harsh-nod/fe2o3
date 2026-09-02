use vstd::prelude::*;

verus! {

pub open spec fn mutated_unmap_prefix_v1(
    previous_prefix: nat,
    cumulative_n_success: nat,
) -> nat {
    previous_prefix + cumulative_n_success
}

pub proof fn mutated_compensation_retains_absolute_cumulative_prefix_v1(
    previous_prefix: nat,
    cumulative_n_success: nat,
)
    requires previous_prefix > 0,
    ensures mutated_unmap_prefix_v1(previous_prefix, cumulative_n_success)
        == cumulative_n_success,
{
}

} // verus!
