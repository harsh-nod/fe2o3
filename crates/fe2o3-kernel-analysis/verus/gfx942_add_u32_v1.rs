//! Projected state transition, conditional on valid instruction operands.
//! No decoder, ISA, compiler-entry relation, memory, or launch-authority theorem.
use vstd::prelude::*;
include!("../src/gfx942_integer_semantics_v1/add_u32_body.rs");
include!("../src/gfx942_integer_semantics_v1/execute_body.rs");
include!("../src/gfx942_integer_semantics_v1/mov_b32_body.rs");
include!("../src/gfx942_integer_semantics_v1/mov_prefix_add_body.rs");

verus! {

pub const GFX942_ORDINARY_SGPR_COUNT_V1: usize = 102;
pub const GFX942_MOV_PREFIX_ADD_MAX_INSTRUCTIONS_V1: usize = 64;

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

pub struct Gfx942SMovB32V1 {
    destination: u8,
    source: Gfx942U32SourceV1,
    bytes: [u8; 8],
    byte_len: usize,
}

#[derive(Clone, Copy)]
pub enum Gfx942U32OriginV1 {
    EntrySgpr(u8),
    Constant(u32),
}

pub struct Gfx942MovPrefixAddU32V1 {
    function: String,
    block: u32,
    first_offset: u64,
    add_offset: u64,
    end_offset: u64,
    moves: Vec<Gfx942SMovB32V1>,
    add: Gfx942SAddU32V1,
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
    fn sources(&self) -> (result: [Gfx942U32SourceV1; 2])
        ensures result == self.sources,
    { self.sources }

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

impl Gfx942SMovB32V1 {
    fn source(&self) -> (result: Gfx942U32SourceV1)
        ensures result == self.source,
    { self.source }

    fn destination(&self) -> (result: u8)
        ensures result == self.destination,
    { self.destination }

    fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> (result: u32)
        requires
            self.destination < GFX942_ORDINARY_SGPR_COUNT_V1,
            valid_source(self.source),
        ensures
            result == source_value(self.source, *old(state)),
            final(state).registers@ == old(state).registers@.update(self.destination as int, result),
            final(state).scc == old(state).scc,
            forall|index: int| 0 <= index < GFX942_ORDINARY_SGPR_COUNT_V1
                && index != self.destination as int
                ==> #[trigger] final(state).registers@[index] == old(state).registers@[index],
    {
        gfx942_s_mov_b32_execute_body_v1!(self, state)
    }
}

spec fn valid_move(instruction: Gfx942SMovB32V1) -> bool {
    instruction.destination < 102 && valid_source(instruction.source)
}

spec fn valid_moves(moves: Seq<Gfx942SMovB32V1>) -> bool {
    forall|i: int| 0 <= i < moves.len() ==> valid_move(#[trigger] moves[i])
}

spec fn valid_add(add: Gfx942SAddU32V1) -> bool {
    add.destination < 102 && valid_source(add.sources@[0]) && valid_source(add.sources@[1])
}

spec fn source_origin(source: Gfx942U32SourceV1, origins: Seq<Gfx942U32OriginV1>) -> Gfx942U32OriginV1 {
    match source {
        Gfx942U32SourceV1::Sgpr(register) => origins[register as int],
        Gfx942U32SourceV1::Constant(value) => Gfx942U32OriginV1::Constant(value),
    }
}

spec fn origins_after(moves: Seq<Gfx942SMovB32V1>, count: nat) -> Seq<Gfx942U32OriginV1>
    decreases count,
{
    if count == 0 {
        Seq::new(102, |register: int| Gfx942U32OriginV1::EntrySgpr(register as u8))
    } else {
        let prior = origins_after(moves, (count - 1) as nat);
        let instruction = moves[count - 1];
        prior.update(instruction.destination as int, source_origin(instruction.source, prior))
    }
}

spec fn origin_value(origin: Gfx942U32OriginV1, entry: Gfx942ScalarIntegerStateV1) -> u32 {
    match origin {
        Gfx942U32OriginV1::EntrySgpr(register) => entry.registers@[register as int],
        Gfx942U32OriginV1::Constant(value) => value,
    }
}

spec fn valid_origin(origin: Gfx942U32OriginV1) -> bool {
    match origin {
        Gfx942U32OriginV1::EntrySgpr(register) => register < 102,
        Gfx942U32OriginV1::Constant(_) => true,
    }
}

spec fn untouched(moves: Seq<Gfx942SMovB32V1>, register: int) -> bool {
    forall|i: int| 0 <= i < moves.len() ==> #[trigger] moves[i].destination != register
}

spec fn prefix_sum(moves: Seq<Gfx942SMovB32V1>, add: Gfx942SAddU32V1, entry: Gfx942ScalarIntegerStateV1) -> int {
    let origins = origins_after(moves, moves.len());
    origin_value(source_origin(add.sources@[0], origins), entry) as int
        + origin_value(source_origin(add.sources@[1], origins), entry) as int
}

fn prefix_origins(moves: &[Gfx942SMovB32V1], add: &Gfx942SAddU32V1)
    -> (result: [Gfx942U32OriginV1; 2])
    requires
        moves@.len() < GFX942_MOV_PREFIX_ADD_MAX_INSTRUCTIONS_V1,
        valid_moves(moves@), valid_add(*add),
    ensures
        result@[0] == source_origin(add.sources@[0], origins_after(moves@, moves@.len())),
        result@[1] == source_origin(add.sources@[1], origins_after(moves@, moves@.len())),
        valid_origin(result@[0]), valid_origin(result@[1]),
{
    gfx942_mov_prefix_origins_body_v1!(verus_exec_expr, moves, add, origins, index, [
        invariant
            index <= 102,
            valid_moves(moves@), valid_add(*add), moves@.len() < 64,
            forall|register: int| 0 <= register < 102 ==> valid_origin(#[trigger] origins@[register]),
            forall|register: int| 0 <= register < index
                ==> #[trigger] origins@[register] == Gfx942U32OriginV1::EntrySgpr(register as u8),
        decreases 102 - index,
    ], [
        invariant
            index <= moves@.len(), moves@.len() < 64,
            valid_moves(moves@), valid_add(*add),
            origins@ == origins_after(moves@, index as nat),
            forall|register: int| 0 <= register < 102 ==> valid_origin(#[trigger] origins@[register]),
        decreases moves@.len() - index,
    ])
}

impl Gfx942MovPrefixAddU32V1 {
fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> (result: Gfx942U32AddResultV1)
    requires
        self.moves@.len() < GFX942_MOV_PREFIX_ADD_MAX_INSTRUCTIONS_V1,
        valid_moves(self.moves@), valid_add(self.add),
    ensures
        result.value as int == prefix_sum(self.moves@, self.add, *old(state)) % 0x1_0000_0000,
        result.scc == (prefix_sum(self.moves@, self.add, *old(state)) >= 0x1_0000_0000),
        prefix_sum(self.moves@, self.add, *old(state)) == result.value as int
            + if result.scc { 0x1_0000_0000int } else { 0int },
        final(state).scc == result.scc,
        forall|register: int| 0 <= register < 102 ==> #[trigger] final(state).registers@[register]
            == if register == self.add.destination as int { result.value } else {
                origin_value(origins_after(self.moves@, self.moves@.len())[register], *old(state))
            },
        forall|register: int| 0 <= register < 102 && untouched(self.moves@, register)
            && register != self.add.destination as int
            ==> #[trigger] final(state).registers@[register] == old(state).registers@[register],
{
    let moves = self.moves.as_slice();
    let add = &self.add;
    gfx942_mov_prefix_execute_body_v1!(verus_exec_expr, moves, add, state, index, [
        invariant
            index <= moves@.len(), moves@.len() < 64,
            valid_moves(moves@), valid_add(*add),
            origins_after(moves@, index as nat).len() == 102,
            state.scc == old(state).scc,
            forall|register: int| 0 <= register < 102 ==> #[trigger] state.registers@[register]
                == origin_value(origins_after(moves@, index as nat)[register], *old(state)),
            forall|register: int| 0 <= register < 102 && untouched(moves@, register)
                ==> #[trigger] state.registers@[register] == old(state).registers@[register],
        decreases moves@.len() - index,
    ])
}
}

}
