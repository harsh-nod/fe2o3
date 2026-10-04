use vstd::prelude::*;
verus! {
pub open spec fn mutated_pair_valid_v1(d2h_qid: nat, h2d_qid: nat) -> bool {
    d2h_qid < 1024 && h2d_qid < 1024
}
pub proof fn mutated_child_queue_collision_is_rejected_v1()
    ensures !mutated_pair_valid_v1(0, 0),
{}
}
