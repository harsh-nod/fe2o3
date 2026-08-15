use vstd::prelude::*;

#[path = "../lds_tiled_edges_alpha_beta.rs"]
mod model;

verus! {

/// Mutation: floor division omits the final K phase when K has a tail.
pub open spec fn mutated_floor_phase_count_v1(k: nat) -> nat {
    k / 16
}

pub proof fn mutated_floor_phases_cover_k_tail_v1()
    ensures 17 <= mutated_floor_phase_count_v1(17) * 16,
{
}

} // verus!
