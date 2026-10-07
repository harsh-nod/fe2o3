use super::*;

#[test]
fn code_bound_kernarg_layout_requires_exact_dispatch_abi_identity() {
    assert!(prepared_kernarg_layout_matches_code(
        true, [0x41; 32], [0x41; 32]
    ));
    assert!(!prepared_kernarg_layout_matches_code(
        true, [0x41; 32], [0x42; 32]
    ));
    assert!(prepared_kernarg_layout_matches_code(
        false, [0x41; 32], [0x42; 32]
    ));
}

#[test]
fn manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(rendered, GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1);
}

#[test]
fn implicit_kernarg_admits_only_geometry_and_dynamic_lds_fields() {
    let admitted = [
        (
            HiddenValueKind::BlockCountX,
            Cov6ImplicitKernargFieldKindV1::BlockCount(0),
            0,
            4,
        ),
        (
            HiddenValueKind::BlockCountY,
            Cov6ImplicitKernargFieldKindV1::BlockCount(1),
            4,
            4,
        ),
        (
            HiddenValueKind::BlockCountZ,
            Cov6ImplicitKernargFieldKindV1::BlockCount(2),
            8,
            4,
        ),
        (
            HiddenValueKind::GroupSizeX,
            Cov6ImplicitKernargFieldKindV1::GroupSize(0),
            12,
            2,
        ),
        (
            HiddenValueKind::GroupSizeY,
            Cov6ImplicitKernargFieldKindV1::GroupSize(1),
            14,
            2,
        ),
        (
            HiddenValueKind::GroupSizeZ,
            Cov6ImplicitKernargFieldKindV1::GroupSize(2),
            16,
            2,
        ),
        (
            HiddenValueKind::RemainderX,
            Cov6ImplicitKernargFieldKindV1::Remainder(0),
            18,
            2,
        ),
        (
            HiddenValueKind::RemainderY,
            Cov6ImplicitKernargFieldKindV1::Remainder(1),
            20,
            2,
        ),
        (
            HiddenValueKind::RemainderZ,
            Cov6ImplicitKernargFieldKindV1::Remainder(2),
            22,
            2,
        ),
        (
            HiddenValueKind::GlobalOffsetX,
            Cov6ImplicitKernargFieldKindV1::GlobalOffset(0),
            40,
            8,
        ),
        (
            HiddenValueKind::GlobalOffsetY,
            Cov6ImplicitKernargFieldKindV1::GlobalOffset(1),
            48,
            8,
        ),
        (
            HiddenValueKind::GlobalOffsetZ,
            Cov6ImplicitKernargFieldKindV1::GlobalOffset(2),
            56,
            8,
        ),
        (
            HiddenValueKind::GridDimensions,
            Cov6ImplicitKernargFieldKindV1::GridDimensions,
            64,
            2,
        ),
        (
            HiddenValueKind::DynamicLdsSize,
            Cov6ImplicitKernargFieldKindV1::DynamicLdsSize,
            120,
            4,
        ),
    ];
    for (metadata_kind, kind, relative_offset, byte_len) in admitted {
        assert_eq!(
            admitted_cov6_implicit_kernarg_field(metadata_kind),
            Some(Cov6ImplicitKernargFieldV1 {
                kind,
                relative_offset,
                byte_len,
            })
        );
    }

    for rejected in [
        HiddenValueKind::None,
        HiddenValueKind::PrintfBuffer,
        HiddenValueKind::HostcallBuffer,
        HiddenValueKind::HeapV1,
        HiddenValueKind::DefaultQueue,
        HiddenValueKind::CompletionAction,
        HiddenValueKind::MultigridSyncArgument,
        HiddenValueKind::PrivateBase,
        HiddenValueKind::SharedBase,
        HiddenValueKind::QueuePointer,
    ] {
        assert_eq!(admitted_cov6_implicit_kernarg_field(rejected), None);
    }
}

#[test]
fn implicit_kernarg_requires_one_exact_trailing_256_byte_extent() {
    assert_eq!(
        exact_cov6_implicit_kernarg_offset(Some(48), 256, 304),
        Some(48)
    );
    for invalid in [
        exact_cov6_implicit_kernarg_offset(None, 256, 304),
        exact_cov6_implicit_kernarg_offset(Some(48), 255, 303),
        exact_cov6_implicit_kernarg_offset(Some(48), 256, 305),
        exact_cov6_implicit_kernarg_offset(Some(u64::MAX - 127), 256, u64::MAX),
    ] {
        assert_eq!(invalid, None);
    }
}

#[test]
fn caller_must_zero_every_byte_of_the_implicit_suffix() {
    let plan = implicit_plan(16, &[]);
    let mut kernarg = vec![0; 16 + COV6_IMPLICIT_ARGUMENT_BYTES_V1];
    kernarg[0] = 0xa5;
    assert!(validate_caller_zero_cov6_implicit_suffix(7, &kernarg, &plan).is_ok());

    for index in [16, 16 + 127, kernarg.len() - 1] {
        let mut hostile = kernarg.clone();
        hostile[index] = 1;
        assert!(matches!(
            validate_caller_zero_cov6_implicit_suffix(7, &hostile, &plan),
            Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: 7,
                detail: "caller must zero the complete COV6 implicit-kernarg suffix"
            })
        ));
    }
    assert!(validate_caller_zero_cov6_implicit_suffix(7, &kernarg[..271], &plan).is_err());
}

#[test]
fn implicit_kernarg_uses_full_block_counts_and_partial_remainders() {
    let values = derive_cov6_implicit_kernarg_values(
        AqlDispatchGeometryV1::new([257, 3, 1], [64, 2, 1]).unwrap(),
        384,
        false,
    )
    .unwrap();
    assert_eq!(values.dispatch_shape.block_count(), [4, 1, 1]);
    assert_eq!(values.dispatch_shape.group_size(), [64, 2, 1]);
    assert_eq!(values.dispatch_shape.remainder(), [1, 1, 0]);
    assert_eq!(values.dispatch_shape.grid_dimensions(), 2);
    assert_eq!(values.dynamic_lds_size, 384);

    assert!(
        derive_cov6_implicit_kernarg_values(
            AqlDispatchGeometryV1::new([257, 3, 1], [64, 2, 1]).unwrap(),
            384,
            true,
        )
        .is_err()
    );
    let uniform = derive_cov6_implicit_kernarg_values(
        AqlDispatchGeometryV1::new([256, 4, 1], [64, 2, 1]).unwrap(),
        0,
        true,
    )
    .unwrap();
    assert_eq!(uniform.dispatch_shape.block_count(), [4, 2, 1]);
    assert_eq!(uniform.dispatch_shape.remainder(), [0, 0, 0]);
}

#[test]
fn private_initializer_writes_only_metadata_declared_implicit_fields() {
    let kinds = [
        HiddenValueKind::BlockCountX,
        HiddenValueKind::BlockCountY,
        HiddenValueKind::BlockCountZ,
        HiddenValueKind::GroupSizeX,
        HiddenValueKind::GroupSizeY,
        HiddenValueKind::GroupSizeZ,
        HiddenValueKind::RemainderX,
        HiddenValueKind::RemainderY,
        HiddenValueKind::RemainderZ,
        HiddenValueKind::GlobalOffsetX,
        HiddenValueKind::GlobalOffsetY,
        HiddenValueKind::GlobalOffsetZ,
        HiddenValueKind::GridDimensions,
        HiddenValueKind::DynamicLdsSize,
    ];
    let plan = implicit_plan(8, &kinds);
    let values = derive_cov6_implicit_kernarg_values(
        AqlDispatchGeometryV1::new([257, 3, 1], [64, 2, 1]).unwrap(),
        384,
        false,
    )
    .unwrap();
    let mut actual = vec![0; 8 + COV6_IMPLICIT_ARGUMENT_BYTES_V1];
    actual[..8].fill(0xa5);
    initialize_cov6_implicit_kernarg(&mut actual, &plan, values);

    let mut expected = vec![0; actual.len()];
    expected[..8].fill(0xa5);
    put_u32(&mut expected, 8, 4);
    put_u32(&mut expected, 12, 1);
    put_u32(&mut expected, 16, 1);
    put_u16(&mut expected, 20, 64);
    put_u16(&mut expected, 22, 2);
    put_u16(&mut expected, 24, 1);
    put_u16(&mut expected, 26, 1);
    put_u16(&mut expected, 28, 1);
    put_u16(&mut expected, 30, 0);
    put_u64(&mut expected, 48, 0);
    put_u64(&mut expected, 56, 0);
    put_u64(&mut expected, 64, 0);
    put_u16(&mut expected, 72, 2);
    put_u32(&mut expected, 128, 384);
    assert_eq!(actual, expected);
}

#[test]
fn public_buffer_contract_rejects_pointer_range_alignment_and_alias_drift() {
    let binding = Gfx942DispatchBufferBindingV1::new(0, 0, 0, 64);
    let bytes = [0u8; 32];
    let (patch, effect, completed_snapshot) = validate_inspected_buffer_contract(
        0,
        &bytes,
        &binding,
        4096,
        4096,
        &[binding],
        inspected(8, Some(ArgumentAccess::ReadOnly), Some(16)),
    )
    .unwrap();
    assert_eq!(patch.byte_offset, 8);
    assert_eq!(effect, DeviceDataEffectV1::ReadOnly);
    assert_eq!(completed_snapshot, None);

    let mut nonzero = bytes;
    nonzero[8] = 1;
    assert!(
        validate_inspected_buffer_contract(
            0,
            &nonzero,
            &binding,
            4096,
            4096,
            &[binding],
            inspected(8, Some(ArgumentAccess::ReadOnly), Some(16)),
        )
        .is_err()
    );

    let overflow = Gfx942DispatchBufferBindingV1::new(0, 0, 4080, 32);
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &overflow,
            4096,
            4096,
            &[overflow],
            inspected(8, Some(ArgumentAccess::WriteOnly), Some(16)),
        )
        .is_err()
    );

    let misaligned = Gfx942DispatchBufferBindingV1::new(0, 0, 4, 64);
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &misaligned,
            4096,
            4096,
            &[misaligned],
            inspected(8, Some(ArgumentAccess::WriteOnly), Some(16)),
        )
        .is_err()
    );

    let alias = Gfx942DispatchBufferBindingV1::new(1, 0, 32, 64);
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &binding,
            4096,
            4096,
            &[binding, alias],
            inspected(8, Some(ArgumentAccess::ReadOnly), Some(16)),
        )
        .is_err()
    );
}

#[test]
fn public_buffer_contract_requires_inspected_access_and_alignment() {
    let binding = Gfx942DispatchBufferBindingV1::new(0, 0, 0, 64);
    let bytes = [0u8; 32];
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &binding,
            4096,
            4096,
            &[binding],
            inspected(8, None, Some(16)),
        )
        .is_err()
    );
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &binding,
            4096,
            4096,
            &[binding],
            inspected(8, Some(ArgumentAccess::ReadOnly), None),
        )
        .is_err()
    );
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &binding,
            4096,
            4096,
            &[binding],
            InspectedBufferContractV1 {
                pointer_offset: 8,
                declared_access: Some(ArgumentAccess::WriteOnly),
                actual_access: Some(ArgumentAccess::ReadOnly),
                pointee_alignment: Some(16),
            },
        )
        .is_err()
    );
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &binding,
            4096,
            4096,
            &[binding],
            inspected(4, Some(ArgumentAccess::ReadOnly), Some(16)),
        )
        .is_err()
    );
}

#[test]
fn completed_snapshot_binding_requires_strict_isolated_writable_enclosure() {
    let valid = Gfx942DispatchBufferBindingV1::new_with_completed_snapshot(0, 0, 64, 64, 32, 128);
    let bytes = [0u8; 32];
    let (_, effect, snapshot) = validate_inspected_buffer_contract(
        0,
        &bytes,
        &valid,
        256,
        64,
        &[valid],
        inspected(8, Some(ArgumentAccess::WriteOnly), Some(16)),
    )
    .unwrap();
    assert_eq!(effect, DeviceDataEffectV1::WriteOnly);
    assert_eq!(
        snapshot,
        Some(CompletedSnapshotRangeV1 {
            offset: 32,
            byte_len: 128,
            interior_offset: 64,
            interior_byte_len: 64,
        })
    );

    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &valid,
            256,
            64,
            &[valid],
            inspected(8, Some(ArgumentAccess::ReadOnly), Some(16)),
        )
        .is_err()
    );
    for invalid in [
        Gfx942DispatchBufferBindingV1::new_with_completed_snapshot(0, 0, 64, 64, 64, 96),
        Gfx942DispatchBufferBindingV1::new_with_completed_snapshot(0, 0, 64, 64, 32, 96),
        Gfx942DispatchBufferBindingV1::new_with_completed_snapshot(0, 0, 64, 64, 32, 240),
    ] {
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &invalid,
                256,
                64,
                &[invalid],
                inspected(8, Some(ArgumentAccess::WriteOnly), Some(16)),
            )
            .is_err()
        );
    }

    let alias = Gfx942DispatchBufferBindingV1::new(1, 0, 144, 16);
    assert!(
        validate_inspected_buffer_contract(
            0,
            &bytes,
            &valid,
            256,
            64,
            &[valid, alias],
            inspected(8, Some(ArgumentAccess::WriteOnly), Some(16)),
        )
        .is_err()
    );
}

#[test]
fn completed_snapshot_premises_require_initialized_coherent_isolated_writes() {
    let host = Gfx942FixedDispatchDataLayoutV1 {
        kind: Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        requested_bytes: 256,
        alignment: 64,
    };
    let device = Gfx942FixedDispatchDataLayoutV1 {
        kind: Gfx942FixedDispatchDataKindV1::DeviceLocal,
        requested_bytes: 256,
        alignment: 64,
    };
    let writable = vec![CompletedWritableRangeV1 {
        offset: 64,
        byte_len: 64,
    }];
    let snapshot = CompletedSnapshotRangeV1 {
        offset: 32,
        byte_len: 128,
        interior_offset: 64,
        interior_byte_len: 64,
    };
    assert!(
        validate_completed_snapshot_premises(
            &[host],
            &[true],
            core::slice::from_ref(&writable),
            &[vec![snapshot]],
        )
        .is_ok()
    );
    assert!(
        validate_completed_snapshot_premises(
            &[host],
            &[false],
            core::slice::from_ref(&writable),
            &[vec![snapshot]],
        )
        .is_err()
    );
    assert!(
        validate_completed_snapshot_premises(
            &[device],
            &[true],
            core::slice::from_ref(&writable),
            &[vec![snapshot]],
        )
        .is_err()
    );

    let extra_write = vec![
        CompletedWritableRangeV1 {
            offset: 64,
            byte_len: 64,
        },
        CompletedWritableRangeV1 {
            offset: 144,
            byte_len: 8,
        },
    ];
    assert!(
        validate_completed_snapshot_premises(&[host], &[true], &[extra_write], &[vec![snapshot]],)
            .is_err()
    );
    let overlapping_snapshot = CompletedSnapshotRangeV1 {
        offset: 16,
        byte_len: 152,
        interior_offset: 64,
        interior_byte_len: 64,
    };
    assert!(
        validate_completed_snapshot_premises(
            &[host],
            &[true],
            &[writable],
            &[vec![snapshot, overlapping_snapshot]],
        )
        .is_err()
    );
    assert!(validate_completed_snapshot_premises(&[host], &[], &[], &[]).is_err());
}

#[test]
fn public_read_effect_requires_sealed_initialization() {
    assert!(
        validate_initialization_premises(&[Some(DeviceDataEffectV1::WriteOnly)], &[false],).is_ok()
    );
    assert!(
        validate_initialization_premises(&[Some(DeviceDataEffectV1::ReadOnly)], &[true],).is_ok()
    );
    assert!(matches!(
        validate_initialization_premises(&[Some(DeviceDataEffectV1::ReadWrite)], &[false],),
        Err(Gfx942DispatchBindingErrorV1::InvalidData { index: 0, .. })
    ));
    assert_eq!(
        merge_effect(
            Some(DeviceDataEffectV1::WriteOnly),
            DeviceDataEffectV1::ReadOnly,
        ),
        DeviceDataEffectV1::ReadWrite
    );
}

#[test]
fn public_fixed_batch_retains_untouched_roster_premises() {
    let layout = Gfx942FixedDispatchDataLayoutV1 {
        kind: Gfx942FixedDispatchDataKindV1::DeviceLocal,
        requested_bytes: 256,
        alignment: 64,
    };
    for untouched_initialized in [false, true] {
        let initialized = [false, untouched_initialized];
        let plans = plan_public_retained_data(
            &[layout, layout],
            &initialized,
            vec![Some(DeviceDataEffectV1::WriteOnly), None],
            vec![
                vec![CompletedWritableRangeV1 {
                    offset: 0,
                    byte_len: 256,
                }],
                vec![],
            ],
            vec![vec![], vec![]],
        )
        .unwrap();

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].effect, Some(DeviceDataEffectV1::WriteOnly));
        assert!(!plans[0].fully_initialized);
        assert_eq!(plans[1].effect, None);
        assert_eq!(plans[1].fully_initialized, untouched_initialized);
        assert!(plans[1].writable_ranges.is_empty());
        assert!(plans[1].completed_snapshots.is_empty());
    }

    assert!(matches!(
        validate_initialization_premises(
            &[
                Some(DeviceDataEffectV1::WriteOnly),
                Some(DeviceDataEffectV1::ReadOnly),
            ],
            &[false, false],
        ),
        Err(Gfx942DispatchBindingErrorV1::InvalidData { index: 1, .. })
    ));
    assert!(
        validate_initialization_premises(
            &[
                Some(DeviceDataEffectV1::WriteOnly),
                Some(DeviceDataEffectV1::ReadOnly),
            ],
            &[false, true],
        )
        .is_ok()
    );
}

#[test]
fn packet_and_data_bounds_are_exact() {
    assert_eq!(
        validate_packet_count::<0>().unwrap_err().to_string(),
        "ZeroPacketCount"
    );
    assert!(validate_packet_count::<1>().is_ok());
    assert!(validate_packet_count::<8192>().is_ok());
    assert!(matches!(
        validate_packet_count::<8193>(),
        Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum { .. })
    ));
    assert!(matches!(
        validate_data_inputs(&[]),
        Err(Gfx942DispatchBindingErrorV1::DataLeaseCount { .. })
    ));
    let sixteen: Vec<_> = (1..=16)
        .map(|seed| fake_input(seed, premise(seed as u8, DeviceDataEffectV1::WriteOnly)))
        .collect();
    assert!(validate_data_inputs(&sixteen).is_ok());
    let seventeen: Vec<_> = (1..=17)
        .map(|seed| fake_input(seed, premise(seed as u8, DeviceDataEffectV1::WriteOnly)))
        .collect();
    assert!(matches!(
        validate_data_inputs(&seventeen),
        Err(Gfx942DispatchBindingErrorV1::DataLeaseCount { .. })
    ));
}

#[test]
fn fixed_batch_ring_must_cover_every_packet_before_native_preparation() {
    assert!(validate_fixed_batch_ring::<1024>(65_536).is_ok());
    assert!(validate_fixed_batch_ring::<8192>(524_288).is_ok());
    assert!(matches!(
        validate_fixed_batch_ring::<8192>(262_144),
        Err(Gfx942DispatchBindingErrorV1::RingCapacity {
            requested: 8192,
            capacity: 4096,
        })
    ));
    assert!(matches!(
        validate_fixed_batch_ring::<8193>(1_048_576),
        Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum { .. })
    ));
}

#[test]
fn data_premises_reject_uninitialized_reads_and_identity_aliases() {
    for effect in [DeviceDataEffectV1::ReadOnly, DeviceDataEffectV1::ReadWrite] {
        assert!(matches!(
            validate_data_inputs(&[fake_input(1, premise(1, effect))]),
            Err(Gfx942DispatchBindingErrorV1::InvalidData {
                detail: "read requires authenticated initialized-content authority",
                ..
            })
        ));
    }
    assert!(
        validate_data_inputs(&[fake_input(1, premise(1, DeviceDataEffectV1::WriteOnly))]).is_ok()
    );
    assert!(matches!(
        validate_data_inputs(&[
            fake_input(1, premise(7, DeviceDataEffectV1::WriteOnly)),
            fake_input(2, premise(7, DeviceDataEffectV1::WriteOnly)),
        ]),
        Err(Gfx942DispatchBindingErrorV1::InvalidData {
            detail: "role identity alias",
            ..
        })
    ));
}

#[test]
fn typed_pointer_layout_is_complete_bounded_and_nonoverlapping() {
    let data = [
        fake_input(1, premise(1, DeviceDataEffectV1::WriteOnly)),
        fake_input(2, premise(2, DeviceDataEffectV1::WriteOnly)),
    ];
    let valid = typed(
        32,
        vec![
            DevicePointerPatchV1::new(0, 0, 0, 4096, 8),
            DevicePointerPatchV1::new(8, 1, 0, 4096, 8),
        ],
    );
    assert!(validate_kernargs(&[valid], 32, &data).is_ok());

    let cases = [
        typed(32, vec![DevicePointerPatchV1::new(1, 0, 0, 8, 8)]),
        typed(32, vec![DevicePointerPatchV1::new(32, 0, 0, 8, 8)]),
        typed(32, vec![DevicePointerPatchV1::new(0, 2, 0, 8, 8)]),
        typed(32, vec![DevicePointerPatchV1::new(0, 0, 4090, 8, 8)]),
        typed(
            32,
            vec![
                DevicePointerPatchV1::new(0, 0, 0, 8, 8),
                DevicePointerPatchV1::new(0, 1, 0, 8, 8),
            ],
        ),
    ];
    for invalid in cases {
        assert!(matches!(
            validate_kernargs(&[invalid], 32, &data),
            Err(Gfx942DispatchBindingErrorV1::InvalidKernarg { .. })
        ));
    }
}

#[test]
fn every_data_lease_must_be_referenced_by_every_batch_shape() {
    let data = [
        fake_input(1, premise(1, DeviceDataEffectV1::WriteOnly)),
        fake_input(2, premise(2, DeviceDataEffectV1::WriteOnly)),
    ];
    let only_first = typed(16, vec![DevicePointerPatchV1::new(0, 0, 0, 8, 8)]);
    assert!(matches!(
        validate_kernargs(&[only_first], 16, &data),
        Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 1,
            detail: "lease not referenced by kernarg"
        })
    ));
}

#[test]
fn owner_phase_is_linear_and_terminal_poison_is_sticky() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let generation = owner.next().unwrap();
    assert_eq!(generation, 1);
    assert!(owner.active().is_err());
    owner.commit_begin(generation);
    assert_eq!(owner.active().unwrap(), generation);
    assert!(owner.cancel(generation + 1).is_err());
    owner.complete(generation).unwrap();
    assert!(owner.recycle(generation + 1).is_err());
    owner.recycle(generation).unwrap();
    assert!(owner.ensure_prepared().is_ok());
    assert_eq!(owner.returned_generation().unwrap(), generation);
    owner.poison();
    assert!(matches!(
        owner.next(),
        Err(Gfx942DispatchBindingErrorV1::Poisoned)
    ));
}

#[test]
fn multi_inflight_epoch_table_is_preallocated_bounded_and_reusable_without_aba() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let table = owner.slots.as_ptr();
    let mut identities = Vec::new();
    for generation in 1..=GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1 as u64 {
        let identity = owner
            .reserve(queue, test_completion_roster_v1(generation))
            .unwrap();
        assert_eq!(identity.dispatch_generation(), generation);
        identities.push(identity);
    }
    assert_eq!(owner.slots.as_ptr(), table);
    for identity in &identities {
        assert_ne!(identity.recipe_occurrence, 0);
        assert_ne!(identity.slot_generation, 0);
        assert_ne!(identity.dispatch_generation, 0);
    }
    let next_generation = owner.next_generation;
    let full_snapshot = owner.clone();
    assert!(matches!(
        owner.reserve(queue, test_completion_roster_v1(next_generation)),
        Err(Gfx942DispatchBindingErrorV1::DispatchEpochCapacity {
            maximum: GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1,
        })
    ));
    assert_eq!(owner, full_snapshot);

    let released = identities[1];
    owner.cancel_epoch(released).unwrap();
    let reused = owner
        .reserve(queue, test_completion_roster_v1(next_generation))
        .unwrap();
    assert_eq!(reused.slot_index, released.slot_index);
    assert_eq!(reused.slot_generation, released.slot_generation + 1);
    assert!(matches!(
        owner.cancel_epoch(released),
        Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
    ));
    assert_eq!(owner.slots.as_ptr(), table);
}

#[test]
fn exact_epoch_authentication_rejects_every_identity_and_completion_substitution() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let identity = owner.reserve(queue, test_completion_roster_v1(1)).unwrap();
    let completion = test_completion_occurrence_v1(1);
    owner.mark_published(identity, completion).unwrap();

    let foreign = DispatchGenerationOwnerV1::new()
        .unwrap()
        .reserve(queue, test_completion_roster_v1(1))
        .unwrap();
    let identities = [
        foreign,
        DispatchEpochIdentityV1 {
            slot_generation: identity.slot_generation + 1,
            ..identity
        },
        DispatchEpochIdentityV1 {
            dispatch_generation: identity.dispatch_generation + 1,
            ..identity
        },
        DispatchEpochIdentityV1 {
            slot_index: identity.slot_index + 1,
            ..identity
        },
        DispatchEpochIdentityV1 {
            slot_index: u16::MAX,
            ..identity
        },
        DispatchEpochIdentityV1 {
            recipe_occurrence: identity.recipe_occurrence + 1,
            ..identity
        },
        DispatchEpochIdentityV1 {
            queue: QueueKeyV1 {
                id: fe2o3_runtime_model::QueueInstanceIdV1(queue.id.0 + 1),
                ..queue
            },
            ..identity
        },
    ];
    for stale in identities {
        assert!(
            r66_retained_published_occurrence_observation_v1(&owner, stale, completion).is_none()
        );
        assert!(matches!(
            owner.validate_published(stale, completion),
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
                | Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration)
        ));
    }

    let substitutions = [
        CompletionBatchOccurrenceV1 {
            batch_id: completion.batch_id + 1,
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            queue: QueueKeyV1 {
                id: fe2o3_runtime_model::QueueInstanceIdV1(queue.id.0 + 1),
                ..queue
            },
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            signal_mapping: MemoryMappingKeyV1 {
                id: fe2o3_runtime_model::MappingIdV1(completion.signal_mapping.id.0 + 1),
                ..completion.signal_mapping
            },
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            packet_count: completion.packet_count + 1,
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            first_packet_id: completion.first_packet_id + 1,
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            last_packet_id: completion.last_packet_id + 1,
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            roster_sha256: [0xa5; 32],
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            dispatch_roster: CompletionDispatchRosterV1 {
                queue: QueueKeyV1 {
                    id: fe2o3_runtime_model::QueueInstanceIdV1(queue.id.0 + 1),
                    ..queue
                },
                ..completion.dispatch_roster
            },
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            dispatch_roster: CompletionDispatchRosterV1 {
                packet_count: completion.dispatch_roster.packet_count + 1,
                ..completion.dispatch_roster
            },
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            dispatch_roster: CompletionDispatchRosterV1 {
                dispatch_generation: completion.dispatch_roster.dispatch_generation + 1,
                ..completion.dispatch_roster
            },
            ..completion
        },
        CompletionBatchOccurrenceV1 {
            dispatch_roster: CompletionDispatchRosterV1 {
                roster_sha256: [0x5a; 32],
                ..completion.dispatch_roster
            },
            ..completion
        },
    ];
    for stale in substitutions {
        assert!(
            r66_retained_published_occurrence_observation_v1(&owner, identity, stale).is_none()
        );
        assert!(matches!(
            owner.validate_published(identity, stale),
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
        ));
    }
    owner.validate_published(identity, completion).unwrap();
}
