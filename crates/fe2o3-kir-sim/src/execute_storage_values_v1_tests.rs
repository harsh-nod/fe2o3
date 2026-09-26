use super::storage_tests_v1::*;
use super::storage_views_tests_v1::{invocation, memory, wide_constant};
use super::*;

fn pointer_niche_program(value_space: AddressSpace, encoded_space: AddressSpace) -> Module {
    use fe2o3_kernel_ir::{ComparePredicate, StorageVariantEncodingV1 as Encoding, StorageVariantV1};
    let llvm_space = match encoded_space {
        AddressSpace::Generic => 0, AddressSpace::Global => 1, AddressSpace::Workgroup => 3,
        AddressSpace::Constant => 4, AddressSpace::Private => 5,
    };
    let recipe = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942.pointer_encoding(llvm_space).unwrap();
    let width = u64::from(recipe.bits() / 8);
    let field = |row| StorageFieldV1 { offset: 0, layout: StorageLayoutIdV1(row) };
    let rows = vec![
        StorageLayoutV1 { size: 4, alignment: 4, kind: StorageLayoutKindV1::Scalar(ScalarType::U32) },
        StorageLayoutV1 { size: width, alignment: width as u32,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 { pointee: StorageLayoutIdV1(0),
                value_space, encoded_space, access: AccessMode::ReadWrite, stored_bits: recipe.bits() }) },
        StorageLayoutV1 { size: width, alignment: width as u32,
            kind: StorageLayoutKindV1::Record(vec![field(1)].into_boxed_slice()) },
        StorageLayoutV1 { size: width, alignment: width as u32, kind: StorageLayoutKindV1::Variants {
            encoding: Encoding::Niche { tag: field(1), untagged_variant: 0, first_niche_variant: 1,
                last_niche_variant: 1, niche_start: recipe.null_bits() },
            variants: vec![
                StorageVariantV1 { discriminant: 0, direct_tag_bits: None,
                    uninhabited: false, layout: StorageLayoutIdV1(2) },
                StorageVariantV1 { discriminant: 1, direct_tag_bits: None,
                    uninhabited: false, layout: StorageLayoutIdV1(0) },
            ].into_boxed_slice(),
        } },
        StorageLayoutV1 { size: width, alignment: width as u32,
            kind: StorageLayoutKindV1::Union(vec![field(3), field(1)].into_boxed_slice()) },
    ];
    let space = AddressSpace::Private;
    let access = MemoryAccess::new(space, width as u32);
    let mut operations = vec![
        value(1, address(4, space), OperationKind::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(4)), count: None,
            address_space: space, alignment: width as u32,
        }),
        project(2, 3, 1, StorageProjectionV1::Field(0), space),
        project(3, 1, 1, StorageProjectionV1::Field(1), space),
    ];
    let input = if value_space == AddressSpace::Private {
        operations.push(value(4, Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite),
            OperationKind::Alloca { element: Type::Scalar(ScalarType::U32), count: None,
                address_space: space, alignment: 4 }));
        4
    } else {
        assert_eq!(value_space, AddressSpace::Global);
        0
    };
    operations.extend([
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(3), value: ValueId(input), access })),
        value(5, Type::Scalar(ScalarType::U128), OperationKind::Storage(StorageOperationV1::ReadDiscriminant {
            address: ValueId(2), access })),
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::SetDiscriminant {
            address: ValueId(2), variant: 1, access })),
        value(6, Type::Scalar(ScalarType::U128), OperationKind::Storage(StorageOperationV1::ReadDiscriminant {
            address: ValueId(2), access })),
    ]);
    operations.extend(wide_constant(7, 100, 0));
    operations.extend(wide_constant(8, 107, 1));
    operations.extend([
        value(9, Type::Scalar(ScalarType::Bool), OperationKind::Compare { predicate: ComparePredicate::Equal,
            lhs: ValueId(5), rhs: ValueId(7) }),
        value(10, Type::Scalar(ScalarType::Bool), OperationKind::Compare { predicate: ComparePredicate::Equal,
            lhs: ValueId(6), rhs: ValueId(8) }), constant(11, 1), constant(12, 0),
        value(13, Type::Scalar(ScalarType::U32), OperationKind::Select { condition: ValueId(10),
            true_value: ValueId(11), false_value: ValueId(12) }),
        value(14, Type::Scalar(ScalarType::U32), OperationKind::Select { condition: ValueId(9),
            true_value: ValueId(13), false_value: ValueId(12) }), output(14),
    ]);
    module(rows, operations)
}

#[test]
fn pointer_niche_uses_current_relocation_domain_then_actual_null_bytes() {
    for profile in [fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950] {
        for (value_space, encoded_space, width) in [
            (AddressSpace::Private, AddressSpace::Private, 4),
            (AddressSpace::Private, AddressSpace::Generic, 8),
            (AddressSpace::Global, AddressSpace::Global, 8),
            (AddressSpace::Global, AddressSpace::Generic, 8),
        ] {
            let module = pointer_niche_program(value_space, encoded_space);
            let mut events = Events::default();
            let result = simulate_storage_view_v1(verified(&module), &request(), None,
                SimulationTargetV1::amdgpu_profile(profile), SimulationLimitsV1::default(), &mut events).unwrap();
            assert_eq!(output_bits(&result), 1);
            let reads = events.0.iter().filter_map(|event| match event.kind {
                SimulationEventKindV1::MemoryRead { bytes, .. } => Some(bytes), _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(reads, [width, width]);
        }
    }
}

#[test]
fn pointer_niche_refuses_domains_crossing_variants_and_incompatible_relocations() {
    for ambiguous in [false, true] {
        let mut module = pointer_niche_program(AddressSpace::Private, AddressSpace::Generic);
        if ambiguous {
            let StorageLayoutKindV1::Variants { encoding, variants } = &mut module.storage_layouts[3].kind
                else { unreachable!() };
            let fe2o3_kernel_ir::StorageVariantEncodingV1::Niche { last_niche_variant, .. } = encoding else { unreachable!() };
            *last_niche_variant = 2;
            let mut expanded = variants.to_vec();
            let mut third = expanded[1].clone();
            third.discriminant = 2;
            expanded.push(third);
            *variants = expanded.into_boxed_slice();
        } else {
            module.storage_layouts.push(module.storage_layouts[1].clone());
            let StorageLayoutKindV1::Pointer(pointer) = &mut module.storage_layouts[1].kind else { unreachable!() };
            pointer.value_space = AddressSpace::Global;
            let StorageLayoutKindV1::Union(fields) = &mut module.storage_layouts[4].kind else { unreachable!() };
            fields[1].layout = StorageLayoutIdV1(5);
            module.functions[0].body.as_mut().unwrap().blocks[0].operations[2] =
                project(3, 5, 1, StorageProjectionV1::Field(1), AddressSpace::Private);
        }
        let mut events = Events::default();
        let result = run(&module, &mut events);
        let expected = if ambiguous { "symbolic pointer domain crosses possible logical variants" }
            else { "pointer tag has a partial or incompatible relocation" };
        assert!(matches!(result, Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StorageViolation { reason }, ..
        })) if reason == expected));
        assert!(!events.0.iter().any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. })));
    }
}

#[test]
fn discriminant_read_rejects_a_relocation_after_the_actual_callee_frame_expires() {
    let mut module = pointer_niche_program(AddressSpace::Private, AddressSpace::Generic);
    let space = AddressSpace::Private;
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.splice(3..5, [Operation::new(vec![], OperationKind::Call {
        callee: "store_local_pointer".into(), arguments: vec![ValueId(3)],
    })]);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        value(1, Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite),
            OperationKind::Alloca { element: Type::Scalar(ScalarType::U32), count: None,
                address_space: space, alignment: 4 }),
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(0), value: ValueId(1), access: MemoryAccess::new(space, 8),
        })),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::definition("store_local_pointer",
        fe2o3_kernel_ir::Signature::new(vec![address(1, space)], vec![]), vec![ValueId(0)], vec![block]));
    let mut events = Events::default();
    let Err(SimulationErrorV1::Execution(error)) = run(&module, &mut events) else {
        panic!("expired pointer tag must refuse at the caller read")
    };
    assert!(matches!(error.kind, SimulationExecutionErrorKindV1::DanglingPointer { .. }));
    let site = error.site.unwrap();
    assert_eq!(site.function, fe2o3_kernel_ir::FunctionId::from("entry"));
    assert_eq!(site.operation, Some(4));
    let write = events.0.iter().position(|event| matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }))
        .expect("the relocation was stored while its pointee was live");
    let released = events.0.iter().position(|event| matches!(event.kind, SimulationEventKindV1::AllocationReleased { .. }))
        .expect("the actual callee frame expired");
    assert!(write < released);
    assert!(!events.0.iter().any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. })));
}

#[test]
fn discriminant_read_does_not_decode_placeholder_bytes_after_partial_alias_overwrite() {
    let mut module = pointer_niche_program(AddressSpace::Private, AddressSpace::Generic);
    module.storage_layouts.push(StorageLayoutV1 { size: 1, alignment: 1,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U8) });
    let StorageLayoutKindV1::Union(fields) = &mut module.storage_layouts[4].kind else { unreachable!() };
    let mut aliases = fields.to_vec();
    aliases.push(StorageFieldV1 { offset: 0, layout: StorageLayoutIdV1(5) });
    *fields = aliases.into_boxed_slice();
    module.functions[0].body.as_mut().unwrap().blocks[0].operations.splice(5..5, [
        project(15, 5, 1, StorageProjectionV1::Field(2), AddressSpace::Private),
        value(16, Type::Scalar(ScalarType::U8), OperationKind::Constant(Constant::U8(7))),
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(15), value: ValueId(16), access: MemoryAccess::new(AddressSpace::Private, 1),
        })),
    ]);
    let mut events = Events::default();
    let Err(SimulationErrorV1::Execution(error)) = run(&module, &mut events) else {
        panic!("partially overwritten pointer tag must remain uninitialized")
    };
    assert!(matches!(error.kind, SimulationExecutionErrorKindV1::UninitializedRead { .. }));
    let site = error.site.unwrap();
    assert_eq!(site.function, fe2o3_kernel_ir::FunctionId::from("entry"));
    assert_eq!(site.operation, Some(8));
    assert_eq!(events.0.iter().filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. })).count(), 2);
    assert!(!events.0.iter().any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. })));
}

#[test]
fn pointer_encoding_domains_require_explicit_profile_width_and_representation_spaces() {
    for (space, width, null) in [(AddressSpace::Private, 32, u32::MAX as u128),
        (AddressSpace::Workgroup, 32, u32::MAX as u128), (AddressSpace::Global, 64, 0),
        (AddressSpace::Constant, 64, 0), (AddressSpace::Generic, 64, 0)] {
        let pointer = StoragePointerV1 { pointee: StorageLayoutIdV1(0), value_space: space,
            encoded_space: space, access: AccessMode::ReadOnly, stored_bits: width };
        assert!(SimulationTargetV1::little_endian(IndexWidthV1::Bits64)
            .storage_pointer_encoding(pointer).is_none());
        for profile in [fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950] {
            let target = SimulationTargetV1::amdgpu_profile(profile);
            let encoding = target.storage_pointer_encoding(pointer).unwrap();
            assert_eq!((encoding.bits(), encoding.null_bits()), (width, null));
            assert!(!encoding.nonnull_domain_contains(null));
            assert!(encoding.nonnull_domain_contains(if null == 0 { 1 } else { 0 }));
            assert!(target.storage_pointer_encoding(StoragePointerV1 {
                stored_bits: if width == 32 { 64 } else { 32 }, ..pointer
            }).is_none());
            let generic = target.storage_pointer_encoding(StoragePointerV1 {
                encoded_space: AddressSpace::Generic, stored_bits: 64, ..pointer
            }).unwrap();
            assert_eq!((generic.bits(), generic.null_bits()), (64, 0));
            if space != AddressSpace::Global {
                assert!(target.storage_pointer_encoding(StoragePointerV1 {
                    encoded_space: AddressSpace::Global, stored_bits: 64, ..pointer
                }).is_none());
            }
        }
    }
}

pub(super) fn pointer_row() -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(0),
            value_space: AddressSpace::Private,
            encoded_space: AddressSpace::Private,
            access: AccessMode::ReadWrite,
            stored_bits: 32,
        }),
    }
}

pub(super) fn pointer_program(overwrite: bool) -> Module {
    let mut layouts = rows();
    layouts.push(pointer_row());
    layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Union(
            vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(3),
                },
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                },
            ]
            .into_boxed_slice(),
        ),
    });
    let space = AddressSpace::Private;
    let scalar_pointer = Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite);
    let mut operations = vec![
        value(
            1,
            scalar_pointer.clone(),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: space,
                alignment: 4,
            },
        ),
        constant(2, 77),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(2),
                access: MemoryAccess::new(space, 4),
            },
        ),
        allocate(3, 4, space),
        project(4, 3, 3, StorageProjectionV1::Field(0), space),
        write(4, 1, space),
    ];
    if overwrite {
        operations.extend([
            project(7, 0, 3, StorageProjectionV1::Field(1), space),
            write(7, 2, space),
        ]);
    }
    operations.extend([
        value(
            5,
            scalar_pointer,
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(4),
                access: MemoryAccess::new(space, 4),
            }),
        ),
        value(
            6,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(5),
                access: MemoryAccess::new(space, 4),
            },
        ),
        output(6),
    ]);
    module(layouts, operations)
}

#[test]
fn storage_symbolic_pointer_roundtrip_preserves_real_allocation_and_legacy_load() {
    let mut events = Events::default();
    let result = run(&pointer_program(false), &mut events).unwrap();
    assert_eq!(output_bits(&result), 77);
    let allocations: Vec<_> = events
        .0
        .iter()
        .filter_map(|e| match e.kind {
            SimulationEventKindV1::AllocationCreated {
                allocation,
                address_space: AddressSpace::Private,
                ..
            } => Some(allocation),
            _ => None,
        })
        .collect();
    assert_eq!(allocations.len(), 2);
    let reads: Vec<_> = events
        .0
        .iter()
        .filter_map(|e| match e.kind {
            SimulationEventKindV1::MemoryRead { allocation, .. } => Some(allocation),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [allocations[1], allocations[0]]);
}

#[test]
fn storage_actual_scalar_overwrite_removes_pointer_relocation_not_its_target() {
    let mut events = Events::default();
    assert!(run(&pointer_program(true), &mut events).is_err());
    assert!(
        !events
            .0
            .iter()
            .any(|e| matches!(e.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

#[test]
fn storage_partial_raw_write_invalidates_entire_symbolic_representation() {
    let (mut memory, address) = memory(vec![0; 8], vec![true; 8], AddressSpace::Private);
    let StorageLayoutKindV1::Pointer(representation) = pointer_row().kind else {
        unreachable!()
    };
    let mut target = address.pointer.clone();
    target.element = ScalarType::U32;
    target.byte_offset = 4;
    target.lower_bound = 4;
    let relocation = StorageRelocationV1 {
        start: 0,
        end: 4,
        representation,
        value: StoragePointerPayloadV1::Scalar(target),
    };
    let allocation = memory
        .allocations
        .get_mut(&address.pointer.allocation)
        .unwrap();
    storage_reserve_v1(
        &mut allocation.storage.relocations,
        1,
        &memory.storage_accounting,
    )
    .unwrap();
    allocation.storage.relocations.push(relocation);
    allocation.storage.relocation_bytes = 4;
    assert!(
        allocation
            .storage
            .raw_read(0, 4, &memory.storage_accounting)
            .is_err()
    );
    let mut raw = address.pointer.clone();
    raw.byte_offset = 1;
    raw.element = ScalarType::U8;
    memory
        .prepare_store(&raw, 1)
        .unwrap()
        .commit(ScalarBitsV1::new(ScalarType::U8, 1, SimulationTargetV1::amdgpu_64()).unwrap());
    let allocation = &memory.allocations[&raw.allocation];
    assert!(allocation.storage.relocations.is_empty());
    assert_eq!(allocation.storage.relocation_bytes, 0);
    assert_eq!(&allocation.initialized[..4], &[false, true, false, false]);
    assert!(
        memory
            .storage_relocation_v1(&address, representation, invocation())
            .is_err()
    );
}

#[test]
fn storage_pointer_bytes_are_never_debugged_or_exported_as_integer_addresses() {
    let (mut memory, address) = memory(vec![0; 4], vec![true; 4], AddressSpace::Global);
    let StorageLayoutKindV1::Pointer(representation) = pointer_row().kind else {
        unreachable!()
    };
    let allocation = memory
        .allocations
        .get_mut(&address.pointer.allocation)
        .unwrap();
    storage_reserve_v1(
        &mut allocation.storage.relocations,
        1,
        &memory.storage_accounting,
    )
    .unwrap();
    allocation.storage.relocations.push(StorageRelocationV1 {
        start: 0,
        end: 4,
        representation,
        value: StoragePointerPayloadV1::Object(address.clone()),
    });
    allocation.storage.relocation_bytes = 4;
    memory
        .argument_allocations
        .push(Some(address.pointer.allocation));
    assert!(copy_back_arguments(&memory, &request().arguments).is_err());
    assert!(matches!(
        capture_debug_memory(&memory, SimulationDebugCaptureLimitsV1::disabled()),
        SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::NotCaptured,
            ..
        }
    ));
    assert_eq!(debug_value(&RuntimeValue::StoragePointer(address)), None);
}

#[test]
fn storage_relocation_checks_actual_backing_space_rights_and_constant_restriction() {
    let (mut memory, address) = memory(vec![0; 4], vec![true; 4], AddressSpace::Private);
    let StorageLayoutKindV1::Pointer(mut representation) = pointer_row().kind else {
        unreachable!()
    };
    let mut value = address.pointer.clone();
    value.element = ScalarType::U32;
    memory
        .storage_validate_pointer_v1(
            &StoragePointerPayloadV1::Scalar(value.clone()),
            representation,
            invocation(),
        )
        .unwrap();
    memory
        .allocations
        .get_mut(&value.allocation)
        .unwrap()
        .access = AccessMode::ReadOnly;
    assert!(
        memory
            .storage_validate_pointer_v1(
                &StoragePointerPayloadV1::Scalar(value.clone()),
                representation,
                invocation()
            )
            .is_err()
    );
    representation.access = AccessMode::ReadOnly;
    value.access = AccessMode::ReadOnly;
    memory
        .storage_validate_pointer_v1(
            &StoragePointerPayloadV1::Scalar(value.clone()),
            representation,
            invocation(),
        )
        .unwrap();
    representation.value_space = AddressSpace::Constant;
    representation.encoded_space = AddressSpace::Constant;
    representation.stored_bits = 64;
    value.address_space = AddressSpace::Constant;
    assert!(
        memory
            .storage_validate_pointer_v1(
                &StoragePointerPayloadV1::Scalar(value.clone()),
                representation,
                invocation()
            )
            .is_err()
    );
    let allocation = memory.allocations.get_mut(&value.allocation).unwrap();
    allocation.address_space = AddressSpace::Constant;
    allocation.access = AccessMode::ReadWrite;
    representation.access = AccessMode::WriteOnly;
    value.access = AccessMode::WriteOnly;
    assert!(
        memory
            .storage_validate_pointer_v1(
                &StoragePointerPayloadV1::Scalar(value),
                representation,
                invocation()
            )
            .is_err()
    );
}

#[test]
fn storage_slice_descriptor_roundtrip_keeps_symbolic_data_and_actual_index_length() {
    let mut layouts = rows();
    layouts.extend([
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: StorageLayoutIdV1(0),
                value_space: AddressSpace::Global,
                encoded_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
                stored_bits: 64,
            }),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Scalar(ScalarType::Index),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Slice {
                element: StorageLayoutIdV1(0),
                value_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
                data: StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(3),
                },
                length: StorageFieldV1 {
                    offset: 8,
                    layout: StorageLayoutIdV1(4),
                },
            },
        },
    ]);
    let slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    let mut module = module(
        layouts,
        vec![
            value(
                2,
                address(5, AddressSpace::Private),
                OperationKind::Alloca {
                    element: object(5),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 8,
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Storage(StorageOperationV1::WriteValue {
                    address: ValueId(2),
                    value: ValueId(1),
                    access,
                }),
            ),
            value(
                3,
                slice.clone(),
                OperationKind::Storage(StorageOperationV1::ReadValue {
                    address: ValueId(2),
                    access,
                }),
            ),
            value(
                4,
                Type::INDEX,
                OperationKind::SliceLength { slice: ValueId(3) },
            ),
            value(
                5,
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                OperationKind::SliceData { slice: ValueId(3) },
            ),
            value(
                6,
                Type::Scalar(ScalarType::U32),
                OperationKind::Load {
                    pointer: ValueId(5),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            output(6),
        ],
    );
    module.functions[0].signature.parameters.push(slice);
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(1));
    let target = SimulationTargetV1::amdgpu_64();
    let mut request = request();
    request.arguments.push(SimulationArgumentV1::Buffer(
        BufferArgumentV1::from_scalars(
            AccessMode::ReadOnly,
            4,
            &[ScalarBitsV1::u32(42), ScalarBitsV1::u32(13)],
            target,
        )
        .unwrap(),
    ));
    let mut events = Events::default();
    let result = simulate_storage_view_v1(
        verified(&module),
        &request,
        None,
        target,
        SimulationLimitsV1::default(),
        &mut events,
    )
    .unwrap();
    assert_eq!(output_bits(&result), 42);
    let reads: Vec<_> = events
        .0
        .iter()
        .filter_map(|e| match e.kind {
            SimulationEventKindV1::MemoryRead { bytes, .. } => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [16, 4]);
}

#[test]
fn storage_vector_read_write_roundtrip_uses_shared_memory_and_finite_lane_arena() {
    use fe2o3_kernel_ir::{FixedVectorTypeV12, VectorLayoutV12};
    for (layout, alignment) in [
        (VectorLayoutV12::Contiguous, 4),
        (VectorLayoutV12::Contiguous, 32),
        (VectorLayoutV12::Interleaved { factor: 2 }, 4),
    ] {
        let vector = FixedVectorTypeV12::new(ScalarType::U32, 4, layout);
        let extent = u64::from(alignment.max(16));
        let layouts = vec![
            scalar_row(),
            StorageLayoutV1 {
                size: 16,
                alignment: 4,
                kind: StorageLayoutKindV1::Array {
                    element: StorageLayoutIdV1(0),
                    length: 4,
                    stride: 4,
                },
            },
            StorageLayoutV1 {
                size: extent,
                alignment,
                kind: StorageLayoutKindV1::Vector(vector),
            },
            StorageLayoutV1 {
                size: extent,
                alignment,
                kind: StorageLayoutKindV1::Union(
                    vec![
                        StorageFieldV1 {
                            offset: 0,
                            layout: StorageLayoutIdV1(1),
                        },
                        StorageFieldV1 {
                            offset: 0,
                            layout: StorageLayoutIdV1(2),
                        },
                    ]
                    .into_boxed_slice(),
                ),
            },
        ];
        let space = AddressSpace::Private;
        let aligned = |id| {
            value(
                id,
                address(3, space),
                OperationKind::Alloca {
                    element: object(3),
                    count: None,
                    address_space: space,
                    alignment,
                },
            )
        };
        let mut operations = vec![
            aligned(1),
            aligned(2),
            project(3, 1, 1, StorageProjectionV1::Field(0), space),
            project(4, 2, 1, StorageProjectionV1::Field(1), space),
            project(5, 2, 2, StorageProjectionV1::Field(1), space),
            project(6, 1, 2, StorageProjectionV1::Field(0), space),
        ];
        for index in 0..4_u32 {
            let id = 7 + index * 3;
            operations.extend([
                value(
                    id,
                    Type::INDEX,
                    OperationKind::Constant(Constant::Index(u64::from(index))),
                ),
                project(
                    id + 1,
                    0,
                    3,
                    StorageProjectionV1::ArrayIndex(ValueId(id)),
                    space,
                ),
                constant(id + 2, 101 + index),
                write(id + 1, id + 2, space),
            ]);
        }
        operations.extend([
            value(
                19,
                Type::Vector(vector),
                OperationKind::Storage(StorageOperationV1::ReadValue {
                    address: ValueId(4),
                    access: MemoryAccess::new(space, 4),
                }),
            ),
            write(5, 19, space),
            project(
                20,
                0,
                6,
                StorageProjectionV1::ArrayIndex(ValueId(13)),
                space,
            ),
            read(21, 20, space),
            output(21),
        ]);
        assert_eq!(
            output_bits(&run(&module(layouts, operations), &mut Events::default()).unwrap()),
            103
        );
    }
}

#[test]
fn storage_enum_untagged_preserves_relocation_and_real_tag_writes_invalidate_it() {
    use super::storage_views_tests_v1::set_discriminant;
    use fe2o3_kernel_ir::{StorageVariantEncodingV1, StorageVariantV1};
    for case in 0..3 {
        let mut module = pointer_program(false);
        module.storage_layouts.push(StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Niche {
                    tag: StorageFieldV1 {
                        offset: if case == 2 { 2 } else { 0 },
                        layout: StorageLayoutIdV1(if case == 2 { 6 } else { 3 }),
                    },
                    untagged_variant: 0,
                    first_niche_variant: 1,
                    last_niche_variant: 1,
                    niche_start: 1,
                },
                variants: [0, 1]
                    .into_iter()
                    .map(|discriminant| StorageVariantV1 {
                        discriminant,
                        direct_tag_bits: None,
                        uninhabited: false,
                        layout: StorageLayoutIdV1(3),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
        });
        if case == 2 {
            module.storage_layouts.push(StorageLayoutV1 {
                size: 2,
                alignment: 2,
                kind: StorageLayoutKindV1::Scalar(ScalarType::U16),
            });
        }
        let StorageLayoutKindV1::Union(fields) = &mut module.storage_layouts[4].kind else {
            unreachable!()
        };
        let mut extended = fields.to_vec();
        extended.push(StorageFieldV1 {
            offset: 0,
            layout: StorageLayoutIdV1(5),
        });
        *fields = extended.into_boxed_slice();
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        operations.insert(
            6,
            project(
                9,
                5,
                3,
                StorageProjectionV1::Field(2),
                AddressSpace::Private,
            ),
        );
        let mut set = set_discriminant(9, u32::from(case != 0));
        if case == 2 {
            let OperationKind::Storage(StorageOperationV1::SetDiscriminant { access, .. }) =
                &mut set.kind
            else {
                unreachable!()
            };
            access.alignment = 2;
        }
        operations.insert(7, set);
        if case == 2 {
            operations[8] = project(
                5,
                0,
                3,
                StorageProjectionV1::Field(1),
                AddressSpace::Private,
            );
            operations[9] = read(6, 5, AddressSpace::Private);
        }
        let mut events = Events::default();
        let result = run(&module, &mut events);
        match case {
            0 => assert_eq!(output_bits(&result.unwrap()), 77),
            1 => assert!(matches!(
                result,
                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::StorageViolation {
                        reason: "pointer representation lacks a complete relocation",
                    },
                    ..
                }))
            )),
            _ => assert!(matches!(
                result,
                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::UninitializedRead {
                        offset: 0,
                        bytes: 4,
                        ..
                    },
                    ..
                }))
            )),
        }
        let tag_events: Vec<_> = events
            .0
            .iter()
            .filter(|event| event.site.operation == Some(7))
            .filter_map(|event| match event.kind {
                SimulationEventKindV1::MemoryWrite { offset, bytes, .. } => Some((offset, bytes)),
                SimulationEventKindV1::MemoryRead { .. } => panic!("SetDiscriminant read a tag"),
                _ => None,
            })
            .collect();
        assert_eq!(
            tag_events,
            match case {
                0 => vec![],
                1 => vec![(0, 4)],
                _ => vec![(2, 2)],
            }
        );
    }
}

#[test]
fn storage_enum_construction_retains_enclosing_active_guard_and_checks_its_invalidation() {
    use super::storage_views_tests_v1::{set_discriminant, variant_module};
    use fe2o3_kernel_ir::{StorageVariantEncodingV1, StorageVariantV1};
    for stale in [false, true] {
        let mut module = variant_module(false, true);
        module.storage_layouts.push(StorageLayoutV1 {
            size: 12,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 4,
                    layout: StorageLayoutIdV1(4),
                }]
                .into_boxed_slice(),
            ),
        });
        module.storage_layouts.push(StorageLayoutV1 {
            size: 12,
            alignment: 4,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Direct {
                    tag: StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                },
                variants: vec![StorageVariantV1 {
                    discriminant: 10,
                    direct_tag_bits: Some(10),
                    uninhabited: false,
                    layout: StorageLayoutIdV1(6),
                }]
                .into_boxed_slice(),
            },
        });
        let space = AddressSpace::Private;
        let write_view = |id, row, base, step| {
            value(
                id,
                Type::pointer(object(row), space, AccessMode::WriteOnly),
                OperationKind::Storage(StorageOperationV1::Project {
                    base: ValueId(base),
                    step,
                }),
            )
        };
        let mut operations = vec![
            allocate(1, 7, space),
            set_discriminant(1, 0),
            project(
                2,
                6,
                1,
                StorageProjectionV1::Variant {
                    index: 0,
                    access: MemoryAccess::new(space, 4),
                },
                space,
            ),
            project(3, 4, 2, StorageProjectionV1::Field(0), space),
            write_view(4, 3, 3, StorageProjectionV1::VariantForWrite { index: 0 }),
            write_view(5, 0, 4, StorageProjectionV1::Field(0)),
            constant(6, 55),
            write(5, 6, space),
            set_discriminant(3, 0),
        ];
        if stale {
            operations.extend([set_discriminant(1, 0), write(5, 6, space)]);
        }
        operations.extend([
            project(
                7,
                3,
                3,
                StorageProjectionV1::Variant {
                    index: 0,
                    access: MemoryAccess::new(space, 4),
                },
                space,
            ),
            project(8, 0, 7, StorageProjectionV1::Field(0), space),
            read(9, 8, space),
            output(9),
        ]);
        module.functions[0].body.as_mut().unwrap().blocks[0].operations = operations;
        let mut events = Events::default();
        let result = run(&module, &mut events);
        if stale {
            assert!(result.is_err());
        } else {
            assert_eq!(output_bits(&result.unwrap()), 55);
        }
        let reads: Vec<_> = events
            .0
            .iter()
            .filter_map(|event| match event.kind {
                SimulationEventKindV1::MemoryRead { offset, bytes, .. } => Some((offset, bytes)),
                _ => None,
            })
            .collect();
        assert_eq!(
            reads,
            if stale {
                vec![(0, 4)]
            } else {
                vec![(0, 4), (4, 4), (8, 4)]
            }
        );
    }
}
