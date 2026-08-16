use vstd::prelude::*;

verus! {

pub open spec fn mutated_tail_value_v1(index: nat) -> int {
    if index < 64 { 0 } else { 1 }
}

pub proof fn mutated_unused_tail_is_nonzero_v1()
    ensures mutated_tail_value_v1(64) == 0,
{
}

}
