use vstd::prelude::*;

#[path = "../two_kernel.rs"]
mod model;

verus! {

/// Mutation: a four-byte element starting at the largest usize address is
/// claimed to have a representable exclusive end address.
pub proof fn mutated_f32_address_overflow_is_representable(
    allocation: model::permission_model::Allocation,
)
    requires
        allocation.base_address == usize::MAX as nat,
    ensures
        model::permission_model::element_byte_end(allocation, 0, 4)
            <= usize::MAX as nat, // mutated_f32_address_overflow_is_representable
{
}

} // verus!
