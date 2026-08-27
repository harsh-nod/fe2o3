use vstd::prelude::*;

verus! {

pub open spec fn mutated_union_omits_third_range_v1(index: nat) -> bool {
    ||| 0 <= index < 16
    ||| 16 <= index < 32
    ||| 48 <= index < 64
}

pub proof fn mutated_destination_union_has_gap_v1()
    ensures mutated_union_omits_third_range_v1(32),
{
}

}
