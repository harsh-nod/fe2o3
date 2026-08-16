use vstd::prelude::*;
verus! {
pub open spec fn analyzer_profile_identity_v1() -> Seq<u64> {
    seq![0xa4ec224c4cd422a7u64, 0xf55a26a0ea7ac6f1u64,
         0x350de9b2ff0a02c8u64, 0xfc88ce9ba1d212b8u64]
}
pub proof fn mutated_analyzer_identity_is_exact_v1()
    ensures analyzer_profile_identity_v1()[2] == 0x350de9b2ff0a02c9u64,
{
}
}
