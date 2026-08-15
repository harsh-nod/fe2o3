use vstd::prelude::*;

verus! {

pub open spec fn mutated_tensor_index_v1(row: nat, column: nat) -> nat {
    row * 8 + column
}

pub proof fn mutated_last_tensor_coordinate_is_exact_v1()
    ensures mutated_tensor_index_v1(7, 15) == 127,
{
}

}
