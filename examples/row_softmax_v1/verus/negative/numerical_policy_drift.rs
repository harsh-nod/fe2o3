use vstd::prelude::*;

verus! {

pub open spec fn mutated_absolute_tolerance_nanos_v1() -> nat { 3001 }

pub proof fn mutated_numerical_policy_matches_reviewed_v1()
    ensures mutated_absolute_tolerance_nanos_v1() == 3000,
{
}

}
