use vstd::prelude::*;

verus! {

pub open spec fn mutated_postpublication_cancel_retains_v1(cancelled: bool) -> bool {
    !cancelled
}

pub proof fn mutated_published_cancellation_retains_leases_v1()
    ensures mutated_postpublication_cancel_retains_v1(true),
{
}

} // verus!
