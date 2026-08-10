use vstd::prelude::*;

#[path = "../two_kernel.rs"]
mod model;

verus! {

/// Mutation: a shared-read permission is treated as readable even though its
/// explicit initialization premise is false.
pub proof fn mutated_uninitialized_input_is_readable(
    region: model::permission_model::ByteRegion,
)
    ensures
        model::permission_model::capability_can_read(
            model::permission_model::RegionCapability {
                permission: model::permission_model::shared_read(region),
                initialized: false,
            },
        ), // mutated_uninitialized_input_is_readable
{
}

} // verus!
