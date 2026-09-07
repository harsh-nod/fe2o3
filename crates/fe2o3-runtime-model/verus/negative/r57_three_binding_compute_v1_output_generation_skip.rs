// Expected-negative R57 mutation: C content generation skips a successor.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_output_generation_skip_is_exact_v1() -> bool { true }
pub proof fn mutated_output_generation_skip_is_rejected_v1()
    ensures !mutated_output_generation_skip_is_exact_v1(),
{}
}
fn main() {}
