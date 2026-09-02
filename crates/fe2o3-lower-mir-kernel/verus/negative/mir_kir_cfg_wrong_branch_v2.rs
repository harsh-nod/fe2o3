use vstd::prelude::*;
verus! {
spec fn mir(input: int, fallback: int) -> int { if input == 0 { input } else { fallback } }
spec fn wrong(input: int, fallback: int) -> int { if input != 0 { input } else { fallback } }
proof fn reversed_branch_is_not_refinement(input: int, fallback: int)
    requires 0 <= input < 4294967296, 0 <= fallback < 4294967296,
    ensures mir(input, fallback) == wrong(input, fallback),
{}
}
fn main() {}
