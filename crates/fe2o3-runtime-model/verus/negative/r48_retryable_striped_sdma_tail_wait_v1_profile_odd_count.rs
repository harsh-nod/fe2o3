// Expected-negative R48 mutation: an odd striped queue count is admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_standalone_admission_v1(queues: nat) -> bool {
    2 <= queues <= 16
}
pub proof fn odd_queue_count_is_rejected_v1()
    ensures !mutated_standalone_admission_v1(7),
{}
}
