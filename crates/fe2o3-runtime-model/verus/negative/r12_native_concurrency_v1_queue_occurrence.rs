use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct QueueOccurrenceV1 {
    pub device: nat,
    pub queue_id: nat,
    pub occurrence: nat,
}

pub open spec fn mutated_queue_matches_v1(a: QueueOccurrenceV1, b: QueueOccurrenceV1) -> bool {
    a.device == b.device && a.queue_id == b.queue_id
}

pub proof fn mutated_queue_occurrence_substitution_is_rejected_v1(
    queue: QueueOccurrenceV1,
)
    ensures !mutated_queue_matches_v1(queue, QueueOccurrenceV1 {
        occurrence: queue.occurrence + 1,
        ..queue
    }),
{
}

}
