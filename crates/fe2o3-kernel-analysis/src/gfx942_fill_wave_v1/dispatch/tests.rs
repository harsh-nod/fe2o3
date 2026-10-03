use super::*;

const HSACO: &[u8] = include_bytes!(
    "../../../../fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco"
);

fn input(count: u64, grid: u32) -> DispatchInput {
    let base = 0x1_0000_2000u64;
    let mut kernarg = [0; 16];
    kernarg[..8].copy_from_slice(&base.to_le_bytes());
    kernarg[8..].copy_from_slice(&count.to_le_bytes());
    DispatchInput {
        kernarg,
        kernarg_address: 0x10_0000_1000,
        kernarg_bytes: 16,
        output_base: base,
        output_bytes: count.checked_mul(4).unwrap_or(u64::MAX),
        grid: [grid, 1, 1],
        workgroup: [64, 1, 1],
    }
}

fn seed() -> Gfx942FillWaveStateV1 {
    Gfx942FillWaveStateV1 {
        sgprs: [0xdead_beef; 8],
        vgprs: [[0xaaaa; 4]; 64],
        exec_mask: 1,
        vcc: 0x1234,
        scc: true,
    }
}

#[test]
fn descriptor_storage_extent_is_closed_before_arithmetic() {
    let mut arguments = input(1, 64);
    for bytes in [16, 272] {
        arguments.kernarg_bytes = bytes;
        assert!(valid_dispatch(&arguments));
    }
    for bytes in [0, 8, 24, 32, 256, 264, 280, u64::MAX] {
        arguments.kernarg_bytes = bytes;
        assert!(!valid_dispatch(&arguments));
    }
}

fn bind<'a, 'b>(
    kernel: &'a Gfx942FillKernelV1<'b>,
    input: DispatchInput,
) -> Result<Gfx942FillDispatchV1<'a, 'b>, Gfx942FillDispatchErrorV1> {
    kernel.check_dispatch(
        input.kernarg,
        input.kernarg_address,
        input.output_base,
        input.output_bytes,
        input.grid,
        input.workgroup,
    )
}

#[test]
fn entry_initialization_changes_only_descriptor_selected_inputs() {
    let seed = seed();
    for group in [0, 1, 67108862] {
        let entry = initialize_entry(seed, 0x2_0000_1000, group);
        assert_eq!(pair(entry.sgprs[0], entry.sgprs[1]), 0x2_0000_1000);
        assert_eq!(entry.sgprs[2], group);
        assert_eq!(entry.sgprs[3..], seed.sgprs[3..]);
        assert_eq!(
            (entry.exec_mask, entry.vcc, entry.scc),
            (u64::MAX, seed.vcc, seed.scc)
        );
        for lane in 0..64 {
            assert_eq!(entry.vgprs[lane][0], lane as u32);
            assert_eq!(entry.vgprs[lane][1..], seed.vgprs[lane][1..]);
        }
    }
}

#[test]
fn all_output_stores_and_bytes_match_including_empty_partial_tail_and_extra_groups() {
    let kernel = Gfx942FillKernelV1::inspect(HSACO, 0).unwrap();
    for count in [0u32, 1, 63, 64, 65, 4097] {
        let input = input(count as u64, count.max(1).div_ceil(64) * 64 + 128);
        let dispatch = bind(&kernel, input).unwrap();
        assert!(std::ptr::eq(dispatch.kernel(), &kernel));
        assert_eq!(dispatch.kernarg(), &input.kernarg);
        let mut seen = vec![false; count as usize];
        for group in 0..dispatch.workgroup_count() {
            let execution = dispatch.execute_group(group, seed()).unwrap();
            assert_eq!(execution.terminal_pc, 0x44);
            assert_eq!(execution.kernarg_read_address, input.kernarg_address);
            assert_eq!(
                execution.retired_instructions,
                if group * 64 >= count { 10 } else { 14 }
            );
            for (lane, store) in execution.stores.iter().enumerate() {
                let index = group as usize * 64 + lane;
                if index < count as usize {
                    let store = store.unwrap();
                    assert_eq!(store.address, input.output_base + index as u64 * 4);
                    assert_eq!(store.value, index as u32);
                    assert!(!std::mem::replace(&mut seen[index], true));
                } else {
                    assert!(store.is_none());
                }
            }
        }
        assert!(seen.into_iter().all(|written| written));
        for index in 0..count {
            for (offset, byte) in index.to_le_bytes().into_iter().enumerate() {
                let address = input.output_base + index as u64 * 4 + offset as u64;
                assert_eq!(dispatch.byte_after(address, !byte), byte);
            }
        }
        for address in [
            0,
            input.output_base - 1,
            input.output_base + input.output_bytes,
            u64::MAX,
        ] {
            assert_eq!(dispatch.byte_after(address, 0xa5), 0xa5);
        }
        assert_eq!(
            dispatch
                .execute_group(dispatch.workgroup_count(), seed())
                .unwrap_err(),
            Gfx942FillDispatchErrorV1::Group
        );
        assert_eq!(
            dispatch.execute_group(u32::MAX, seed()).unwrap_err(),
            Gfx942FillDispatchErrorV1::Group
        );
    }
}

#[test]
fn largest_full_grid_is_constant_storage_and_never_rounded() {
    let kernel = Gfx942FillKernelV1::inspect(HSACO, 0).unwrap();
    let grid = u32::MAX - 63;
    let arguments = input(grid as u64, grid);
    let dispatch = bind(&kernel, arguments).unwrap();
    let last = dispatch
        .execute_group(dispatch.workgroup_count() - 1, seed())
        .unwrap();
    let store = last.stores[63].unwrap();
    assert_eq!(store.value, grid - 1);
    assert_eq!(store.address, arguments.output_base + 4 * (grid as u64 - 1));
    for (offset, byte) in (grid - 1).to_le_bytes().into_iter().enumerate() {
        assert_eq!(
            dispatch.byte_after(store.address + offset as u64, !byte),
            byte
        );
    }
    assert!(bind(&kernel, input(u32::MAX as u64, u32::MAX)).is_err());
    assert!(bind(&kernel, input(u32::MAX as u64, grid)).is_err());
    assert!(bind(&kernel, input(u64::MAX, grid)).is_err());
}

#[test]
fn invalid_geometry_regions_and_substituted_kernarg_fields_reject() {
    let kernel = Gfx942FillKernelV1::inspect(HSACO, 0).unwrap();
    for case in 0..19 {
        let mut input = input(65, 128);
        match case {
            0 => input.grid[0] = 0,
            1 => input.grid[0] = 65,
            2 => input.grid[0] = 64,
            3 => input.grid[1] = 2,
            4 => input.grid[2] = 0,
            5 => input.workgroup[0] = 32,
            6 => input.workgroup[1] = 2,
            7 => input.workgroup[2] = 0,
            8 => input.kernarg_address += 1,
            9 => input.kernarg_address = u64::MAX - 15,
            10 => input.output_base += 4,
            11 => {
                input.output_base += 1;
                input.kernarg[..8].copy_from_slice(&input.output_base.to_le_bytes());
            }
            12 => input.output_bytes -= 4,
            13 => input.kernarg[8..].copy_from_slice(&64u64.to_le_bytes()),
            14 => {
                input.output_base = u64::MAX - 3;
                input.kernarg[..8].copy_from_slice(&input.output_base.to_le_bytes());
            }
            15 => input.kernarg_address = input.output_base,
            16 => input.kernarg_address = input.output_base - 8,
            17 => input.kernarg_address = input.output_base + 256,
            18 => {
                input.kernarg[8..].fill(0);
                input.output_bytes = 4;
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(bind(&kernel, input), Err(Gfx942FillDispatchErrorV1::Input)),
            "case{case}"
        );
    }
    for address in [0x1_0000_2000 - 16, 0x1_0000_2000 + 264] {
        let mut input = input(65, 128);
        input.kernarg_address = address;
        assert!(bind(&kernel, input).is_ok(), "adjacent, disjoint regions");
    }
    let mut empty = input(0, 64);
    empty.output_base = 0;
    empty.kernarg[..8].fill(0);
    assert!(bind(&kernel, empty).is_ok());
    empty.output_base = empty.kernarg_address;
    empty.kernarg[..8].copy_from_slice(&empty.output_base.to_le_bytes());
    assert!(
        bind(&kernel, empty).is_ok(),
        "empty output has no overlapping bytes"
    );
}

#[test]
fn adjacent_regions_near_u64_max_keep_full_pointer_words() {
    let kernel = Gfx942FillKernelV1::inspect(HSACO, 0).unwrap();
    let mut arguments = input(1, 64);
    arguments.kernarg_address = u64::MAX - 23;
    arguments.output_base = u64::MAX - 7;
    arguments.kernarg[..8].copy_from_slice(&arguments.output_base.to_le_bytes());
    let dispatch = bind(&kernel, arguments).unwrap();
    let execution = dispatch.execute_group(0, seed()).unwrap();
    assert_eq!(execution.kernarg_read_address, arguments.kernarg_address);
    assert_eq!(execution.stores[0].unwrap().address, arguments.output_base);
    assert!(execution.stores[1..].iter().all(Option::is_none));
    for offset in 0..4 {
        assert_eq!(dispatch.byte_after(arguments.output_base + offset, 0xa5), 0);
    }
    assert_eq!(dispatch.byte_after(u64::MAX, 0xa5), 0xa5);
}

#[test]
fn arbitrary_unselected_seed_registers_do_not_change_output_effects() {
    let kernel = Gfx942FillKernelV1::inspect(HSACO, 0).unwrap();
    let input = input(65, 128);
    let dispatch = bind(&kernel, input).unwrap();
    let alternate = Gfx942FillWaveStateV1 {
        sgprs: [u32::MAX; 8],
        vgprs: [[u32::MAX; 4]; 64],
        exec_mask: 0,
        vcc: 0,
        scc: false,
    };
    for group in 0..2 {
        let left = dispatch.execute_group(group, seed()).unwrap();
        let right = dispatch.execute_group(group, alternate).unwrap();
        assert_eq!(left.stores, right.stores);
        assert_eq!(left.kernarg_read_address, right.kernarg_read_address);
        assert_eq!(left.terminal_pc, right.terminal_pc);
        assert_eq!(left.retired_instructions, right.retired_instructions);
        for store in left.stores.iter().flatten() {
            for offset in 0..4 {
                let address = store.address + offset;
                assert_eq!(
                    left.byte_after(address, 0xa5),
                    right.byte_after(address, 0xa5)
                );
                assert_eq!(
                    left.byte_after(address, 0xa5),
                    dispatch.byte_after(address, 0xa5)
                );
            }
        }
    }
}
