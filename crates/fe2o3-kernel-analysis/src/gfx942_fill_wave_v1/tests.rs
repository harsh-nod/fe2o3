use super::*;

const HSACO: &[u8] =
    include_bytes!("../../../fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco");

fn entry(group: u32, exec_mask: u64) -> Gfx942FillWaveStateV1 {
    Gfx942FillWaveStateV1 {
        sgprs: [0x1000, 1, group, 0xdead_beef, 4, 5, 6, 7],
        vgprs: std::array::from_fn(|lane| [lane as u32, 0xaaaa, 0xbbbb, 0xcccc]),
        exec_mask,
        vcc: u64::MAX,
        scc: true,
    }
}

fn kernarg(pointer: u64, count: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&pointer.to_le_bytes());
    bytes[8..].copy_from_slice(&count.to_le_bytes());
    bytes
}

fn run(state: Gfx942FillWaveStateV1, pointer: u64, count: u64) -> Gfx942FillWaveExecutionV1 {
    Gfx942FillKernelV1::inspect(HSACO, 0)
        .unwrap()
        .execute(
            state,
            &kernarg(pointer, count),
            pointer,
            count * 4,
            [64, 1, 1],
            [0, 0],
        )
        .unwrap()
}

#[test]
fn binds_actual_rust_symbol_not_section_padding() {
    let model = Gfx942FillKernelV1::inspect(HSACO, 0).unwrap();
    assert_eq!(model.binding().entry_size(), 68);
    assert_eq!(model.code_object().as_ptr(), HSACO.as_ptr());
    assert_eq!(
        Gfx942FillKernelV1::inspect(HSACO, 1).unwrap_err(),
        Gfx942FillErrorV1::KernelIndex
    );
    let end = model.binding().entry_file_offset() as usize + 68;
    let mut changed = HSACO.to_vec();
    changed[end] ^= 1;
    assert!(Gfx942FillKernelV1::inspect(&changed, 0).is_ok());
}

#[test]
fn every_instruction_bit_is_checked() {
    let start = Gfx942FillKernelV1::inspect(HSACO, 0)
        .unwrap()
        .binding()
        .entry_file_offset() as usize;
    for byte in 0..68 {
        for bit in 0..8 {
            let mut changed = HSACO.to_vec();
            changed[start + byte] ^= 1 << bit;
            assert!(
                Gfx942FillKernelV1::inspect(&changed, 0).is_err(),
                "byte={byte},bit={bit}"
            );
        }
    }
}

#[test]
fn descriptor_input_relocation_is_not_accepted_with_old_instructions() {
    let offset = Gfx942FillKernelV1::inspect(HSACO, 0)
        .unwrap()
        .binding()
        .descriptor_file_offset() as usize;
    let mut changed = HSACO.to_vec();
    changed[offset + 56..offset + 58].copy_from_slice(&0xau16.to_le_bytes());
    changed[offset + 52..offset + 56].copy_from_slice(&0x88u32.to_le_bytes());
    assert!(inspect_and_bind_kernel_descriptors(&changed).is_ok());
    assert_eq!(
        Gfx942FillKernelV1::inspect(&changed, 0).unwrap_err(),
        Gfx942FillErrorV1::EntryLayout
    );
}

#[test]
fn changed_resource_configuration_and_symbol_extents_are_rejected() {
    let offset = Gfx942FillKernelV1::inspect(HSACO, 0)
        .unwrap()
        .binding()
        .descriptor_file_offset() as usize;
    for field in [44, 48, 52] {
        for bit in 0..32 {
            let mut changed = HSACO.to_vec();
            changed[offset + field + bit / 8] ^= 1 << (bit % 8);
            assert!(
                Gfx942FillKernelV1::inspect(&changed, 0).is_err(),
                "field={field},bit={bit}"
            );
        }
    }
    // Captured ELF64 .dynsym[1] and .symtab[10], st_size at record offset16.
    for size in [64u64, 72, 0x480] {
        let mut changed = HSACO.to_vec();
        for field in [0x438 + 24 + 16, 0xbf8 + 10 * 24 + 16] {
            assert_eq!(&changed[field..field + 8], &68u64.to_le_bytes());
            changed[field..field + 8].copy_from_slice(&size.to_le_bytes());
        }
        assert!(inspect_and_bind_kernel_descriptors(&changed).is_ok());
        assert_eq!(
            Gfx942FillKernelV1::inspect(&changed, 0).unwrap_err(),
            Gfx942FillErrorV1::CodeExtent
        );
    }
}

#[test]
fn full_wave_writes_and_frames_exactly_four_bytes_per_lane() {
    let result = run(entry(0, u64::MAX), 0x8000, 64);
    assert_eq!(result.kernarg_read_address, 0x1_0000_1000);
    assert_eq!(
        result.state.sgprs,
        [u32::MAX, u32::MAX, 0, 0, 0x8000, 0, 64, 0]
    );
    assert_eq!(result.state.exec_mask, u64::MAX);
    assert_eq!(result.state.vcc, u64::MAX);
    assert!(result.state.scc);
    assert_eq!(result.retired_instructions, 14);
    assert_eq!(result.terminal_pc, 0x44);
    for lane in 0..64 {
        assert_eq!(
            result.stores[lane],
            Some(Gfx942FillStoreV1 {
                address: 0x8000 + lane as u64 * 4,
                value: lane as u32
            })
        );
        assert_eq!(
            result.state.vgprs[lane],
            [lane as u32, 0, 0x8000 + lane as u32 * 4, 0]
        );
    }
    for address in 0x7ffc..0x8104 {
        let expected = if (0x8000..0x8100).contains(&address) {
            let offset = address - 0x8000;
            ((offset / 4) as u32).to_le_bytes()[(offset % 4) as usize]
        } else {
            0xa5
        };
        assert_eq!(result.byte_after(address, 0xa5), expected);
    }
}

#[test]
fn sparse_exec_boundaries_high_indices_and_high_pointers() {
    for group in [0, 1, (1 << 26) - 1, 1 << 26, u32::MAX] {
        let first = u64::from(group) * 64;
        for extra in [0, 1, 31, 32, 63, 64, 65] {
            let count = first + extra;
            for mask in [0, 1, 1 << 63, 0xaaaa_aaaa_aaaa_aaaa, u64::MAX] {
                let initial = entry(group, mask);
                let pointer = 0x1234_0000_0000;
                let result = run(initial, pointer, count);
                let mut expected_mask = 0;
                for lane in 0..64 {
                    let active = mask & (1 << lane) != 0;
                    let index = first + lane as u64;
                    let selected = active && index < count;
                    if selected {
                        expected_mask |= 1 << lane;
                    }
                    assert_eq!(
                        result.stores[lane],
                        selected.then_some(Gfx942FillStoreV1 {
                            address: pointer + index * 4,
                            value: index as u32,
                        })
                    );
                    if !active {
                        assert_eq!(result.state.vgprs[lane], initial.vgprs[lane]);
                    } else if selected {
                        let address = pointer + 4 * index;
                        assert_eq!(
                            result.state.vgprs[lane],
                            [
                                index as u32,
                                (index >> 32) as u32,
                                address as u32,
                                (address >> 32) as u32
                            ]
                        );
                    } else if !selected {
                        assert_eq!(
                            result.state.vgprs[lane],
                            [index as u32, (index >> 32) as u32, 0xbbbb, 0xcccc]
                        );
                    }
                }
                assert_eq!(result.state.exec_mask, expected_mask);
                assert_eq!(result.state.vcc, expected_mask);
                assert_eq!(pair(result.state.sgprs[0], result.state.sgprs[1]), mask);
                assert_eq!(result.state.scc, expected_mask != 0);
                assert_eq!(
                    result.retired_instructions,
                    if expected_mask == 0 { 10 } else { 14 }
                );
                assert_eq!(result.terminal_pc, 0x44);
            }
        }
    }
}

#[test]
fn spatial_inputs_fail_closed() {
    use Gfx942FillErrorV1 as E;
    let kernel = Gfx942FillKernelV1::inspect(HSACO, 0).unwrap();
    let state = entry(0, u64::MAX);
    let call = |s, p, n, bytes, size, yz| {
        kernel
            .execute(s, &kernarg(p, n), p, bytes, size, yz)
            .unwrap_err()
    };
    assert_eq!(
        call(state, 0x8000, 64, 256, [32, 1, 1], [0, 0]),
        E::Geometry
    );
    assert_eq!(
        call(state, 0x8000, 64, 256, [64, 1, 1], [1, 0]),
        E::Geometry
    );
    assert_eq!(
        call(state, 0x8001, 64, 256, [64, 1, 1], [0, 0]),
        E::OutputRegion
    );
    assert_eq!(
        call(state, 0x8000, 64, 255, [64, 1, 1], [0, 0]),
        E::OutputRegion
    );
    assert_eq!(
        call(state, u64::MAX - 3, 1, 4, [64, 1, 1], [0, 0]),
        E::OutputRegion
    );
    assert_eq!(
        call(state, 0, u64::MAX, 0, [64, 1, 1], [0, 0]),
        E::OutputRegion
    );
    assert_eq!(
        call(state, 0x1_0000_1000, 4, 16, [64, 1, 1], [0, 0]),
        E::OverlappingRegions
    );
    let mut changed = state;
    changed.sgprs[0] |= 1;
    assert_eq!(
        call(changed, 0x8000, 64, 256, [64, 1, 1], [0, 0]),
        E::KernargRegion
    );
    changed = state;
    changed.sgprs[0] = u32::MAX - 7;
    changed.sgprs[1] = u32::MAX;
    assert_eq!(
        call(changed, 0x8000, 64, 256, [64, 1, 1], [0, 0]),
        E::KernargRegion
    );
    changed = state;
    changed.vgprs[63][0] = 64;
    assert_eq!(
        call(changed, 0x8000, 64, 256, [64, 1, 1], [0, 0]),
        E::LocalId
    );
    changed.vgprs[63][0] = 0;
    assert_eq!(
        call(changed, 0x8000, 64, 256, [64, 1, 1], [0, 0]),
        E::DuplicateLocalId
    );
    changed.exec_mask &= !(1 << 63);
    assert!(
        kernel
            .execute(
                changed,
                &kernarg(0x8000, 64),
                0x8000,
                256,
                [64, 1, 1],
                [0, 0]
            )
            .is_ok()
    );
    assert_eq!(
        kernel
            .execute(state, &kernarg(0x8000, 64), 0x9000, 256, [64, 1, 1], [0, 0])
            .unwrap_err(),
        E::OutputRegion
    );
}

#[test]
fn permuted_local_ids_do_not_change_address_value_relation() {
    let mut state = entry(11, u64::MAX);
    for lane in 0..64 {
        state.vgprs[lane][0] = 63 - lane as u32;
    }
    let result = run(state, 0x8000, 12 * 64);
    for lane in 0..64 {
        let index = 11 * 64 + 63 - lane as u64;
        assert_eq!(
            result.stores[lane],
            Some(Gfx942FillStoreV1 {
                address: 0x8000 + index * 4,
                value: index as u32
            })
        );
    }
}
