#[path = "../../../crates/fe2o3-lower-mir-kernel/verus/mir_kir_structured_cfg_v3.rs"]
mod cfg;

use vstd::prelude::*;

verus! {

proof fn hostile_mutation_v3()
    ensures cfg::mir_xor_diamond_call_observation_v3(1, 0, 7)[6] == 1,
{
}

}
