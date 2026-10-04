use vstd::prelude::*;

verus! {

pub open spec fn mutated_rmw_return_v1(old: int, operand: int) -> int {
    old + operand
}

pub proof fn mutated_atomic_rmw_returns_old_value_v1(old: int, operand: int)
    requires operand != 0,
    ensures mutated_rmw_return_v1(old, operand) == old,
{
}

} // verus!
