use vstd::prelude::*;
verus! {
pub open spec fn published_machine_body_identity_v1() -> Seq<u64> {
    seq![0x60e09278e2901a18u64, 0x67a5a187614a4d33u64,
         0xf12a45a733e266bfu64, 0x35b2693b85975d65u64]
}
pub proof fn mutated_artifact_identity_is_exact_v1()
    ensures published_machine_body_identity_v1()[0] == 0x60e09278e2901a19u64,
{
}
}
