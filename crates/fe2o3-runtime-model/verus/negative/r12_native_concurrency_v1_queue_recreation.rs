use vstd::prelude::*;

verus! {

pub open spec fn mutated_recreate_queue_v1(occurrence: nat) -> nat {
    occurrence
}

pub proof fn mutated_drained_queue_recreation_advances_occurrence_v1(occurrence: nat)
    ensures mutated_recreate_queue_v1(occurrence) == occurrence + 1,
{
}

}
