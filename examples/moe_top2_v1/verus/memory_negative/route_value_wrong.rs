use vstd::prelude::*;
verus! {
pub open spec fn drop_route_v1() -> nat { 4294967295 }
pub proof fn mutated_drop_route_is_in_range_v1()
    ensures drop_route_v1() < 16,
{
}
}
