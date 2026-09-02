use vstd::prelude::*;
verus! {
spec fn mir(input: int, fallback: int) -> int { if input == 0 { input } else { fallback } }
proof fn wrong_return_is_not_refinement(input: int, fallback: int)
    requires 0 <= input < 4294967296, 0 <= fallback < 4294967296,
    ensures mir(input, fallback) == 0,
{}
}
fn main() {}
