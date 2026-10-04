use vstd::prelude::*;
verus! {
pub open spec fn effect_identity_v1() -> Seq<u64> {
    seq![0xf993ef6952da81e5u64, 0x63100577b239770eu64,
         0x912cc5b56bf803bfu64, 0xce4e47436f726172u64]
}
pub proof fn mutated_effect_identity_is_exact_v1()
    ensures effect_identity_v1()[3] == 0xce4e47436f726173u64,
{
}
}
