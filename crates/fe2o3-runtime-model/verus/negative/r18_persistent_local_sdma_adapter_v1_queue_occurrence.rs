use vstd::prelude::*;
verus! {
pub struct OccurrenceV1 { pub logical: nat, pub native: nat, pub occurrence: nat }
pub open spec fn mutated_same_child_occurrence_v1(
    left: OccurrenceV1,
    right: OccurrenceV1,
) -> bool {
    left.logical == right.logical && left.native == right.native
}
pub proof fn mutated_native_child_queue_reuse_is_rejected_v1()
    ensures !mutated_same_child_occurrence_v1(
        OccurrenceV1 { logical: 7, native: 0, occurrence: 1 },
        OccurrenceV1 { logical: 7, native: 0, occurrence: 2 },
    ),
{}
}
