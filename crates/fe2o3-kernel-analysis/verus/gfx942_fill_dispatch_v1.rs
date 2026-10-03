// Complete projected dispatch; includes the actual existing wave proof unchanged.
include!("gfx942_fill_wave_v1.rs");
include!("../src/gfx942_fill_wave_v1/dispatch_body.rs");

verus! {
#[derive(Clone, Copy)]
pub struct DispatchInput {
    pub kernarg: [u8; 16],
    pub kernarg_address: u64,
    pub output_base: u64,
    pub output_bytes: u64,
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
}

spec fn dispatch_valid(input: DispatchInput) -> bool {
    input.grid[0] > 0 && input.grid[0] % 64 == 0
    && input.grid[1] == 1 && input.grid[2] == 1
    && input.workgroup[0] == 64 && input.workgroup[1] == 1 && input.workgroup[2] == 1
    && count(input.kernarg@) <= input.grid[0]
    && input.kernarg_address % 8 == 0
    && input.kernarg_address as int + 16 <= u64::MAX
    && pointer(input.kernarg@) == input.output_base
    && input.output_base % 4 == 0
    && input.output_bytes == 4 * count(input.kernarg@) as int
    && input.output_base as int + input.output_bytes as int <= u64::MAX
    && (input.output_bytes == 0
        || input.output_base as int >= input.kernarg_address as int + 16
        || input.kernarg_address as int >= input.output_base as int + input.output_bytes as int)
}

fn valid_dispatch(input: &DispatchInput) -> (result: bool)
    ensures result == dispatch_valid(*input),
{ gfx942_fill_dispatch_valid_body_v1!(input) }

spec fn entry_matches(entry: Gfx942FillWaveStateV1, address: u64, group: u32) -> bool {
    pair_spec(entry.sgprs[0], entry.sgprs[1]) == address
    && entry.sgprs[2] == group && entry.exec_mask == u64::MAX
    && forall|lane: int| 0 <= lane < 64 ==> #[trigger] entry.vgprs[lane][0] == lane
}

fn initialize_entry(mut state: Gfx942FillWaveStateV1, address: u64, group: u32)
    -> (result: Gfx942FillWaveStateV1)
    ensures
        entry_matches(result, address, group),
        forall|j: int| 3 <= j < 8 ==> #[trigger] result.sgprs[j] == state.sgprs[j],
        forall|j: int, k: int| 0 <= j < 64 && 1 <= k < 4 ==>
            #[trigger] result.vgprs[j][k] == state.vgprs[j][k],
        result.vcc == state.vcc, result.scc == state.scc,
{
    let ghost seed = state;
    proof { pair_roundtrip(address); }
    gfx942_fill_entry_body_v1!(verus_exec_expr, state, address, group, lane, [
        invariant
            lane <= 64,
            pair_spec(state.sgprs[0], state.sgprs[1]) == address,
            state.sgprs[2] == group, state.exec_mask == u64::MAX,
            state.vcc == seed.vcc, state.scc == seed.scc,
            forall|j: int| 3 <= j < 8 ==> #[trigger] state.sgprs[j] == seed.sgprs[j],
            forall|j: int| 0 <= j < 64 ==> #[trigger] state.vgprs[j]
                == [if j < lane { j as u32 } else { seed.vgprs[j][0] },
                    seed.vgprs[j][1], seed.vgprs[j][2], seed.vgprs[j][3]],
        decreases 64 - lane,
    ])
}

spec fn group_index(group: u32, lane: int) -> int { group as int * 64 + lane }
spec fn group_store(input: DispatchInput, group: u32, lane: int) -> Option<Gfx942FillStoreV1> {
    if group_index(group, lane) < count(input.kernarg@) {
        Some(Gfx942FillStoreV1 {
            address: (input.output_base as int + 4 * group_index(group, lane)) as u64,
            value: group_index(group, lane) as u32,
        })
    } else { None }
}

proof fn full_lane(lane: int)
    requires 0 <= lane < 64,
    ensures active(u64::MAX, lane),
{
    let n = lane as u64;
    assert(u64::MAX & (1u64 << n) != 0) by (bit_vector) requires n < 64;
}

proof fn dispatch_entry(input: DispatchInput, group: u32, entry: Gfx942FillWaveStateV1)
    requires dispatch_valid(input), group < input.grid[0] / 64,
        entry_matches(entry, input.kernarg_address, group),
    ensures
        valid(entry, input.kernarg@),
        forall|lane: int| 0 <= lane < 64 ==>
            global_index(entry, lane) == group_index(group, lane),
        forall|lane: int| 0 <= lane < 64 ==>
            store_spec(entry, input.kernarg@, lane) == group_store(input, group, lane),
{
    assert forall|lane: int| 0 <= lane < 64 implies
        global_index(entry, lane) == group_index(group, lane)
        && store_spec(entry, input.kernarg@, lane) == group_store(input, group, lane) by {
        full_lane(lane);
    }
}

fn execute_group(input: &DispatchInput, group: u32, seed: Gfx942FillWaveStateV1)
    -> (result: Option<Gfx942FillWaveExecutionV1>)
    requires dispatch_valid(*input),
    ensures
        result.is_some() == (group < input.grid[0] / 64),
        result.is_some() ==> {
            let execution = result.unwrap();
            execution.terminal_pc == 0x44
            && execution.kernarg_read_address == input.kernarg_address
            && execution.retired_instructions == if execution.state.exec_mask == 0 { 10u8 } else { 14u8 }
            && forall|lane: int| 0 <= lane < 64 ==>
                #[trigger] execution.stores[lane] == group_store(*input, group, lane)
        },
{
    gfx942_fill_group_body_v1!(verus_exec_expr, input, group, seed, entry, [
        proof { dispatch_entry(*input, group, entry); }
    ])
}

proof fn exact_coordinate(input: DispatchInput, index: int)
    requires dispatch_valid(input), 0 <= index < count(input.kernarg@),
    ensures
        0 <= index / 64 < input.grid[0] / 64,
        0 <= index % 64 < 64,
        group_index((index / 64) as u32, index % 64) == index,
        group_store(input, (index / 64) as u32, index % 64)
            == Some(Gfx942FillStoreV1 { address: (input.output_base as int + 4 * index) as u64, value: index as u32 }),
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(index, 64);
    vstd::arithmetic::div_mod::lemma_mod_pos_bound(index, 64);
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(input.grid[0] as int, 64);
}

proof fn unique_coordinates(group_a: u32, lane_a: int, group_b: u32, lane_b: int)
    requires 0 <= lane_a < 64, 0 <= lane_b < 64,
        group_index(group_a, lane_a) == group_index(group_b, lane_b),
    ensures group_a == group_b, lane_a == lane_b,
{
    let i = group_index(group_a, lane_a);
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(i, 64, group_a as int, lane_a);
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(i, 64, group_b as int, lane_b);
}

proof fn distinct_dispatch_intervals(input: DispatchInput, ga: u32, la: int, gb: u32, lb: int)
    requires dispatch_valid(input), 0 <= la < 64, 0 <= lb < 64,
        group_index(ga, la) < count(input.kernarg@),
        group_index(gb, lb) < count(input.kernarg@), ga != gb || la != lb,
    ensures
        input.output_base as int + 4 * group_index(ga, la) + 4
            <= input.output_base as int + 4 * group_index(gb, lb)
        || input.output_base as int + 4 * group_index(gb, lb) + 4
            <= input.output_base as int + 4 * group_index(ga, la),
{
    if group_index(ga, la) == group_index(gb, lb) { unique_coordinates(ga, la, gb, lb); }
}

spec fn inside_output(input: DispatchInput, address: u64) -> bool {
    input.output_base <= address && (address as int) < input.output_base as int + input.output_bytes as int
}
spec fn output_byte(input: DispatchInput, address: u64, before: u8) -> u8 {
    if inside_output(input, address) {
        let offset = address as int - input.output_base as int;
        ((offset / 4) as u32 >> ((offset % 4 * 8) as u64)) as u8
    } else { before }
}

proof fn covering_coordinates(input: DispatchInput, group: u32, lane: int, address: u64)
    requires dispatch_valid(input), 0 <= lane < 64,
    ensures covers(group_store(input, group, lane), address) ==
        (inside_output(input, address)
            && group as int == ((address as int - input.output_base as int) / 4) / 64
            && lane == ((address as int - input.output_base as int) / 4) % 64),
{
    let delta = address as int - input.output_base as int;
    let index = group_index(group, lane);
    if covers(group_store(input, group, lane), address) {
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(delta, 4, index, delta - 4 * index);
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(index, 64, group as int, lane);
    }
    if inside_output(input, address) {
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(delta, 4);
        vstd::arithmetic::div_mod::lemma_mod_pos_bound(delta, 4);
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(delta / 4, 64);
    }
}

proof fn observe_unique(stores: Seq<Option<Gfx942FillStoreV1>>, address: u64, before: u8, n: nat, lane: int)
    requires n <= stores.len(), 0 <= lane < n,
        covers(stores[lane], address),
        forall|j: int| 0 <= j < n && j != lane ==> !covers(#[trigger] stores[j], address),
    ensures observe(stores, address, before, n)
        == (stores[lane].unwrap().value >> ((8 * (address - stores[lane].unwrap().address)) as u64)) as u8,
    decreases n,
{
    if n - 1 != lane {
        observe_unique(stores, address, before, (n - 1) as nat, lane);
    }
}

fn dispatch_byte_after(input: &DispatchInput, address: u64, before: u8) -> (result: u8)
    requires dispatch_valid(*input),
    ensures result == output_byte(*input, address, before),
{
    gfx942_fill_dispatch_byte_body_v1!(verus_exec_expr, input, address, before,
        index, group, entry, execution, [
        proof {
            let delta = address as int - input.output_base as int;
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(delta, 4);
            vstd::arithmetic::div_mod::lemma_mod_pos_bound(delta, 4);
            exact_coordinate(*input, index as int);
            dispatch_entry(*input, group, entry);
        }
    ], [
        proof {
            let lane = index as int % 64;
            assert forall|j: int| 0 <= j < 64 implies
                covers(#[trigger] execution.stores[j], address) == (j == lane) by {
                covering_coordinates(*input, group, j, address);
            }
            observe_unique(execution.stores@, address, before, 64, lane);
        }
    ])
}
}
