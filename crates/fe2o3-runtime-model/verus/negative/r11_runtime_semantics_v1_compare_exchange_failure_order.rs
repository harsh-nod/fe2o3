use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum MemoryOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

pub open spec fn mutated_compare_exchange_order_pair_v1(
    _success: MemoryOrderV1,
    _failure: MemoryOrderV1,
) -> bool {
    true
}

pub proof fn mutated_release_failure_order_is_rejected_v1()
    ensures !mutated_compare_exchange_order_pair_v1(
        MemoryOrderV1::AcquireRelease,
        MemoryOrderV1::Release,
    ),
{
}

} // verus!
