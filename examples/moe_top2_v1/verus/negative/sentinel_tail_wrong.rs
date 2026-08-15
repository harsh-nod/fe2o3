use vstd::prelude::*;

verus! {

pub open spec fn mutated_tail_value_v1(_slot: nat) -> nat { 0 }

pub proof fn mutated_unused_tail_is_sentinel_v1()
    ensures mutated_tail_value_v1(12) == 16,
{
}

}
