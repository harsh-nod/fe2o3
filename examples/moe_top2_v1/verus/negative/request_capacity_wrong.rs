use vstd::prelude::*;

verus! {

pub open spec fn mutated_admitted_count_v1(requested: nat) -> nat {
    requested
}

pub proof fn mutated_admission_respects_capacity_v1()
    ensures mutated_admitted_count_v1(5) <= 4,
{
}

}
