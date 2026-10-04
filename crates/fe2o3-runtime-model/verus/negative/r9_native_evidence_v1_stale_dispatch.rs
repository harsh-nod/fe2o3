use vstd::prelude::*;

verus! {

pub open spec fn mutated_dispatch_current_v1(device_current: bool, code_current: bool) -> bool {
    device_current || code_current
}

pub proof fn mutated_any_stale_surface_blocks_dispatch_v1()
    ensures !mutated_dispatch_current_v1(true, false),
{
}

} // verus!
