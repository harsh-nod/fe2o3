use vstd::prelude::*;

verus! {

pub open spec fn mutated_model_identity_v1() -> Seq<u64> {
    seq![
        0xf8543b2709377789u64,
        0x0dd0d1fab0767924u64,
        0x21c1d3c64df6571cu64,
        0u64,
    ]
}

pub proof fn mutated_model_identity_is_still_exact_v1()
    ensures mutated_model_identity_v1()[3] == 0x83c91b3ffa361da7u64,
{
}

}
