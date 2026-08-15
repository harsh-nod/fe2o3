use vstd::prelude::*;

verus! {

pub open spec fn mutated_denominator_v1() -> real { 0real }

pub proof fn mutated_denominator_is_positive_v1()
    ensures mutated_denominator_v1() > 0real,
{
}

}
