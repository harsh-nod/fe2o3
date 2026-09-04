use vstd::prelude::*;

verus! {

pub open spec fn mutated_drain_allowed_v1(
    _expected_occurrence: nat,
    _observed_occurrence: nat,
    owns_custody: bool,
) -> bool {
    !owns_custody
}

pub proof fn mutated_stale_queue_occurrence_cannot_be_drained_v1(occurrence: nat)
    ensures !mutated_drain_allowed_v1(occurrence, occurrence + 1, false),
{
}

}
