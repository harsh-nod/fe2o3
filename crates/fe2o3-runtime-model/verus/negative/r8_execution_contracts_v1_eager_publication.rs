use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum OperationPhaseV1 {
    Reserved,
    Published,
}

pub open spec fn mutated_reserve_copy_v1() -> OperationPhaseV1 {
    OperationPhaseV1::Published
}

pub proof fn mutated_reservation_is_deferred_v1()
    ensures mutated_reserve_copy_v1() == OperationPhaseV1::Reserved,
{
}

} // verus!
