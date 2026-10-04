use vstd::prelude::*;

verus! {

pub open spec fn mutated_profile_identity_v1() -> Seq<u64> {
    seq![0u64, 0u64, 0u64, 0u64]
}

pub proof fn mutated_profile_identity_is_still_exact_v1()
    ensures mutated_profile_identity_v1()[0] == 0x4dfe870bb76dd32bu64,
{
}

}
