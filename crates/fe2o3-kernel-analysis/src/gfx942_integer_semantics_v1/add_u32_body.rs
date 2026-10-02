macro_rules! gfx942_add_u32_body_v1 {
    ($a:expr, $b:expr) => {{
        let sum = $a as u64 + $b as u64;
        Gfx942U32AddResultV1 {
            value: (sum % 0x1_0000_0000_u64) as u32,
            scc: sum >= 0x1_0000_0000_u64,
        }
    }};
}
