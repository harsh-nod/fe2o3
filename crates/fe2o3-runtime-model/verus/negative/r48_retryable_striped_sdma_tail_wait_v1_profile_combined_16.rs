// Expected-negative R48 mutation: combined mode admits all sixteen queues.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_combined_admission_v1(queues: nat) -> bool {
    2 <= queues <= 16 && queues % 2 == 0
}
pub proof fn combined_sixteen_is_rejected_v1()
    ensures !mutated_combined_admission_v1(16),
{}
}
