use vstd::prelude::*;

#[path = "../vecadd.rs"]
mod model;

verus! {

/// Expected failure: an exclusive output capability cannot alias an
/// initialized shared-read capability for the same non-empty byte region.
pub proof fn mutated_same_source_output_alias_is_compatible(region: model::ByteRegion)
    requires
        region.byte_length > 0,
    ensures
        model::permissions_are_compatible(
            model::output_write_capability(region, false).permission,
            model::initialized_read_capability(region).permission,
        ),
{
}

} // verus!
