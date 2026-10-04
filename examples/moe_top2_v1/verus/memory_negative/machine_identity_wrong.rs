use vstd::prelude::*;
verus! {
pub open spec fn published_machine_body_identity_v1() -> Seq<u64> {
    seq![0x4728028b85cc3ff4u64, 0x07190de6a70b9c84u64,
         0x4437e9f92fc587e0u64, 0x614940be898346cfu64]
}
pub proof fn mutated_machine_identity_is_exact_v1()
    ensures published_machine_body_identity_v1()[0] == 0x4728028b85cc3ff5u64,
{
}
}
