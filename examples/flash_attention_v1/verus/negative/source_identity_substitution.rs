use vstd::prelude::*;

verus! {

pub open spec fn mutated_source_identity_v1() -> Seq<u64> {
    seq![
        0x2b00a64e43e69c41u64,
        0x6e70080e013edf90u64,
        0xe861fef94ee66441u64,
        0u64,
    ]
}

pub proof fn mutated_source_identity_is_still_exact_v1()
    ensures mutated_source_identity_v1()[3] == 0xda93d2c11b3e8f17u64,
{
}

}
