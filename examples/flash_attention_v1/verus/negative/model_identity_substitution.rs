use vstd::prelude::*;

verus! {

pub open spec fn mutated_model_identity_v1() -> Seq<u64> {
    seq![
        0xf26a435e375adfebu64,
        0x1753dd7429870532u64,
        0xb90c88bbd46054b9u64,
        0u64,
    ]
}

pub proof fn mutated_model_identity_is_still_exact_v1()
    ensures mutated_model_identity_v1()[3] == 0x498c82408bcd062bu64,
{
}

}
