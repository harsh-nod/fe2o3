use super::*;

fn metadata() -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: SYMBOL.into(),
        object_sha256: [7; 32],
        kernarg_bytes: 376,
        kernarg_alignment: 8,
        group_segment_bytes: 512,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(120),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..15)
            .map(|index| crate::engineering_wire::ExplicitArgumentV1 {
                offset: index * 8,
                bytes: 8,
                global_buffer: true,
                pointee_alignment: Some(role_alignment(index as usize)),
                access: Some(role_access(index as usize)),
            })
            .collect(),
    }
}

fn regions() -> [OwnedRegion; 15] {
    core::array::from_fn(|index| OwnedRegion {
        buffer: index as u64 + 1,
        base: 0x1000_0000 + index as u64 * 0x1000_0000,
        requested: EXTENTS[index],
        backing: EXTENTS[index].div_ceil(PAGE_BYTES) * PAGE_BYTES,
    })
}

fn terminal() -> [u32; 284] {
    let mut words = [0; 284];
    words[..4].copy_from_slice(&[1, 0, 0, 31]);
    for start in [4, 9] {
        words[start..start + 5].copy_from_slice(&[1, 48, 1, 16, 64]);
    }
    for start in [14, 19] {
        words[start..start + 5].copy_from_slice(&[u32::MAX, u32::MAX, u32::MAX, u32::MAX, 3]);
    }
    for (index, owner) in words[24..154].iter_mut().enumerate() {
        *owner = (index % 64 + 1) as u32;
    }
    words[154..].fill(64);
    words
}

#[test]
fn prefix_v6_roster_retains_v5_data_extents_but_not_old_state_or_grid() {
    assert_eq!(
        EXTENTS[..14],
        super::super::wave_qkv_attention_output_tasks_v5::EXTENTS[..14]
    );
    assert_eq!(
        EXTENTS[14],
        284 * core::mem::size_of::<core::sync::atomic::AtomicU32>()
    );
    assert_eq!(WORKGROUP, [64, 1, 1]);
    assert_eq!(GRID, [4096, 1, 1]);
    assert_eq!(KERNARG_BYTES, 376);
    assert_eq!(INITIAL_STATE[0], 1);
    assert_eq!(INITIAL_STATE[2], 1);
    assert_eq!(INITIAL_STATE.iter().sum::<u32>(), 2);
    let roots = regions();
    assert_eq!(roots[14].backing, 4096);
    validate_regions(&roots, &fixups(&roots)).unwrap();
    for old in [44, 88, 2192, 4096] {
        let mut roots = roots;
        roots[14].requested = old;
        assert!(validate_regions(&roots, &fixups(&roots)).is_err());
    }
}

#[test]
fn prefix_v6_metadata_preserves_all_fifteen_slots_and_readonly_roles() {
    validate_metadata(&metadata(), [7; 32], SYMBOL).unwrap();
    assert!(validate_metadata(&metadata(), [8; 32], SYMBOL).is_err());
    assert!(validate_metadata(&metadata(), [7; 32], "old-v5").is_err());
    for index in 0..15 {
        for change in 0..5 {
            let mut value = metadata();
            let argument = &mut value.explicit_arguments[index];
            match change {
                0 => argument.offset += 8,
                1 => argument.bytes = 4,
                2 => argument.global_buffer = false,
                3 => {
                    argument.pointee_alignment =
                        Some(if role_alignment(index) == 4 { 2 } else { 4 })
                }
                _ => {
                    argument.access = Some(if index < 7 {
                        BufferAccessV1::ReadWrite
                    } else {
                        BufferAccessV1::Read
                    })
                }
            }
            assert!(
                validate_metadata(&value, [7; 32], SYMBOL).is_err(),
                "root {index}, change {change}"
            );
        }
    }
}

#[test]
fn prefix_v6_fixups_refuse_wrong_offsets_roles_and_physical_padding_alias() {
    let roots = regions();
    for index in 0..15 {
        for change in 0..5 {
            let mut pointers = fixups(&roots);
            match change {
                0 => pointers[index].kernarg_offset += 8,
                1 => pointers[index].buffer += 1,
                2 => pointers[index].buffer_offset = 4,
                3 => pointers[index].extent_bytes += 1,
                _ => {
                    pointers[index].access = if index < 7 {
                        BufferAccessV1::ReadWrite
                    } else {
                        BufferAccessV1::Read
                    }
                }
            }
            assert!(validate_regions(&roots, &pointers).is_err());
        }
    }
    let mut roots = roots;
    // The requested 512 bytes do not overlap, but their retained backing does.
    roots[3].backing = PAGE_BYTES * 2;
    roots[4].base = roots[3].base + PAGE_BYTES as u64;
    assert!(validate_regions(&roots, &fixups(&roots)).is_err());
}

#[test]
fn prefix_v6_terminal_requires_all_130_tasks_and_all_284_words() {
    let expected = terminal();
    validate_final_state(expected).unwrap();
    for owner in [1, 64] {
        let mut value = expected;
        value[24..154].fill(owner);
        validate_final_state(value).unwrap();
    }
    for index in 0..284 {
        let mut value = expected;
        value[index] = if (24..154).contains(&index) {
            0
        } else {
            value[index] ^ 1
        };
        assert!(validate_final_state(value).is_err(), "word {index}");
    }
    for invalid in [65, u32::MAX] {
        for index in 24..154 {
            let mut value = expected;
            value[index] = invalid;
            assert!(validate_final_state(value).is_err());
        }
    }
    assert!(validate_final_state(INITIAL_STATE).is_err());
}

#[test]
fn prefix_v6_terminal_does_not_accept_relabelled_v5_or_mlp_tiles_partition() {
    let mut words = terminal();
    words[4..9].copy_from_slice(&[1, 96, 96, 1, 64]);
    words[9..14].copy_from_slice(&[1, 96, 96, 1, 64]);
    assert!(validate_final_state(words).is_err());
    let mut words = terminal();
    words[..6].copy_from_slice(&[1, 0, 65535, 65535, 1, 0]);
    assert!(validate_final_state(words).is_err());
}
