use vstd::prelude::*;

verus! {

pub open spec fn mutated_row_width_v1() -> nat { 63 }

pub proof fn mutated_specialization_keeps_width_64_v1()
    ensures mutated_row_width_v1() == 64,
{
}

}
