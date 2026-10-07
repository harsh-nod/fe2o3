// Shared dispatch validation, ABI entry construction, and actual wave composition.
macro_rules! gfx942_fill_dispatch_valid_body_v1 {
    ($input:ident) => {{
        let words = decode_kernarg(&$input.kernarg);
        let count = pair(words[2], words[3]);
        $input.grid[0] > 0
            && $input.grid[0] % 64 == 0
            && $input.grid[1] == 1
            && $input.grid[2] == 1
            && $input.workgroup[0] == 64
            && $input.workgroup[1] == 1
            && $input.workgroup[2] == 1
            && count <= $input.grid[0] as u64
            && ($input.kernarg_bytes == 16 || $input.kernarg_bytes == 272)
            && $input.kernarg_address % 8 == 0
            && $input.kernarg_address <= u64::MAX - $input.kernarg_bytes
            && pair(words[0], words[1]) == $input.output_base
            && $input.output_base % 4 == 0
            && $input.output_bytes == count * 4
            && $input.output_base <= u64::MAX - $input.output_bytes
            && ($input.output_bytes == 0
                || $input.output_base >= $input.kernarg_address + $input.kernarg_bytes
                || $input.kernarg_address >= $input.output_base + $input.output_bytes)
    }};
}

macro_rules! gfx942_fill_entry_body_v1 {
    ($syntax:ident, $state:ident, $address:ident, $group:ident, $lane:ident,
     [$($annotations:tt)*]) => {
        $syntax!({
            $state.sgprs[0] = $address as u32;
            $state.sgprs[1] = ($address >> 32) as u32;
            $state.sgprs[2] = $group;
            $state.exec_mask = u64::MAX;
            let mut $lane = 0usize;
            while $lane < 64
                $($annotations)*
            {
                let mut registers = $state.vgprs[$lane];
                registers[0] = $lane as u32;
                $state.vgprs[$lane] = registers;
                $lane += 1;
            }
            $state
        })
    };
}

macro_rules! gfx942_fill_group_body_v1 {
    ($syntax:ident, $input:ident, $group:ident, $seed:ident, $entry:ident, [$($after_entry:tt)*]) => {
        $syntax!({
            if $group >= $input.grid[0] / 64 {
                None
            } else {
                let $entry = initialize_entry($seed, $input.kernarg_address, $group);
                $($after_entry)*
                Some(execute_wave($entry, &$input.kernarg))
            }
        })
    };
}

macro_rules! gfx942_fill_dispatch_byte_body_v1 {
    ($syntax:ident, $input:ident, $address:ident, $before:ident,
     $index:ident, $group:ident, $entry:ident, $execution:ident,
     [$($before_wave:tt)*], [$($after_wave:tt)*]) => {
        $syntax!({
            if $address < $input.output_base
                || $address - $input.output_base >= $input.output_bytes
            {
                $before
            } else {
                let $index = ($address - $input.output_base) / 4;
                let $group = ($index / 64) as u32;
                let seed = Gfx942FillWaveStateV1 {
                    sgprs: [0; 8], vgprs: [[0; 4]; 64], exec_mask: 0, vcc: 0, scc: false,
                };
                let $entry = initialize_entry(seed, $input.kernarg_address, $group);
                $($before_wave)*
                let $execution = execute_wave($entry, &$input.kernarg);
                $($after_wave)*
                $execution.byte_after($address, $before)
            }
        })
    };
}
