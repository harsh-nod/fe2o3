//! Universal arithmetic of the executable shared model, not ISA conformance.
use vstd::prelude::*;
include!("../src/gfx942_integer_semantics_v1/add_u32_body.rs");

verus! {

pub struct Gfx942U32AddResultV1 {
    pub value: u32,
    pub scc: bool,
}

fn gfx942_add_u32_v1(a: u32, b: u32) -> (result: Gfx942U32AddResultV1)
    ensures
        result.value as int == (a as int + b as int) % 0x1_0000_0000,
        result.scc == (a as int + b as int >= 0x1_0000_0000),
        a as int + b as int == result.value as int
            + if result.scc { 0x1_0000_0000int } else { 0int },
{
    gfx942_add_u32_body_v1!(a, b)
}

}
