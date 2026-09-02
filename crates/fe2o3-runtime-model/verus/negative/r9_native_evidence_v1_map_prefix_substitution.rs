use vstd::prelude::*;

verus! {

pub open spec fn mutated_map_prefix_v1(
    previous_prefix: nat,
    cumulative_n_success: nat,
) -> nat {
    previous_prefix + cumulative_n_success
}

pub proof fn mutated_failed_map_retains_exact_prefix_v1(
    previous_prefix: nat,
    cumulative_n_success: nat,
)
    requires previous_prefix > 0,
    ensures mutated_map_prefix_v1(previous_prefix, cumulative_n_success) == cumulative_n_success,
{
}

} // verus!
