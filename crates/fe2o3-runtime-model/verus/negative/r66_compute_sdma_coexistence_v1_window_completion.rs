// Expected negative: a substituted completion fence is accepted.
use vstd::prelude::*;
verus! {
pub open spec fn completion_unchecked_v1(generation: u32, expected: u32, completion: u32) -> bool {
    generation != 0 && generation == expected
}
pub proof fn mutated_window_completion_v1(generation: u32, expected: u32, completion: u32)
    requires completion_unchecked_v1(generation, expected, completion),
    ensures completion == generation,
{}
}
