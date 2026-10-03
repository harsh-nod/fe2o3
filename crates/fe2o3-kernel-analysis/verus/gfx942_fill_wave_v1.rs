//! Shared complete closed-wave projection; not ISA or compiler refinement.
use vstd::prelude::*;
include!("../src/gfx942_fill_wave_v1/body.rs");

verus! {
#[derive(Clone, Copy)]
pub struct Gfx942FillWaveStateV1 {
    pub sgprs: [u32; 8],
    pub vgprs: [[u32; 4]; 64],
    pub exec_mask: u64,
    pub vcc: u64,
    pub scc: bool,
}
#[derive(Clone, Copy)]
pub struct Gfx942FillStoreV1 {
    pub address: u64,
    pub value: u32,
}
pub struct Gfx942FillWaveExecutionV1 {
    pub state: Gfx942FillWaveStateV1,
    pub kernarg_read_address: u64,
    pub stores: [Option<Gfx942FillStoreV1>; 64],
    pub retired_instructions: u8,
    pub terminal_pc: u64,
}

spec fn pair_spec(low: u32, high: u32) -> u64 {
    (low as u64) | ((high as u64) << 32)
}
fn pair(low: u32, high: u32) -> (result: u64)
    ensures result == pair_spec(low, high),
{ gfx942_fill_pair_body_v1!(low, high) }

spec fn word_spec(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}
fn word(a: u8, b: u8, c: u8, d: u8) -> (result: u32)
    ensures result == word_spec(a, b, c, d),
{ gfx942_fill_word_body_v1!(a, b, c, d) }

spec fn words_spec(bytes: Seq<u8>) -> Seq<u32> {
    seq![word_spec(bytes[0], bytes[1], bytes[2], bytes[3]),
         word_spec(bytes[4], bytes[5], bytes[6], bytes[7]),
         word_spec(bytes[8], bytes[9], bytes[10], bytes[11]),
         word_spec(bytes[12], bytes[13], bytes[14], bytes[15])]
}
fn decode_kernarg(bytes: &[u8; 16]) -> (result: [u32; 4])
    ensures result@ == words_spec(bytes@),
{ gfx942_fill_decode_body_v1!(bytes) }

fn index_words(low: u32, high: u32, local: u32) -> (result: [u32; 2])
    ensures result@[0] == (low | local), result@[1] == high,
{ gfx942_fill_index_body_v1!(low, high, local) }

spec fn active(mask: u64, lane: int) -> bool {
    mask & (1u64 << lane) != 0
}
spec fn index_regs(regs: [u32; 4], group: u32) -> [u32; 4] {
    [(((group as u64) << 6) as u32) | regs[0],
     (((group as u64) << 6) >> 32) as u32, regs[2], regs[3]]
}
spec fn indexed(entry: Gfx942FillWaveStateV1, lane: int) -> [u32; 4] {
    if active(entry.exec_mask, lane) {
        index_regs(entry.vgprs[lane], entry.sgprs[2])
    } else { entry.vgprs[lane] }
}
spec fn global_index(entry: Gfx942FillWaveStateV1, lane: int) -> u64 {
    ((entry.sgprs[2] as u64) * 64 + entry.vgprs[lane][0] as u64) as u64
}
spec fn pointer(bytes: Seq<u8>) -> u64 {
    pair_spec(words_spec(bytes)[0], words_spec(bytes)[1])
}
spec fn count(bytes: Seq<u8>) -> u64 {
    pair_spec(words_spec(bytes)[2], words_spec(bytes)[3])
}
spec fn selected(entry: Gfx942FillWaveStateV1, bytes: Seq<u8>, lane: int) -> bool {
    active(entry.exec_mask, lane) && global_index(entry, lane) < count(bytes)
}
spec fn store_spec(entry: Gfx942FillWaveStateV1, bytes: Seq<u8>, lane: int)
    -> Option<Gfx942FillStoreV1>
{
    if selected(entry, bytes, lane) {
        Some(Gfx942FillStoreV1 {
            address: (pointer(bytes) + 4 * global_index(entry, lane)) as u64,
            value: global_index(entry, lane) as u32,
        })
    } else { None }
}
spec fn final_regs(entry: Gfx942FillWaveStateV1, bytes: Seq<u8>, lane: int) -> [u32; 4] {
    if selected(entry, bytes, lane) {
        let index = global_index(entry, lane);
        let address = (pointer(bytes) + 4 * index) as u64;
        [index as u32, (index >> 32) as u32, address as u32, (address >> 32) as u32]
    } else { indexed(entry, lane) }
}
spec fn valid(entry: Gfx942FillWaveStateV1, bytes: Seq<u8>) -> bool {
    bytes.len() == 16
    && pointer(bytes) as int + 4 * count(bytes) as int <= u64::MAX
    && forall|lane: int| 0 <= lane < 64 && active(entry.exec_mask, lane)
        ==> #[trigger] entry.vgprs[lane][0] < 64
}

proof fn index_arithmetic(group: u32, local: u32)
    requires local < 64,
    ensures
        pair_spec((((group as u64) << 6) as u32) | local,
            (((group as u64) << 6) >> 32) as u32) == (group as u64) * 64 + local as u64,
        ((((group as u64) << 6) as u32) | local) == ((group as u64) * 64 + local as u64) as u32,
        (((group as u64) << 6) >> 32) as u32 == ((((group as u64) * 64 + local as u64) as u64) >> 32) as u32,
        (group as u64) * 64 + (local as u64) < 0x40_0000_0000u64,
{
    assert(
        pair_spec((((group as u64) << 6) as u32) | local,
            (((group as u64) << 6) >> 32) as u32) == (group as u64) * 64 + local as u64
    ) by (bit_vector) requires local < 64;
    assert(
        ((((group as u64) << 6) as u32) | local) == ((group as u64) * 64 + local as u64) as u32
        && (((group as u64) << 6) >> 32) as u32 == ((((group as u64) * 64 + local as u64) as u64) >> 32) as u32
        && (group as u64) * 64 + (local as u64) < 0x40_0000_0000u64
    ) by (bit_vector) requires local < 64;
}

proof fn mask_insert(mask: u64, lane: usize)
    requires lane < 64,
    ensures
        active(mask | (1u64 << lane), lane as int),
        forall|other: int| 0 <= other < 64 && other != lane
            ==> active(mask | (1u64 << lane), other) == active(mask, other),
{
    assert(active(mask | (1u64 << lane), lane as int)) by (bit_vector)
        requires lane < 64;
    assert forall|other: int| 0 <= other < 64 && other != lane
        implies active(mask | (1u64 << lane), other) == active(mask, other) by {
        let n = other as u64;
        assert((mask | (1u64 << lane)) & (1u64 << n) == mask & (1u64 << n))
            by (bit_vector) requires lane < 64, n < 64, n != lane;
    }
}

proof fn mask_zero()
    ensures forall|lane: int| 0 <= lane < 64 ==> !active(0, lane),
{
    assert forall|lane: int| 0 <= lane < 64 implies !active(0, lane) by {
        let n = lane as u64;
        assert(0u64 & (1u64 << n) == 0u64) by (bit_vector);
    }
}

proof fn mask_subset(incoming: u64, comparison: u64)
    requires forall|lane: int| 0 <= lane < 64 && active(comparison, lane) ==> active(incoming, lane),
    ensures incoming & comparison == comparison,
{
    assert forall|n: u64| n < 64 && comparison & (1u64 << n) != 0 implies
        incoming & (1u64 << n) != 0 by {
        assert(active(comparison, n as int) ==> active(incoming, n as int));
    }
    assert(incoming & comparison == comparison) by (bit_vector)
        requires forall|n: u64| n < 64 ==>
            (comparison & (1u64 << n) != 0 ==> incoming & (1u64 << n) != 0);
}

proof fn pair_roundtrip(value: u64)
    ensures pair_spec(value as u32, (value >> 32) as u32) == value,
{
    assert(pair_spec(value as u32, (value >> 32) as u32) == value) by (bit_vector);
}

proof fn address_arithmetic(index: u64, base: u64, length: u64)
    requires index < length, base as int + 4 * length as int <= u64::MAX,
    ensures
        (index << 2).wrapping_add(base) == base as int + 4 * index as int,
        base as int + 4 * index as int + 4 <= base as int + 4 * length as int,
{
    assert((index << 2) == (index * 4) as u64) by (bit_vector);
}

fn execute_wave(mut state: Gfx942FillWaveStateV1, kernarg: &[u8; 16])
    -> (result: Gfx942FillWaveExecutionV1)
    requires valid(state, kernarg@),
    ensures
        result.kernarg_read_address == pair_spec(state.sgprs[0], state.sgprs[1]),
        result.terminal_pc == 0x44,
        result.retired_instructions == if result.state.exec_mask == 0 { 10u8 } else { 14u8 },
        result.state.scc == (result.state.exec_mask != 0),
        result.state.vcc == result.state.exec_mask,
        pair_spec(result.state.sgprs[0], result.state.sgprs[1]) == state.exec_mask,
        result.state.sgprs[2] == state.sgprs[2],
        result.state.sgprs[3] == 0,
        result.state.sgprs[4] == words_spec(kernarg@)[0],
        result.state.sgprs[5] == words_spec(kernarg@)[1],
        result.state.sgprs[6] == words_spec(kernarg@)[2],
        result.state.sgprs[7] == words_spec(kernarg@)[3],
        forall|lane: int| 0 <= lane < 64 ==>
            active(result.state.exec_mask, lane) == selected(state, kernarg@, lane),
        forall|lane: int| 0 <= lane < 64 ==>
            #[trigger] result.stores[lane] == store_spec(state, kernarg@, lane),
        forall|lane: int| 0 <= lane < 64 ==>
            #[trigger] result.state.vgprs[lane] == final_regs(state, kernarg@, lane),
{
    let ghost entry = state;
    proof { mask_zero(); }
    gfx942_fill_wave_body_v1!(verus_exec_expr, state, kernarg, lane, stores, incoming, words, length,
    [
        invariant
            lane <= 64, incoming == entry.exec_mask, valid(entry, kernarg@),
            state.sgprs[2] == entry.sgprs[2], state.sgprs[3] == 0,
            state.sgprs[0] == ((entry.sgprs[2] as u64) << 6) as u32,
            state.sgprs[1] == (((entry.sgprs[2] as u64) << 6) >> 32) as u32,
            forall|j: int| 0 <= j < 64 ==> #[trigger] state.vgprs[j]
                == if j < lane { indexed(entry, j) } else { entry.vgprs[j] },
        decreases 64 - lane,
    ], [
        invariant
            lane <= 64, incoming == entry.exec_mask, valid(entry, kernarg@),
            words@ == words_spec(kernarg@),
            length == count(kernarg@),
            forall|j: int| 0 <= j < 64 ==> #[trigger] state.vgprs[j] == indexed(entry, j),
            forall|j: int| 0 <= j < 64 ==> active(state.vcc, j)
                == (j < lane && selected(entry, kernarg@, j)),
        decreases 64 - lane,
    ], [
        invariant
            lane <= 64, incoming == entry.exec_mask, valid(entry, kernarg@),
            words@ == words_spec(kernarg@),
            forall|j: int| 0 <= j < 64 ==> active(state.exec_mask, j) == selected(entry, kernarg@, j),
            forall|j: int| 0 <= j < 64 ==> #[trigger] state.vgprs[j]
                == if j < lane { final_regs(entry, kernarg@, j) } else { indexed(entry, j) },
            forall|j: int| 0 <= j < 64 ==> #[trigger] stores[j]
                == if j < lane { store_spec(entry, kernarg@, j) } else { None },
        decreases 64 - lane,
    ], [
        proof {
            assert forall|j: int| 0 <= j < 64 && active(entry.exec_mask, j)
                implies pair_spec(state.vgprs[j][0], state.vgprs[j][1]) == global_index(entry, j) by {
                index_arithmetic(entry.sgprs[2], entry.vgprs[j][0]);
            }
        }
    ], [
        let ghost comparison_before = state.vcc;
        proof {
            mask_insert(state.vcc, lane);
            if active(entry.exec_mask, lane as int) {
                index_arithmetic(entry.sgprs[2], entry.vgprs[lane as int][0]);
            }
            assert((incoming & (1u64 << lane) != 0
                && pair_spec(state.vgprs[lane as int][0], state.vgprs[lane as int][1]) < count(kernarg@))
                == selected(entry, kernarg@, lane as int));
        }
    ], [
        proof {
            mask_subset(incoming, state.vcc);
            pair_roundtrip(incoming);
        }
    ], [
        proof {
            if active(state.exec_mask, lane as int) {
                index_arithmetic(entry.sgprs[2], entry.vgprs[lane as int][0]);
                address_arithmetic(global_index(entry, lane as int), pointer(kernarg@), count(kernarg@));
            }
        }
    ], [
        proof {
            assert(state.vcc == if selected(entry, kernarg@, lane as int) {
                comparison_before | (1u64 << lane)
            } else { comparison_before });
            assert forall|j: int| 0 <= j < 64 implies active(state.vcc, j)
                == (j <= lane && selected(entry, kernarg@, j)) by {
                if j != lane {
                    assert(active(state.vcc, j) == active(comparison_before, j));
                    assert(active(comparison_before, j) == (j < lane && selected(entry, kernarg@, j)));
                } else if selected(entry, kernarg@, j) {
                    assert(active(state.vcc, j));
                } else {
                    assert(!active(comparison_before, j));
                }
            }
        }
    ], [
        proof {
            if state.exec_mask == 0 {
                assert forall|j: int| 0 <= j < 64 implies !selected(entry, kernarg@, j) by {
                    assert(!active(0, j));
                }
            }
        }
    ])
}

spec fn covers(store: Option<Gfx942FillStoreV1>, address: u64) -> bool {
    match store {
        Some(s) => address >= s.address && address - s.address < 4,
        None => false,
    }
}
spec fn observe(stores: Seq<Option<Gfx942FillStoreV1>>, address: u64, before: u8, n: nat) -> u8
    decreases n,
{
    if n == 0 { before } else {
        match stores[n - 1] {
            Some(s) => if covers(stores[n - 1], address) {
                (s.value >> ((8 * (address - s.address)) as u64)) as u8
            } else { observe(stores, address, before, (n - 1) as nat) },
            None => observe(stores, address, before, (n - 1) as nat),
        }
    }
}

impl Gfx942FillWaveExecutionV1 {
    fn byte_after(&self, address: u64, before: u8) -> (result: u8)
        ensures
            result == observe(self.stores@, address, before, 64),
            (forall|lane: int| 0 <= lane < 64 ==> !covers(self.stores[lane], address))
                ==> result == before,
    {
        gfx942_fill_byte_body_v1!(verus_exec_expr, self, address, before, lane, byte, [
            invariant
                lane <= 64,
                byte == observe(self.stores@, address, before, lane as nat),
                (forall|j: int| 0 <= j < lane ==> !covers(self.stores[j], address)) ==> byte == before,
            decreases 64 - lane,
        ])
    }
}

proof fn distinct_store_intervals(group: u32, left: u32, right: u32, base: u64, length: u64)
    requires
        left < 64, right < 64, left != right,
        (group as int) * 64 + (left as int) < length,
        (group as int) * 64 + (right as int) < length,
        base as int + length as int * 4 <= u64::MAX,
    ensures
        base as int + 4 * (group as int * 64 + left as int) + 4
            <= base as int + 4 * (group as int * 64 + right as int)
        || base as int + 4 * (group as int * 64 + right as int) + 4
            <= base as int + 4 * (group as int * 64 + left as int),
{}
}
