use vstd::prelude::*;
verus! {
pub struct QueueV1 { pub id: nat, pub generation: nat }
pub open spec fn mutated_same_queue_v1(left: QueueV1, right: QueueV1) -> bool {
    left.id == right.id
}
pub proof fn mutated_logical_queue_occurrence_substitution_is_rejected_v1()
    ensures !mutated_same_queue_v1(
        QueueV1 { id: 7, generation: 1 },
        QueueV1 { id: 7, generation: 2 },
    ),
{}
}
