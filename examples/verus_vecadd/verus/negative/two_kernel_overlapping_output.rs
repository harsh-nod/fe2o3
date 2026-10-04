use vstd::prelude::*;

#[path = "../two_kernel.rs"]
mod model;

verus! {

/// Mutation: two distinct threads are assigned the same exclusive output
/// element instead of their injective identity-indexed elements.
pub proof fn mutated_overlapping_output_ownership_is_race_free(
    allocation: model::permission_model::Allocation,
    left: nat,
    right: nat,
)
    requires
        left != right,
    ensures
        model::permission_model::permissions_are_compatible(
            model::permission_model::exclusive_write(
                model::permission_model::element_region(allocation, left, 4),
            ),
            model::permission_model::exclusive_write(
                model::permission_model::element_region(allocation, left, 4),
            ),
        ), // mutated_overlapping_output_ownership_is_race_free
{
}

} // verus!
