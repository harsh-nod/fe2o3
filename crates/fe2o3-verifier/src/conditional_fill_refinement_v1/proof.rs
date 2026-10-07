proof fn generated_store_relation(input: DispatchInput, group: u32, lane: int)
    requires artifact_dispatch_valid(input), group < input.grid[0] / 64, 0 <= lane < 64,
    ensures
        semantic_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64)
            == group_store(input, group, lane),
        neutral_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64)
            == group_store(input, group, lane),
        target_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64)
            == group_store(input, group, lane),
{}

fn execute_refining_group(input: &DispatchInput, group: u32, seed: Gfx942FillWaveStateV1)
    -> (result: Gfx942FillWaveExecutionV1)
    requires artifact_dispatch_valid(*input), group < input.grid[0] / 64,
    ensures
        result.terminal_pc == 0x44,
        result.kernarg_read_address == input.kernarg_address,
        forall|lane: int| 0 <= lane < 64 ==> #[trigger] result.stores[lane]
            == semantic_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64),
        forall|lane: int| 0 <= lane < 64 ==> #[trigger] result.stores[lane]
            == neutral_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64),
        forall|lane: int| 0 <= lane < 64 ==> #[trigger] result.stores[lane]
            == target_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64),
{
    let result = execute_group(input, group, seed).unwrap();
    proof {
        assert forall|lane: int| 0 <= lane < 64 implies
            result.stores[lane] == semantic_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64)
            && result.stores[lane] == neutral_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64)
            && result.stores[lane] == target_store(input.output_base, count(input.kernarg@), group_index(group, lane) as u64)
        by { generated_store_relation(*input, group, lane); }
    }
    result
}

proof fn generated_coverage(input: DispatchInput, index: int)
    requires artifact_dispatch_valid(input), 0 <= index < count(input.kernarg@),
    ensures
        0 <= index / 64 < input.grid[0] / 64,
        semantic_store(input.output_base, count(input.kernarg@), index as u64)
            == Some(Gfx942FillStoreV1 { address: (input.output_base as int + 4 * index) as u64, value: index as u32 }),
        neutral_store(input.output_base, count(input.kernarg@), index as u64)
            == semantic_store(input.output_base, count(input.kernarg@), index as u64),
        target_store(input.output_base, count(input.kernarg@), index as u64)
            == semantic_store(input.output_base, count(input.kernarg@), index as u64),
{
    exact_coordinate(input, index);
    generated_store_relation(input, (index / 64) as u32, index % 64);
}

proof fn generated_byte_relation(input: DispatchInput, address: u64, before: u8)
    requires artifact_dispatch_valid(input),
    ensures
        semantic_byte(input, address, before) == output_byte(input, address, before),
        neutral_byte(input, address, before) == output_byte(input, address, before),
        target_byte(input, address, before) == output_byte(input, address, before),
{
    if inside_output(input, address) {
        let offset = address as int - input.output_base as int;
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(offset, 4);
        vstd::arithmetic::div_mod::lemma_mod_pos_bound(offset, 4);
        generated_coverage(input, offset / 4);
    }
}

fn execute_refining_byte(input: &DispatchInput, address: u64, before: u8) -> (result: u8)
    requires artifact_dispatch_valid(*input),
    ensures
        result == semantic_byte(*input, address, before),
        result == neutral_byte(*input, address, before),
        result == target_byte(*input, address, before),
        !inside_output(*input, address) ==> result == before,
{
    let result = dispatch_byte_after(input, address, before);
    proof { generated_byte_relation(*input, address, before); }
    result
}
