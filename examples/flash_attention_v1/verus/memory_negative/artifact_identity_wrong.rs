use vstd::prelude::*;
verus! {
pub open spec fn artifact_identity_v1() -> Seq<u64> {
    seq![0xf4b3af45a48151fbu64, 0x2e24fea004a77d21u64,
         0x9f64944ea155c276u64, 0x710de05b25ad9651u64]
}
pub proof fn mutated_artifact_identity_is_exact_v1()
    ensures artifact_identity_v1()[0] == 0xf4b3af45a48151fau64,
{
}
}
