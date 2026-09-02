use vstd::prelude::*;
verus! {
spec fn mir(input: int, fallback: int) -> int { if input == 0 { input } else { fallback } }
spec fn wrong_phi(input: int, fallback: int) -> int { if input == 0 { fallback } else { input } }
proof fn swapped_edge_arguments_are_not_refinement(input: int, fallback: int)
    requires 0 <= input < 4294967296, 0 <= fallback < 4294967296,
    ensures mir(input, fallback) == wrong_phi(input, fallback),
{}
}
fn main() {}
