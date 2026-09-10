// Expected negative: a retained slot no longer matches its current ring generation.
use vstd::prelude::*;
verus! {
pub open spec fn generation_unchecked_v1(generation: u32, expected: u32, completion: u32) -> bool {
    generation != 0 && completion == generation
}
pub proof fn mutated_window_generation_v1(generation: u32, expected: u32, completion: u32)
    requires generation_unchecked_v1(generation, expected, completion),
    ensures generation == expected,
{}
}
