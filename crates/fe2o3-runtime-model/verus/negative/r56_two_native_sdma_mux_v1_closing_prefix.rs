// Expected-negative R56 mutation: closing-currentness loss after two confirmed
// publications is falsely relabeled as a zero-publication stale opening.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_closing_failure_prefix_v1() -> nat { 0 }
pub proof fn mutated_closing_failure_prefix_is_rejected_v1()
    ensures mutated_closing_failure_prefix_v1() == 2,
{}
}
fn main() {}
