// Expected-negative R45 mutation: target batch signal coordinates may drift.
use vstd::prelude::*;
verus! {
pub open spec fn target_slot_v1() -> nat { 61 }
pub open spec fn completion_signal_slot_v1() -> nat { 62 }
pub proof fn mutated_target_completion_signal_is_exact_v1()
    ensures target_slot_v1() == completion_signal_slot_v1(),
{}
}
