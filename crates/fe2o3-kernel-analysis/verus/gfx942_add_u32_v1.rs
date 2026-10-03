//! Projected state transition, conditional on valid instruction operands.
//! No decoder, ISA, compiler-entry relation, memory, or launch-authority theorem.
use vstd::prelude::*;
include!("../src/gfx942_integer_semantics_v1/add_u32_body.rs");
include!("../src/gfx942_integer_semantics_v1/execute_body.rs");

verus! {

pub const GFX942_ORDINARY_SGPR_COUNT_V1: usize = 102;

pub struct Gfx942U32AddResultV1 {
    pub value: u32,
    pub scc: bool,
}

#[derive(Clone, Copy)]
pub enum Gfx942U32SourceV1 {
    Sgpr(u8),
    Constant(u32),
}

pub struct Gfx942ScalarIntegerStateV1 {
    registers: [u32; GFX942_ORDINARY_SGPR_COUNT_V1],
    scc: bool,
}

pub struct Gfx942SAddU32V1 {
    destination: u8,
    sources: [Gfx942U32SourceV1; 2],
    bytes: [u8; 8],
    byte_len: usize,
}

spec fn valid_source(source: Gfx942U32SourceV1) -> bool {
    match source {
        Gfx942U32SourceV1::Sgpr(index) => index < GFX942_ORDINARY_SGPR_COUNT_V1,
        Gfx942U32SourceV1::Constant(_) => true,
    }
}

spec fn source_value(source: Gfx942U32SourceV1, state: Gfx942ScalarIntegerStateV1) -> u32 {
    match source {
        Gfx942U32SourceV1::Sgpr(index) => state.registers@[index as int],
        Gfx942U32SourceV1::Constant(value) => value,
    }
}

spec fn input_sum(instruction: Gfx942SAddU32V1, state: Gfx942ScalarIntegerStateV1) -> int {
    source_value(instruction.sources@[0], state) as int
        + source_value(instruction.sources@[1], state) as int
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

impl Gfx942SAddU32V1 {
    fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> (result: Gfx942U32AddResultV1)
        requires
            self.destination < GFX942_ORDINARY_SGPR_COUNT_V1,
            valid_source(self.sources@[0]),
            valid_source(self.sources@[1]),
        ensures
            result.value as int == input_sum(*self, *old(state)) % 0x1_0000_0000,
            result.scc == (input_sum(*self, *old(state)) >= 0x1_0000_0000),
            input_sum(*self, *old(state)) == result.value as int
                + if result.scc { 0x1_0000_0000int } else { 0int },
            final(state).registers@ == old(state).registers@.update(self.destination as int, result.value),
            final(state).scc == result.scc,
            forall|index: int| 0 <= index < GFX942_ORDINARY_SGPR_COUNT_V1
                && index != self.destination as int
                ==> #[trigger] final(state).registers@[index] == old(state).registers@[index],
    {
        gfx942_s_add_u32_execute_body_v1!(self, state)
    }
}

}
