use vstd::prelude::*;

verus! {

pub open spec fn mutated_fetch_add_return_v1(old_value: int, operand: int) -> int {
    old_value + operand
}

pub proof fn mutated_fetch_add_returns_old_v1(old_value: int, operand: int)
    requires operand != 0,
    ensures mutated_fetch_add_return_v1(old_value, operand) == old_value,
{
}

} // verus!
