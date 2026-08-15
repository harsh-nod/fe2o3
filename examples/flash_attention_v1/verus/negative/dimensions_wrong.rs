use vstd::prelude::*;

verus! {

pub open spec fn mutated_sequence_v1() -> nat { 16 }

pub proof fn mutated_sequence_is_exact_profile_v1()
    ensures mutated_sequence_v1() == 8,
{
}

}
