use vstd::prelude::*;

verus! {

pub open spec fn mutated_capability_admitted_v1(
    stable: bool,
    multi_queue: bool,
    queue_count: nat,
    max_queues: nat,
) -> bool {
    stable && multi_queue && 1 <= queue_count <= max_queues
}

pub proof fn mutated_single_queue_capability_is_rejected_v1(max_queues: nat)
    requires max_queues >= 2,
    ensures !mutated_capability_admitted_v1(true, true, 1, max_queues),
{
}

}
