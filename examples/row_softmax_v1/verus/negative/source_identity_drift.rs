use vstd::prelude::*;

verus! {

pub open spec fn mutated_source_identity_v1() -> nat {
    0x9551d13970d1e6d6
}

pub proof fn mutated_source_identity_matches_reviewed_v1()
    ensures mutated_source_identity_v1()
        == 0x9551d13970d1e6d5,
{
}

}
