use vstd::prelude::*;
verus! {
spec fn norm(value: int) -> int { value % 4294967296 }
proof fn swapped_subtraction_is_not_a_composition(left: int, right: int)
    ensures norm(left - right) == norm(right - left),
{}
}
fn main() {}
