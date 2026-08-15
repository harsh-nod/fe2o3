use vstd::prelude::*;

#[path = "../lds_tiled_slice1_source_refinement.rs"]
mod model;

verus! {

/// Mutation: an A slice one element short is claimed to pass the exact source
/// guard and therefore refine the fixed canonical profile.
pub proof fn mutated_short_a_is_admitted_by_exact_source_guard_v1()
    ensures model::source_guard_accepts_v1(0, 255, 256, 256),
{
}

} // verus!
