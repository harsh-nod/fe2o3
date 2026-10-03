macro_rules! gfx942_s_mov_b32_execute_body_v1 {
    ($instruction:ident, $state:ident) => {{
        let value = match $instruction.source {
            Gfx942U32SourceV1::Sgpr(index) => $state.registers[index as usize],
            Gfx942U32SourceV1::Constant(value) => value,
        };
        $state.registers[$instruction.destination as usize] = value;
        value
    }};
}
