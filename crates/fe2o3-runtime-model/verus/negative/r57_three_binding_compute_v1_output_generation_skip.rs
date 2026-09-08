// Expected-negative R57 mutation: output generation advances by two.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_output_generation_v1(opening: nat) -> nat {
    opening + 2
}
pub proof fn mutated_output_generation_skip_is_rejected_v1(opening: nat)
    ensures mutated_output_generation_v1(opening) == opening + 1,
{}
}
fn main() {}
