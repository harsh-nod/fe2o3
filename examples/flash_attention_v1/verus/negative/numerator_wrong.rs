use vstd::prelude::*;

verus! {

pub open spec fn mutated_numerator_update_v1(
    old: int,
    scale: int,
    _value: int,
    current: int,
) -> int {
    old * scale + current
}

pub proof fn mutated_numerator_weights_current_value_v1()
    ensures mutated_numerator_update_v1(3, 2, 7, 5) == 41,
{
}

}
