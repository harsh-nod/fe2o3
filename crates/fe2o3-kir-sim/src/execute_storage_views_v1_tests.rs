use super::storage_tests_v1::*;
use super::*;

pub(super) fn wide_constant(result: u32, scratch: u32, bits: u128) -> [Operation; 8] {
    use fe2o3_kernel_ir::{BinaryOp, CastKind};
    let narrow = |id, bits| value(id, Type::Scalar(ScalarType::U64),
        OperationKind::Constant(Constant::U64(bits)));
    let widen = |id, operand| value(id, Type::Scalar(ScalarType::U128),
        OperationKind::Cast { kind: CastKind::ZeroExtend, value: ValueId(operand),
            to: Type::Scalar(ScalarType::U128) });
    // KIR has no 128-bit constant opcode. Keep both words in the expected value.
    [
        narrow(scratch, bits as u64),
        narrow(scratch + 1, (bits >> 64) as u64),
        narrow(scratch + 2, 64),
        widen(scratch + 3, scratch),
        widen(scratch + 4, scratch + 1),
        widen(scratch + 5, scratch + 2),
        value(scratch + 6, Type::Scalar(ScalarType::U128), OperationKind::Binary {
            op: BinaryOp::ShiftLeft, lhs: ValueId(scratch + 4), rhs: ValueId(scratch + 5) }),
        value(result, Type::Scalar(ScalarType::U128), OperationKind::Binary {
            op: BinaryOp::BitOr, lhs: ValueId(scratch + 3), rhs: ValueId(scratch + 6) }),
    ]
}

fn discriminant_read_module(niche: bool, initialized: bool, initial: u8, replacement: u8, expected: [u128; 2]) -> Module {
    use fe2o3_kernel_ir::{ComparePredicate, StorageVariantEncodingV1 as Encoding, StorageVariantV1};
    let field = |offset, row| StorageFieldV1 { offset, layout: StorageLayoutIdV1(row) };
    let layouts = vec![
        StorageLayoutV1 { size: 1, alignment: 1, kind: StorageLayoutKindV1::Scalar(ScalarType::U8) },
        StorageLayoutV1 { size: 4, alignment: 4, kind: StorageLayoutKindV1::Scalar(ScalarType::U32) },
        StorageLayoutV1 { size: 12, alignment: 4, kind: StorageLayoutKindV1::Record(
            vec![field(0, 0), field(4, 1)].into_boxed_slice()) },
        StorageLayoutV1 { size: 12, alignment: 4, kind: StorageLayoutKindV1::Variants {
            encoding: if niche { Encoding::Niche { tag: field(0, 0), untagged_variant: 0,
                first_niche_variant: 1, last_niche_variant: 2, niche_start: 255 } }
                else { Encoding::Direct { tag: field(0, 0) } },
            variants: [3_u128, 17, 250].into_iter().zip([255, 1_u128 << 100, u128::MAX])
                .enumerate().map(|(ordinal, (bits, discriminant))| StorageVariantV1 {
                    discriminant: if niche { ordinal as u128 } else { discriminant },
                    direct_tag_bits: (!niche).then_some(bits), uninhabited: false,
                    layout: StorageLayoutIdV1(2) }).collect::<Vec<_>>().into_boxed_slice(),
        } },
        StorageLayoutV1 { size: 12, alignment: 4, kind: StorageLayoutKindV1::Union(
            vec![field(0, 3), field(0, 0)].into_boxed_slice()) },
    ];
    let space = AddressSpace::Private;
    let access = MemoryAccess::new(space, 1);
    let tag_write = |value| Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue {
        address: ValueId(3), value: ValueId(value), access }));
    let tag_read = |id| value(id, Type::Scalar(ScalarType::U128),
        OperationKind::Storage(StorageOperationV1::ReadDiscriminant { address: ValueId(2), access }));
    let mut operations = vec![allocate(1, 4, space),
        project(2, 3, 1, StorageProjectionV1::Field(0), space),
        project(3, 0, 1, StorageProjectionV1::Field(1), space),
        value(4, Type::Scalar(ScalarType::U8), OperationKind::Constant(Constant::U8(initial)))];
    if initialized { operations.push(tag_write(4)); }
    operations.extend([tag_read(5),
        value(6, Type::Scalar(ScalarType::U8), OperationKind::Constant(Constant::U8(replacement))),
        tag_write(6), tag_read(7)]);
    operations.extend(wide_constant(8, 100, expected[0]));
    operations.extend(wide_constant(9, 107, expected[1]));
    operations.extend([
        value(10, Type::Scalar(ScalarType::Bool), OperationKind::Compare { predicate: ComparePredicate::Equal,
            lhs: ValueId(5), rhs: ValueId(8) }),
        value(11, Type::Scalar(ScalarType::Bool), OperationKind::Compare { predicate: ComparePredicate::Equal,
            lhs: ValueId(7), rhs: ValueId(9) }), constant(12, 1), constant(13, 0),
        value(14, Type::Scalar(ScalarType::U32), OperationKind::Select { condition: ValueId(11),
            true_value: ValueId(12), false_value: ValueId(13) }),
        value(15, Type::Scalar(ScalarType::U32), OperationKind::Select { condition: ValueId(10),
            true_value: ValueId(14), false_value: ValueId(13) }), output(15)]);
    module(layouts, operations)
}

#[test]
fn discriminant_reads_return_logical_u128_bits_and_observe_alias_retags_not_uninitialized_payloads() {
    for (niche, initial, replacement, expected) in [
        (false, 3, 17, [255, 1_u128 << 100]),
        (false, 250, 3, [u128::MAX, 255]),
        (true, 255, 0, [1, 2]),
        (true, 0, 42, [2, 0]),
    ] {
        let module = discriminant_read_module(niche, true, initial, replacement, expected);
        let mut events = Events::default();
        assert_eq!(output_bits(&run(&module, &mut events).unwrap()), 1);
        let reads = events.0.iter().filter_map(|event| match event.kind {
            SimulationEventKindV1::MemoryRead { bytes, .. } => Some(bytes), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(reads, [1, 1], "only the actual tag bytes are read");
    }
}

#[test]
fn discriminant_reads_reject_invalid_or_uninitialized_actual_tags_before_output() {
    for (initialized, bits) in [(false, 3), (true, 23)] {
        let module = discriminant_read_module(false, initialized, bits, 17, [255, 1 << 100]);
        let mut events = Events::default();
        assert!(run(&module, &mut events).is_err());
        assert!(!events.0.iter().any(|event| matches!(event.kind,
            SimulationEventKindV1::MemoryWrite { bytes: 4, .. })));
    }
}

pub(super) fn invocation() -> SimulationInvocationV1 {
    SimulationInvocationV1 {
        global: [0; 3],
        workgroup: [0; 3],
        local: [0; 3],
        workgroup_size: [1; 3],
        workgroup_count: [1; 3],
        launch_extent: [1; 3],
    }
}

pub(super) fn memory(
    bytes: Vec<u8>,
    initialized: Vec<bool>,
    space: AddressSpace,
) -> (Memory, StorageAddressV1) {
    let limits = SimulationLimitsV1::default();
    let mut memory = Memory::new(0, 0, limits).unwrap();
    memory.storage_invocation.set(Some(invocation()));
    let width = bytes.len();
    let id = memory
        .allocate(space, AccessMode::ReadWrite, 4, bytes, initialized, limits)
        .unwrap();
    (
        memory,
        StorageAddressV1 {
            pointer: PointerValue {
                allocation: id,
                byte_offset: 0,
                element: ScalarType::U8,
                address_space: space,
                access: AccessMode::ReadWrite,
                lower_bound: 0,
                upper_bound: width,
                abi_argument_ordinal: NO_ABI_ARGUMENT_V1,
                storage_guard: None,
                generic_exposed: false,
            },
            layout: StorageLayoutIdV1(0),
        },
    )
}

#[test]
fn storage_actual_array_projection_checks_index_and_preserves_last_element() {
    let space = AddressSpace::Private;
    for index in [1, 2, u64::MAX] {
        let module = module(
            rows(),
            vec![
                allocate(1, 2, space),
                constant(2, 31),
                value(
                    3,
                    Type::INDEX,
                    OperationKind::Constant(Constant::Index(index)),
                ),
                project(4, 0, 1, StorageProjectionV1::ArrayIndex(ValueId(3)), space),
                write(4, 2, space),
                read(5, 4, space),
                output(5),
            ],
        );
        let mut events = Events::default();
        let result = run(&module, &mut events);
        if index == 1 {
            assert_eq!(output_bits(&result.unwrap()), 31);
        } else {
            assert!(result.is_err());
            assert!(
                !events
                    .0
                    .iter()
                    .any(|e| matches!(e.kind, SimulationEventKindV1::MemoryWrite { .. }))
            );
        }
    }
}

pub(super) fn variant_module(stale: bool, uninitialized: bool) -> Module {
    use fe2o3_kernel_ir::{StorageVariantEncodingV1, StorageVariantV1};
    let mut layouts = rows();
    layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 4,
        kind: StorageLayoutKindV1::Record(
            vec![StorageFieldV1 {
                offset: 4,
                layout: StorageLayoutIdV1(0),
            }]
            .into_boxed_slice(),
        ),
    });
    layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 4,
        kind: StorageLayoutKindV1::Variants {
            encoding: StorageVariantEncodingV1::Direct {
                tag: StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                },
            },
            variants: [10, 20]
                .into_iter()
                .map(|tag| StorageVariantV1 {
                    discriminant: tag,
                    direct_tag_bits: Some(tag),
                    uninhabited: false,
                    layout: StorageLayoutIdV1(3),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
    });
    layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 4,
        kind: StorageLayoutKindV1::Union(
            vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                },
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(4),
                },
            ]
            .into_boxed_slice(),
        ),
    });
    let space = AddressSpace::Private;
    // This low-level Union fixture writes actual tag bytes. It is not a source
    // enum constructor or authority for the still-required SetDiscriminant hook.
    let mut operations = vec![
        allocate(1, 5, space),
        constant(2, 10),
        project(3, 0, 1, StorageProjectionV1::Field(0), space),
    ];
    if !uninitialized {
        operations.push(write(3, 2, space));
    }
    operations.extend([
        project(4, 4, 1, StorageProjectionV1::Field(1), space),
        project(
            5,
            3,
            4,
            StorageProjectionV1::Variant {
                index: 0,
                access: MemoryAccess::new(space, 4),
            },
            space,
        ),
        project(6, 0, 5, StorageProjectionV1::Field(0), space),
        constant(7, 99),
        write(6, 7, space),
    ]);
    if stale {
        operations.push(write(3, 2, space));
    }
    operations.extend([read(8, 6, space), output(8)]);
    module(layouts, operations)
}

#[test]
fn storage_variant_reads_tag_once_and_payload_write_preserves_guard() {
    let module = variant_module(false, false);
    let mut events = Events::default();
    assert_eq!(output_bits(&run(&module, &mut events).unwrap()), 99);
    let reads: Vec<_> = events
        .0
        .iter()
        .filter_map(|e| match e.kind {
            SimulationEventKindV1::MemoryRead { offset, bytes, .. } => Some((offset, bytes)),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [(0, 4), (4, 4)]);
}

#[test]
fn storage_variant_equal_tag_rewrite_invalidates_without_hidden_reread() {
    let module = variant_module(true, false);
    let mut events = Events::default();
    assert!(run(&module, &mut events).is_err());
    let reads: Vec<_> = events
        .0
        .iter()
        .filter_map(|e| match e.kind {
            SimulationEventKindV1::MemoryRead { offset, bytes, .. } => Some((offset, bytes)),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [(0, 4)]);
}

#[test]
fn storage_fresh_variant_projection_is_not_an_initializer() {
    let mut events = Events::default();
    assert!(run(&variant_module(false, true), &mut events).is_err());
    assert!(
        !events
            .0
            .iter()
            .any(|e| matches!(e.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

#[test]
fn storage_legacy_scalar_commit_invalidates_tag_but_unrelated_range_does_not() {
    let (mut memory, address) = memory(vec![0; 8], vec![true; 8], AddressSpace::Private);
    let guard = memory.storage_register_guard_v1(&address, 0, 4).unwrap();
    let mut viewed = address.clone();
    viewed.pointer.storage_guard = Some(guard);
    let mut raw = address.pointer.clone();
    raw.element = ScalarType::U32;
    raw.byte_offset = 4;
    memory
        .prepare_store(&raw, 4)
        .unwrap()
        .commit(ScalarBitsV1::u32(1));
    assert!(memory.allocation(&viewed.pointer).is_ok());
    raw.byte_offset = 0;
    memory
        .prepare_store(&raw, 4)
        .unwrap()
        .commit(ScalarBitsV1::u32(0));
    assert!(memory.allocation(&viewed.pointer).is_err());
}

#[test]
fn storage_dead_allocation_and_wrong_invocation_never_reuse_view_identity() {
    let (mut memory, address) = memory(vec![0; 4], vec![true; 4], AddressSpace::Private);
    let mut foreign = invocation();
    foreign.global[0] = 1;
    assert!(
        memory
            .storage_validate_v1(
                &address,
                MemoryAccess::new(AddressSpace::Private, 4),
                4,
                false,
                foreign
            )
            .is_err()
    );
    let dead = address.pointer.allocation;
    memory.release_one(dead).unwrap();
    let next = memory
        .allocate(
            AddressSpace::Private,
            AccessMode::ReadWrite,
            4,
            vec![0; 4],
            vec![true; 4],
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(next > dead);
    assert!(matches!(memory.allocation(&address.pointer),
        Err(SimulationExecutionErrorKindV1::DanglingPointer { allocation }) if allocation == dead));
}

#[test]
fn storage_access_enforces_holder_space_alignment_and_pointer_rights() {
    let (memory, address) = memory(vec![0; 8], vec![true; 8], AddressSpace::Private);
    for (mut changed, access, write) in [
        (
            address.clone(),
            MemoryAccess::new(AddressSpace::Global, 4),
            false,
        ),
        (
            address.clone(),
            MemoryAccess::new(AddressSpace::Private, 8),
            false,
        ),
        (
            address.clone(),
            MemoryAccess::new(AddressSpace::Private, 4),
            true,
        ),
    ] {
        if write {
            changed.pointer.access = AccessMode::ReadOnly;
        }
        assert!(
            memory
                .storage_validate_v1(&changed, access, 4, write, invocation())
                .is_err()
        );
    }
    let mut out = address.clone();
    out.pointer.byte_offset = 6;
    assert!(
        memory
            .storage_validate_v1(
                &out,
                MemoryAccess::new(AddressSpace::Private, 1),
                4,
                false,
                invocation()
            )
            .is_err()
    );
    let mut write_only = address.clone();
    write_only.pointer.access = AccessMode::WriteOnly;
    assert!(
        memory
            .storage_validate_v1(
                &write_only,
                MemoryAccess::new(AddressSpace::Private, 4),
                4,
                false,
                invocation()
            )
            .is_err()
    );
}

#[test]
fn storage_pointer_niche_refuses_without_explicit_pointer_profile_or_tag_event() {
    let mut module = variant_module(false, false);
    module
        .storage_layouts
        .push(super::storage_values_tests_v1::pointer_row());
    let StorageLayoutKindV1::Variants { encoding, variants } = &mut module.storage_layouts[4].kind
    else {
        unreachable!()
    };
    *encoding = fe2o3_kernel_ir::StorageVariantEncodingV1::Niche {
        tag: StorageFieldV1 {
            offset: 0,
            layout: StorageLayoutIdV1(6),
        },
        untagged_variant: 0,
        first_niche_variant: 1,
        last_niche_variant: 1,
        niche_start: 0,
    };
    for (index, variant) in variants.iter_mut().enumerate() {
        variant.discriminant = index as u128;
        variant.direct_tag_bits = None;
    }
    let mut events = Events::default();
    assert!(matches!(
        simulate_storage_view_v1(verified(&module), &request(), None,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits64),
            SimulationLimitsV1::default(), &mut events),
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StorageViolation {
                reason: "pointer niche requires an explicit exact AMDGPU encoding profile",
            },
            ..
        }))
    ));
    assert!(
        !events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

#[test]
fn storage_nested_guard_keeps_ancestor_state_without_rereading_tags() {
    let (mut memory, address) = memory(vec![0; 12], vec![true; 12], AddressSpace::Private);
    let parent = memory.storage_register_guard_v1(&address, 0, 4).unwrap();
    let mut child = address.clone();
    child.pointer.storage_guard = Some(parent);
    let nested = memory.storage_register_guard_v1(&child, 4, 8).unwrap();
    child.pointer.storage_guard = Some(nested);
    let before = memory.storage_accounting.steps();
    assert!(memory.allocation(&child.pointer).is_ok());
    assert_eq!(memory.storage_accounting.steps() - before, 2);
    let mut raw = address.pointer.clone();
    raw.element = ScalarType::U32;
    memory
        .prepare_store(&raw, 4)
        .unwrap()
        .commit(ScalarBitsV1::u32(0));
    assert!(memory.allocation(&child.pointer).is_err());
}

pub(super) fn set_discriminant(address: u32, variant: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Storage(StorageOperationV1::SetDiscriminant {
            address: ValueId(address),
            variant,
            access: MemoryAccess::new(AddressSpace::Private, 4),
        }),
    )
}

fn construction(id: u32, row: u32, base: u32, step: StorageProjectionV1) -> Operation {
    value(
        id,
        Type::pointer(object(row), AddressSpace::Private, AccessMode::WriteOnly),
        OperationKind::Storage(StorageOperationV1::Project {
            base: ValueId(base),
            step,
        }),
    )
}

pub(super) fn construction_module(variant: u32, observe: bool) -> Module {
    let mut module = variant_module(false, true);
    let space = AddressSpace::Private;
    let mut operations = vec![
        allocate(1, 4, space),
        construction(
            2,
            3,
            1,
            StorageProjectionV1::VariantForWrite { index: variant },
        ),
        construction(3, 0, 2, StorageProjectionV1::Field(0)),
        constant(4, 99),
        write(3, 4, space),
        set_discriminant(1, variant),
    ];
    if observe {
        operations.extend([
            project(
                5,
                3,
                1,
                StorageProjectionV1::Variant {
                    index: variant,
                    access: MemoryAccess::new(space, 4),
                },
                space,
            ),
            project(6, 0, 5, StorageProjectionV1::Field(0), space),
            read(7, 6, space),
            output(7),
        ]);
    }
    module.functions[0].body.as_mut().unwrap().blocks[0].operations = operations;
    module
}

pub(super) fn niche(module: &mut Module, tag: StorageFieldV1, start: u128, last: u32) {
    let StorageLayoutKindV1::Variants { encoding, variants } = &mut module.storage_layouts[4].kind
    else {
        unreachable!()
    };
    *encoding = fe2o3_kernel_ir::StorageVariantEncodingV1::Niche {
        tag,
        untagged_variant: 0,
        first_niche_variant: 1,
        last_niche_variant: last,
        niche_start: start,
    };
    if last == 2 {
        let mut rows = variants.to_vec();
        let mut next = rows[1].clone();
        next.discriminant += 1;
        rows.push(next);
        *variants = rows.into_boxed_slice();
    }
    for (index, variant) in variants.iter_mut().enumerate() {
        variant.discriminant = index as u128;
        variant.direct_tag_bits = None;
    }
}

#[test]
fn storage_enum_construction_writes_payload_before_tag_and_reads_no_tag_early() {
    let mut events = Events::default();
    assert_eq!(
        output_bits(&run(&construction_module(0, true), &mut events).unwrap()),
        99
    );
    let allocation = events
        .0
        .iter()
        .find_map(|event| match event.kind {
            SimulationEventKindV1::AllocationCreated {
                allocation,
                address_space: AddressSpace::Private,
                ..
            } => Some(allocation),
            _ => None,
        })
        .unwrap();
    let accesses: Vec<_> = events
        .0
        .iter()
        .filter_map(|event| match event.kind {
            SimulationEventKindV1::MemoryRead {
                allocation: id,
                offset,
                bytes,
            } if id == allocation => Some((false, offset, bytes)),
            SimulationEventKindV1::MemoryWrite {
                allocation: id,
                offset,
                bytes,
            } if id == allocation => Some((true, offset, bytes)),
            _ => None,
        })
        .collect();
    assert_eq!(
        accesses,
        [(true, 4, 4), (true, 0, 4), (false, 0, 4), (false, 4, 4)]
    );
}

#[test]
fn storage_enum_same_discriminant_store_invalidates_existing_active_view() {
    let mut module = construction_module(0, true);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(8, set_discriminant(1, 0));
    let mut events = Events::default();
    assert!(run(&module, &mut events).is_err());
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(
                event.kind,
                SimulationEventKindV1::MemoryWrite {
                    offset: 0,
                    bytes: 4,
                    ..
                }
            ))
            .count(),
        2
    );
}

#[test]
fn storage_enum_partial_payload_remains_uninitialized_after_discriminant() {
    let mut module = construction_module(0, true);
    module.storage_layouts[3].size = 12;
    module.storage_layouts[3].kind = StorageLayoutKindV1::Record(
        vec![
            StorageFieldV1 {
                offset: 4,
                layout: StorageLayoutIdV1(0),
            },
            StorageFieldV1 {
                offset: 8,
                layout: StorageLayoutIdV1(0),
            },
        ]
        .into_boxed_slice(),
    );
    module.storage_layouts[4].size = 12;
    module.storage_layouts[5].size = 12;
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[7] = project(
        6,
        0,
        5,
        StorageProjectionV1::Field(1),
        AddressSpace::Private,
    );
    let mut events = Events::default();
    assert!(matches!(
        run(&module, &mut events),
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::UninitializedRead {
                offset: 8,
                bytes: 4,
                ..
            },
            ..
        }))
    ));
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
            .count(),
        1
    );
}

#[test]
fn storage_enum_niche_wraparound_and_raw_bool_tag_use_physical_width() {
    for boolean in [false, true] {
        let index = if boolean { 1 } else { 2 };
        let mut module = construction_module(index, true);
        let tag = if boolean {
            module.storage_layouts.push(StorageLayoutV1 {
                size: 1,
                alignment: 1,
                kind: StorageLayoutKindV1::Scalar(ScalarType::Bool),
            });
            StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(6),
            }
        } else {
            StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(0),
            }
        };
        niche(
            &mut module,
            tag,
            if boolean { 254 } else { u128::from(u32::MAX) },
            index,
        );
        if boolean {
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let OperationKind::Storage(StorageOperationV1::SetDiscriminant { access, .. }) =
                &mut operations[5].kind
            else {
                unreachable!()
            };
            access.alignment = 1;
            let OperationKind::Storage(StorageOperationV1::Project {
                step: StorageProjectionV1::Variant { access, .. },
                ..
            }) = &mut operations[6].kind
            else {
                unreachable!()
            };
            access.alignment = 1;
        }
        let mut events = Events::default();
        assert_eq!(output_bits(&run(&module, &mut events).unwrap()), 99);
        let width = if boolean { 1 } else { 4 };
        assert!(events.0.iter().any(|event| matches!(event.kind,
            SimulationEventKindV1::MemoryRead { offset: 0, bytes, .. } if bytes == width)));
    }
}

#[test]
fn storage_enum_untagged_noop_does_not_read_initialize_or_certify_tag() {
    let mut module = construction_module(0, false);
    niche(
        &mut module,
        StorageFieldV1 {
            offset: 0,
            layout: StorageLayoutIdV1(0),
        },
        0,
        1,
    );
    module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        allocate(1, 4, AddressSpace::Private),
        set_discriminant(1, 0),
    ];
    let mut events = Events::default();
    run(&module, &mut events).unwrap();
    assert!(!events.0.iter().any(|event| matches!(
        event.kind,
        SimulationEventKindV1::MemoryRead { .. } | SimulationEventKindV1::MemoryWrite { .. }
    )));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(project(
            2,
            3,
            1,
            StorageProjectionV1::Variant {
                index: 0,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
            AddressSpace::Private,
        ));
    let mut events = Events::default();
    assert!(matches!(
        run(&module, &mut events),
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::UninitializedRead { .. },
            ..
        }))
    ));
    assert!(
        !events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

#[test]
fn storage_enum_pointer_niche_store_is_real_but_unprofiled_active_decode_refuses() {
    let mut module = construction_module(1, true);
    module
        .storage_layouts
        .push(super::storage_values_tests_v1::pointer_row());
    niche(
        &mut module,
        StorageFieldV1 {
            offset: 0,
            layout: StorageLayoutIdV1(6),
        },
        0,
        1,
    );
    let mut events = Events::default();
    assert!(matches!(
        simulate_storage_view_v1(verified(&module), &request(), None,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits64),
            SimulationLimitsV1::default(), &mut events),
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StorageViolation {
                reason: "pointer niche requires an explicit exact AMDGPU encoding profile",
            },
            ..
        }))
    ));
    assert!(events.0.iter().any(|event| matches!(
        event.kind,
        SimulationEventKindV1::MemoryWrite {
            offset: 0,
            bytes: 4,
            ..
        }
    )));
    assert!(
        !events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

#[test]
fn storage_enum_construction_cannot_be_retyped_or_used_as_a_readable_pointer() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    };
    for retype in [false, true] {
        let mut module = construction_module(0, false);
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        if retype {
            operations[1].results[0].ty = address(3, AddressSpace::Private);
        } else {
            operations.push(read(8, 3, AddressSpace::Private));
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
        let checked = fe2o3_kernel_ir::check_module_storage_v1(
            &module,
            fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        )
        .unwrap();
        assert!(
            fe2o3_kernel_ir::verify_storage_module_ref_with_budget_v1(checked, None, &mut budget)
                .is_err()
        );
    }
}

#[test]
fn storage_enum_direct_signed_and_128_bit_tags_use_exact_physical_bits() {
    for wide in [false, true] {
        let mut module = construction_module(0, true);
        let width = if wide { 16 } else { 4 };
        module.storage_layouts.push(StorageLayoutV1 {
            size: width,
            alignment: width as u32,
            kind: StorageLayoutKindV1::Scalar(if wide {
                ScalarType::U128
            } else {
                ScalarType::I32
            }),
        });
        if wide {
            module.storage_layouts[3].size = 20;
            module.storage_layouts[3].kind = StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 16,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            );
            module.storage_layouts[4].size = 32;
            module.storage_layouts[4].alignment = 16;
            module.storage_layouts[5].size = 32;
            module.storage_layouts[5].alignment = 16;
        }
        let StorageLayoutKindV1::Variants { encoding, variants } =
            &mut module.storage_layouts[4].kind
        else {
            unreachable!()
        };
        *encoding = fe2o3_kernel_ir::StorageVariantEncodingV1::Direct {
            tag: StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(6),
            },
        };
        variants[0].discriminant = u128::MAX;
        variants[0].direct_tag_bits = Some(if wide {
            u128::MAX
        } else {
            u128::from(u32::MAX)
        });
        if wide {
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let OperationKind::Alloca { alignment, .. } = &mut operations[0].kind else {
                unreachable!()
            };
            *alignment = 16;
            let OperationKind::Storage(StorageOperationV1::SetDiscriminant { access, .. }) =
                &mut operations[5].kind
            else {
                unreachable!()
            };
            access.alignment = 16;
            let OperationKind::Storage(StorageOperationV1::Project {
                step: StorageProjectionV1::Variant { access, .. },
                ..
            }) = &mut operations[6].kind
            else {
                unreachable!()
            };
            access.alignment = 16;
        }
        let mut events = Events::default();
        assert_eq!(output_bits(&run(&module, &mut events).unwrap()), 99);
        assert!(events.0.iter().any(|event| matches!(event.kind,
            SimulationEventKindV1::MemoryWrite { offset: 0, bytes, .. } if bytes == width as usize)));
        assert!(
            events.0.iter().any(|event| matches!(event.kind,
            SimulationEventKindV1::MemoryRead { offset: 0, bytes, .. } if bytes == width as usize))
        );
    }
}
