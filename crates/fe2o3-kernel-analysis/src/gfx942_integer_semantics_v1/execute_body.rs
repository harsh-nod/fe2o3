macro_rules! gfx942_s_add_u32_execute_body_v1 {
    ($instruction:ident, $state:ident) => {{
        let left = match $instruction.sources[0] {
            Gfx942U32SourceV1::Sgpr(index) => $state.registers[index as usize],
            Gfx942U32SourceV1::Constant(value) => value,
        };
        let right = match $instruction.sources[1] {
            Gfx942U32SourceV1::Sgpr(index) => $state.registers[index as usize],
            Gfx942U32SourceV1::Constant(value) => value,
        };
        let result = gfx942_add_u32_body_v1!(left, right);
        $state.registers[$instruction.destination as usize] = result.value;
        $state.scc = result.scc;
        result
    }};
}
