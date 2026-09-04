use vstd::prelude::*;
verus! {
pub struct OccurrenceV1 { pub logical: nat, pub native: nat }
pub open spec fn mutated_same_occurrence_v1(left: OccurrenceV1, right: OccurrenceV1) -> bool {
    left.logical == right.logical
}
pub proof fn mutated_native_queue_id_substitution_is_rejected_v1()
    ensures !mutated_same_occurrence_v1(
        OccurrenceV1 { logical: 7, native: 0 },
        OccurrenceV1 { logical: 7, native: 1 },
    ),
{}
}
