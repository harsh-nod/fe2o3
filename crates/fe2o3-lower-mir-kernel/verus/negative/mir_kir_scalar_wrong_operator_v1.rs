use vstd::prelude::*;

verus! {
spec fn norm(value: int) -> int { value % 4294967296 }
spec fn mir_add(left: int, right: int) -> int { norm(left + right) }
spec fn kir_subtract(left: int, right: int) -> int { norm(left - right) }

proof fn hostile_operator_mutation_is_not_a_refinement(left: int, right: int)
    ensures mir_add(left, right) == kir_subtract(left, right),
{
}
}

fn main() {}
