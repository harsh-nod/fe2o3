use vstd::prelude::*;

verus! {

pub open spec fn row_elements_v1() -> nat { 64 }

pub open spec fn mutated_output_index_v1(lane: nat) -> nat {
    if lane == 63 { 0 } else { lane }
}

pub proof fn mutated_output_ownership_is_injective_v1(left: nat, right: nat)
    requires
        left < row_elements_v1(),
        right < row_elements_v1(),
        left != right,
    ensures mutated_output_index_v1(left) != mutated_output_index_v1(right),
{
}

} // verus!
