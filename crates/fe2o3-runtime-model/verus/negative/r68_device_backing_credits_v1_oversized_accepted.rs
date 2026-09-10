// Removing the native profile upper bound accepts an oversized backing charge.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_projection_v1(backing: u64) -> Option<Seq<u64>> {
    if backing == 0 { None }
    else { Some(Seq::new(19, |i: int| if i == 3 { backing } else if i == 18 { 1u64 } else { 0u64 })) }
}
pub proof fn mutated_oversized_accepted_v1(backing: u64)
    requires backing > 206158430208,
    ensures mutated_projection_v1(backing).is_none(),
{}
}
