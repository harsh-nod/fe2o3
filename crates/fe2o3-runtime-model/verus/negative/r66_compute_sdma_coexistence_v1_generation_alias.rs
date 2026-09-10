// Expected negative: a changed generation makes the same allocation disjoint.
use vstd::prelude::*;
verus! {
pub open spec fn generation_sensitive_disjoint_v1(left: u64, right: u64, left_generation: u64, right_generation: u64) -> bool {
    left != 0 && right != 0 && left_generation != 0 && right_generation != 0
        && (left != right || left_generation != right_generation)
}
pub proof fn mutated_generation_alias_v1(left: u64, right: u64, left_generation: u64, right_generation: u64)
    requires generation_sensitive_disjoint_v1(left, right, left_generation, right_generation),
    ensures left != right,
{}
}
