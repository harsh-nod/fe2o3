// Shared executable bodies; annotation slots contain proof-only loop invariants.
macro_rules! gfx942_fill_pair_body_v1 {
    ($low:expr, $high:expr) => {
        ($low as u64) | (($high as u64) << 32)
    };
}

macro_rules! gfx942_fill_decode_body_v1 {
    ($bytes:ident) => {{
        [
            word($bytes[0], $bytes[1], $bytes[2], $bytes[3]),
            word($bytes[4], $bytes[5], $bytes[6], $bytes[7]),
            word($bytes[8], $bytes[9], $bytes[10], $bytes[11]),
            word($bytes[12], $bytes[13], $bytes[14], $bytes[15]),
        ]
    }};
}

macro_rules! gfx942_fill_word_body_v1 {
    ($a:ident, $b:ident, $c:ident, $d:ident) => {
        ($a as u32) | (($b as u32) << 8) | (($c as u32) << 16) | (($d as u32) << 24)
    };
}

macro_rules! gfx942_fill_index_body_v1 {
    ($low:ident, $high:ident, $local:ident) => {{ [($low | $local), $high] }};
}

macro_rules! gfx942_fill_wave_body_v1 {
    ($syntax:ident, $state:ident, $kernarg:ident, $lane:ident, $stores:ident,
     $incoming:ident, $words:ident, $length:ident,
     [$($index_annotations:tt)*], [$($compare_annotations:tt)*],
     [$($store_annotations:tt)*], [$($after_index:tt)*],
     [$($compare_step:tt)*], [$($after_compare:tt)*], [$($store_step:tt)*],
     [$($compare_end:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            // S_LOAD captures its base and bytes before S_LSHL overwrites s0:s1.
            let read_address = pair($state.sgprs[0], $state.sgprs[1]);
            let pending_load = decode_kernarg($kernarg);
            let $incoming = $state.exec_mask;
            $state.sgprs[3] = 0;
            let shifted = ($state.sgprs[2] as u64) << 6;
            $state.sgprs[0] = shifted as u32;
            $state.sgprs[1] = (shifted >> 32) as u32;
            let mut $lane = 0usize;
            while $lane < 64
                $($index_annotations)*
            {
                if $incoming & (1u64 << $lane) != 0 {
                    let mut registers = $state.vgprs[$lane];
                    let index = index_words($state.sgprs[0], $state.sgprs[1], registers[0]);
                    registers[0] = index[0];
                    registers[1] = index[1];
                    $state.vgprs[$lane] = registers;
                }
                $lane += 1;
            }
            $($after_index)*
            // WAIT lgkmcnt(0): materialize the captured read, not the new s0:s1.
            let $words = pending_load;
            $state.sgprs[4] = $words[0];
            $state.sgprs[5] = $words[1];
            $state.sgprs[6] = $words[2];
            $state.sgprs[7] = $words[3];
            let $length = pair($words[2], $words[3]);
            $state.vcc = 0;
            $lane = 0;
            while $lane < 64
                $($compare_annotations)*
            {
                $($compare_step)*
                let bit = 1u64 << $lane;
                let index = pair($state.vgprs[$lane][0], $state.vgprs[$lane][1]);
                if $incoming & bit != 0 && $length > index {
                    $state.vcc |= bit;
                }
                $($compare_end)*
                $lane += 1;
            }
            $($after_compare)*
            // AND_SAVEEXEC reads old EXEC before replacing the aliased s0:s1.
            $state.sgprs[0] = $incoming as u32;
            $state.sgprs[1] = ($incoming >> 32) as u32;
            $state.exec_mask = $incoming & $state.vcc;
            $state.scc = $state.exec_mask != 0;
            let mut $stores = [None; 64];
            let retired_instructions;
            // CBRANCH_EXECZ targets END directly; no VGPR stores on this path.
            if $state.exec_mask == 0 {
                retired_instructions = 10;
            } else {
                retired_instructions = 14;
                $lane = 0;
                while $lane < 64
                    $($store_annotations)*
                {
                    $($store_step)*
                    if $state.exec_mask & (1u64 << $lane) != 0 {
                        let mut registers = $state.vgprs[$lane];
                        registers[2] = $words[0];
                        registers[3] = $words[1];
                        let index = pair(registers[0], registers[1]);
                        let base = pair(registers[2], registers[3]);
                        let address = (index << 2).wrapping_add(base);
                        registers[2] = address as u32;
                        registers[3] = (address >> 32) as u32;
                        $state.vgprs[$lane] = registers;
                        $stores[$lane] = Some(Gfx942FillStoreV1 {
                            address,
                            value: registers[0],
                        });
                    }
                    $lane += 1;
                }
            }
            $($finish)*
            // END implicitly drains this wave's operations in the projection.
            // It does not establish host visibility or AQL completion.
            Gfx942FillWaveExecutionV1 {
                state: $state,
                kernarg_read_address: read_address,
                stores: $stores,
                retired_instructions,
                terminal_pc: 0x44,
            }
        })
    };
}

macro_rules! gfx942_fill_byte_body_v1 {
    ($syntax:ident, $execution:ident, $address:ident, $before:ident,
     $lane:ident, $byte:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $byte = $before;
            let mut $lane = 0usize;
            while $lane < 64
                $($annotations)*
            {
                if let Some(store) = $execution.stores[$lane] {
                    if $address >= store.address && $address - store.address < 4 {
                        $byte = (store.value >> (8 * ($address - store.address))) as u8;
                    }
                }
                $lane += 1;
            }
            $byte
        })
    };
}
