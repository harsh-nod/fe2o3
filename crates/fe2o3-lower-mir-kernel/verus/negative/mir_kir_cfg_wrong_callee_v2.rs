use vstd::prelude::*;
verus! {
spec fn helper(input: int, fallback: int) -> int { if input == 0 { input } else { fallback } }
spec fn substituted(input: int, fallback: int) -> int { if input == 0 { fallback } else { input } }
proof fn callee_substitution_is_not_refinement(input: int, fallback: int)
    requires 0 <= input < 4294967296, 0 <= fallback < 4294967296,
    ensures helper(input, fallback) == substituted(input, fallback),
{}
}
fn main() {}
