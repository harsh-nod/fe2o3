include!("production_execution_slice_holder_v29_tests.rs");

fn cfg_index_test_carrier_v29(local: u32) -> ExecutionCfgCarrierV29 {
    // Inert table data only. Ordinary cannot pass the carrier authority checks.
    ExecutionCfgCarrierV29 {
        source_type: SemanticTypeIdV1::from_index(local),
        transport_type: SemanticTypeIdV1::from_index(local),
        binding: SemanticPromotedBindingV1::Ordinary,
        kernel_types: Vec::new().into_boxed_slice(),
    }
}

fn cfg_index_test_headers_v29() -> usize {
    std::mem::size_of::<ExecutionCfgCarriersV29>()
        + crate::production_semantic_kir_v1::instance_correspondence_tests::cfg_carrier_index_storage_v29()
}

#[test]
fn cfg_carrier_sorted_index_has_independent_headers_and_first_capacity_cuts() {
    let headers = cfg_index_test_headers_v29();
    assert_eq!(
        headers,
        std::mem::size_of::<ExecutionCfgCarriersV29>()
            + execution_cfg_carrier_index_headers_v29().unwrap()
    );
    let capacity = 4 * std::mem::size_of::<(u32, ExecutionCfgCarrierV29)>();
    for (limit, succeeds) in [
        (headers - 1, false),
        (headers, false),
        (headers + capacity - 1, false),
        (headers + capacity, true),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let mut table = ExecutionCfgCarriersV29::default();
        let result = budget
            .reserve_storage(headers)
            .map_err(ProductionSemanticKirErrorV1::from)
            .and_then(|()| table.append(7, cfg_index_test_carrier_v29(7), &mut budget));
        if succeeds {
            result.unwrap();
            assert_eq!(table.locals.capacity(), 4);
            assert_eq!(
                table
                    .lookup(7, &mut budget)
                    .unwrap()
                    .unwrap()
                    .source_type
                    .index(),
                7
            );
            assert_eq!(budget.storage(), headers + capacity);
        } else {
            let expected = if limit < headers {
                headers
            } else {
                headers + capacity
            };
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error))) if error.actual() == expected && error.limit() == limit)
            );
            assert!(table.locals.is_empty());
            assert_eq!(
                table.locals.capacity(),
                0,
                "first debit precedes allocation"
            );
            assert_eq!(budget.storage(), if limit < headers { 0 } else { headers });
        }
        drop(table);
        budget.release_storage(budget.storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn cfg_carrier_sorted_index_empty_singleton_growth_and_misses_keep_exact_rows() {
    let headers = cfg_index_test_headers_v29();
    let row_bytes = std::mem::size_of::<(u32, ExecutionCfgCarrierV29)>();
    for count in [0, 1, 4, 16, 128] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(headers).unwrap();
        let mut table = ExecutionCfgCarriersV29::default();
        for index in 0..count {
            let local = index * 2 + 1;
            table
                .append(local, cfg_index_test_carrier_v29(local), &mut budget)
                .unwrap();
            assert_eq!(
                budget.storage(),
                headers + table.locals.capacity() * row_bytes
            );
        }
        assert_eq!(table.locals.len(), count as usize);
        let expected_work = (count.checked_ilog2().unwrap_or(0) as usize + 2) * 16;
        let before_storage = (budget.storage(), budget.peak_storage());
        for local in (0..=count * 2).chain([u32::MAX]) {
            let before_work = budget.work();
            let found = table.lookup(local, &mut budget).unwrap();
            let expected = local % 2 == 1 && local / 2 < count;
            assert_eq!(found.is_some(), expected);
            if let Some(carrier) = found {
                assert_eq!(
                    (carrier.source_type.index(), carrier.transport_type.index()),
                    (local, local)
                );
            }
            assert_eq!(budget.work() - before_work, expected_work);
            assert_eq!((budget.storage(), budget.peak_storage()), before_storage);
        }
        drop(table);
        budget.release_storage(budget.storage()).unwrap();
    }
}

#[test]
fn cfg_carrier_sorted_index_refuses_duplicate_and_descending_insertion() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    budget
        .reserve_storage(cfg_index_test_headers_v29())
        .unwrap();
    let mut table = ExecutionCfgCarriersV29::default();
    for local in [2, 5] {
        table
            .append(local, cfg_index_test_carrier_v29(local), &mut budget)
            .unwrap();
    }
    let storage = (budget.storage(), budget.peak_storage());
    for local in [5, 1] {
        assert!(matches!(
            table.append(local, cfg_index_test_carrier_v29(99), &mut budget),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(
            table.locals.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            [2, 5]
        );
        assert_eq!((budget.storage(), budget.peak_storage()), storage);
        assert_eq!(
            table
                .lookup(5, &mut budget)
                .unwrap()
                .unwrap()
                .source_type
                .index(),
            5
        );
    }
    drop(table);
    budget.release_storage(budget.storage()).unwrap();
}

#[test]
fn cfg_carrier_sorted_index_prepays_lookup_and_replacement_before_mutation() {
    let headers = cfg_index_test_headers_v29();
    let row_bytes = std::mem::size_of::<(u32, ExecutionCfgCarrierV29)>();
    // Eight replacement rows coexist with the four old rows until append.
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let limit = headers + 12 * row_bytes - usize::from(short);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(headers).unwrap();
        let mut table = ExecutionCfgCarriersV29::default();
        for local in 0..4 {
            table
                .append(local, cfg_index_test_carrier_v29(local), &mut budget)
                .unwrap();
        }
        let result = table.append(4, cfg_index_test_carrier_v29(4), &mut budget);
        if short {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error))) if error.actual() == headers + 12 * row_bytes && error.limit() == limit)
            );
            assert_eq!((table.locals.len(), table.locals.capacity()), (4, 4));
            assert_eq!(budget.storage(), headers + 4 * row_bytes);
        } else {
            result.unwrap();
            assert_eq!((table.locals.len(), table.locals.capacity()), (5, 8));
            assert_eq!(budget.storage(), headers + 8 * row_bytes);
            assert_eq!(budget.peak_storage(), limit);
        }
        drop(table);
        budget.release_storage(budget.storage()).unwrap();
    }
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(headers).unwrap();
        let mut table = ExecutionCfgCarriersV29::default();
        table
            .append(1, cfg_index_test_carrier_v29(1), &mut budget)
            .unwrap();
        let expected_work = 2 * 16;
        budget
            .charge_work(1000 - budget.work() - expected_work + usize::from(short))
            .unwrap();
        let storage = (budget.storage(), budget.peak_storage());
        let result = table.lookup(1, &mut budget);
        if short {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error))) if error.actual() == 1001 && error.limit() == 1000)
            );
        } else {
            assert_eq!(result.unwrap().unwrap().source_type.index(), 1);
            assert_eq!(budget.work(), 1000);
        }
        assert_eq!((budget.storage(), budget.peak_storage()), storage);
        drop(table);
        budget.release_storage(budget.storage()).unwrap();
    }
}

#[test]
fn cfg_carrier_scratch_rebuild_preserves_nested_source_shape() {
    let scalar = SemanticTypeIdV1::from_index(0);
    let pointer = SemanticTypeIdV1::from_index(1);
    let inner = SemanticTypeIdV1::from_index(2);
    let scratch = SemanticTypeIdV1::from_index(3);
    let aggregate = |tag, fields, offsets, size| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                8,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        )
    };
    let types = [
        plain_bit_scalar_type(
            10,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([12; 32]),
            SemanticLayoutIdentityV1::from_sha256([13; 32]),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    scalar,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
        aggregate(14, vec![pointer, scalar], vec![0, 8], 16),
        aggregate(16, vec![inner], vec![0], 16),
    ];
    let values = [
        ValueDef::new(
            ValueId(31),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            ),
        ),
        ValueDef::new(ValueId(32), Type::Scalar(ScalarType::U32)),
    ];
    check_paid_compiler_carrier_construction(
        SemanticPromotedBindingV1::WorkgroupCollectiveScratch { element: scalar },
        &types,
        scratch,
        &values,
    );
}

#[test]
fn cfg_carrier_plain_route_does_not_admit_reference_or_storage_nodes() {
    let plain = SourceReferenceNodeV29 {
        ty: SemanticTypeIdV1::from_index(0),
        kind: SourceReferenceNodeKindV29::Plain(None),
        value_origin: None,
        storage: None,
        inactive: None,
        descriptor: None,
        atomic_custody: None,
    };
    assert!(execution_cfg_plain_carrier_node_v29(&plain));
    assert!(!execution_cfg_plain_carrier_node_v29(
        &SourceReferenceNodeV29 {
            atomic_custody: Some(0),
            ..plain
        }
    ));
    for kind in [
        SourceReferenceNodeKindV29::Absent,
        SourceReferenceNodeKindV29::Plain(Some(SourceReferenceAnchorV29 {
            argument: 0,
            ty: plain.ty,
        })),
        SourceReferenceNodeKindV29::Loan(0),
        SourceReferenceNodeKindV29::Address(0),
        SourceReferenceNodeKindV29::Aggregate { first: 0, count: 0 },
        SourceReferenceNodeKindV29::Enum { first: 0, count: 0 },
        SourceReferenceNodeKindV29::EnumView(0),
        SourceReferenceNodeKindV29::Discriminant(0),
    ] {
        assert!(!execution_cfg_plain_carrier_node_v29(
            &SourceReferenceNodeV29 { kind, ..plain }
        ));
    }
    for field in 0..4 {
        let mut row = plain;
        match field {
            0 => row.value_origin = Some(0),
            1 => {
                row.storage = Some(SourceReferenceValueStorageV29 {
                    snapshot: 0,
                    first: 0,
                    count: 0,
                    selector_source: None,
                })
            }
            2 => row.inactive = Some(SourceReferenceInactiveShapeV29 { first: 0, count: 0 }),
            _ => row.descriptor = Some(0),
        }
        assert!(!execution_cfg_plain_carrier_node_v29(&row));
    }
}

fn cfg_option_pointer_equation_fixture() -> (ExecutionCfgCarrierV29, SemanticValueBindingV1) {
    let descriptor = SemanticPromotedBindingV1::OptionPointer {
        element: ScalarType::U32,
        address_space: AddressSpace::Global,
        access: AccessMode::ReadWrite,
        availability: test_option_availability_v1(),
    };
    let ty = SemanticTypeIdV1::from_index(0);
    let carrier = ExecutionCfgCarrierV29 {
        source_type: ty,
        transport_type: ty,
        binding: descriptor,
        kernel_types: vec![
            Type::BOOL,
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
        ]
        .into_boxed_slice(),
    };
    let values = [
        ValueDef::new(ValueId(11), carrier.kernel_types[0].clone()),
        ValueDef::new(ValueId(13), carrier.kernel_types[1].clone()),
    ];
    let binding = descriptor.binding_from_transport(&[], ty, &values).unwrap();
    (carrier, binding)
}

#[test]
fn source_pointer_holder_identity_equations_preserve_all_pointer_components() {
    // Inert identity equations, not pointer provenance or memory admission.
    let semantic = SemanticTypeIdV1::from_index(0);
    for space in [
        AddressSpace::Generic,
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Workgroup,
    ] {
        for access in [
            AccessMode::ReadOnly,
            AccessMode::ReadWrite,
            AccessMode::WriteOnly,
        ] {
            for nested in [false, true] {
                let pointee = if nested {
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::ReadOnly,
                    )
                } else {
                    Type::Scalar(ScalarType::U32)
                };
                let original = SemanticValueBindingV1::Value {
                    id: ValueId(61),
                    ty: Type::pointer(pointee, space, access),
                };
                for mutation in 0..5 {
                    let mut candidate = original.clone();
                    let SemanticValueBindingV1::Value { id, ty } = &mut candidate else {
                        unreachable!()
                    };
                    match mutation {
                        0 => {}
                        1 => *id = ValueId(62),
                        2 => {
                            let Type::Pointer(pointer) = ty else {
                                unreachable!()
                            };
                            pointer.address_space = if space == AddressSpace::Generic {
                                AddressSpace::Global
                            } else {
                                AddressSpace::Generic
                            };
                        }
                        3 => {
                            let Type::Pointer(pointer) = ty else {
                                unreachable!()
                            };
                            pointer.access = if access == AccessMode::ReadOnly {
                                AccessMode::ReadWrite
                            } else {
                                AccessMode::ReadOnly
                            };
                        }
                        _ => {
                            let Type::Pointer(pointer) = ty else {
                                unreachable!()
                            };
                            pointer.pointee = Box::new(Type::Scalar(ScalarType::U64));
                        }
                    }
                    let left = SemanticValueBindingV1::Aggregate(vec![
                        SemanticValueBindingV1::Unit,
                        candidate,
                    ]);
                    let right = SemanticValueBindingV1::Aggregate(vec![
                        SemanticValueBindingV1::Unit,
                        original.clone(),
                    ]);
                    let projections = [
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), semantic)
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, semantic)
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(99), semantic)
                            .unwrap(),
                    ];
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
                    let mut budget = ArgumentBudgetV1::new(&mut work, 19);
                    budget.reserve_storage(19).unwrap();
                    assert_eq!(
                        execution_archive_pointer_holder_same_v29(
                            &left,
                            &right,
                            &projections,
                            &mut budget
                        )
                        .unwrap(),
                        Some(mutation == 0)
                    );
                    assert_eq!(budget.storage(), 19);
                    // The suffix is deliberately not authorized by this check.
                }
            }
        }
    }
}

#[test]
fn source_pointer_holder_identity_retains_object_and_nonpointer_refusals() {
    let semantic = SemanticTypeIdV1::from_index(0);
    let projections =
        [SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, semantic).unwrap()];
    for binding in [
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::MovedExecution,
        SemanticValueBindingV1::Value {
            id: ValueId(61),
            ty: Type::Scalar(ScalarType::U32),
        },
        SemanticValueBindingV1::Value {
            id: ValueId(61),
            ty: Type::pointer(
                Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(7)),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        },
        cfg_option_pointer_equation_fixture().1,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert_eq!(
            execution_archive_pointer_holder_same_v29(
                &binding,
                &binding,
                &projections,
                &mut budget
            )
            .unwrap(),
            None
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_pointer_holder_identity_has_exact_structural_work_and_no_scratch() {
    let semantic = SemanticTypeIdV1::from_index(0);
    let projections =
        [SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, semantic).unwrap()];
    let binding = SemanticValueBindingV1::Value {
        id: ValueId(61),
        ty: Type::pointer(
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
    };
    // Dispatch + one projection + both complete three-node pointer types.
    let required = 1 + 1 + 2 * 3;
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(required - usize::from(short));
        let mut budget = ArgumentBudgetV1::new(&mut work, 29);
        budget.reserve_storage(29).unwrap();
        let result = execution_archive_pointer_holder_same_v29(
            &binding,
            &binding,
            &projections,
            &mut budget,
        );
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        } else {
            assert_eq!(result.unwrap(), Some(true));
            assert_eq!(budget.work(), required);
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (29, 29));
    }
}

#[test]
fn cfg_carrier_merge_requires_exact_archived_components_and_metadata() {
    let (carrier, original) = cfg_option_pointer_equation_fixture();
    for mutation in 0..7 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(37).unwrap();
        let mut candidate = original.clone();
        if let SemanticValueBindingV1::OptionPointer {
            present,
            pointer,
            pointer_ty,
            ..
        } = &mut candidate
        {
            match mutation {
                0 => {}
                1 => *present = ValueId(12),
                2 => *pointer = ValueId(14),
                3 => {
                    *pointer_ty = Type::pointer(
                        Type::Scalar(ScalarType::U64),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    )
                }
                4 => {
                    *pointer_ty = Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    )
                }
                5 => {
                    *pointer_ty = Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadOnly,
                    )
                }
                _ => std::mem::swap(present, pointer),
            }
        } else {
            panic!("fixture is not an option pointer");
        }
        let result = scoped_slot_attempt_v29(&mut budget, |budget| {
            merge_execution_cfg_carrier_v29(&carrier, &original, &candidate, budget)
        });
        if mutation == 0 {
            result.unwrap();
            let retained = budget.storage() - 37;
            budget.release_storage(retained).unwrap();
        } else {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "execution CFG transport differs from its captured SSA state")
            );
        }
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn cfg_carrier_zero_component_rebuild_has_independent_exact_resource_limits() {
    use std::mem::size_of;
    #[allow(dead_code)]
    struct AllocationHeader<'a> {
        budget: Option<&'a mut dyn SemanticEmissionBudgetV1>,
        representation: ExecutionCfgRepresentationV29,
    }
    // No heap backing is needed: allocation policy, returned binding envelope,
    // expected type vector envelope, and physical component vector envelope.
    let bytes = size_of::<AllocationHeader<'_>>()
        + size_of::<SemanticValueBindingV1>()
        + size_of::<Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1>>()
        + size_of::<Vec<Type>>()
        + size_of::<Result<Vec<Type>, ProductionSemanticKirErrorV1>>()
        + size_of::<Vec<(ValueId, Type)>>()
        + size_of::<Result<Vec<(ValueId, Type)>, ProductionSemanticKirErrorV1>>();
    // One descriptor dispatch, two empty vector constructors, one arity check.
    let required_work = 8;
    for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(required_work - usize::from(work_short));
        let mut budget = ArgumentBudgetV1::new(&mut work, 37 + bytes - usize::from(storage_short));
        budget.reserve_storage(37).unwrap();
        let result = scoped_slot_attempt_v29(&mut budget, |budget| {
            let binding = SemanticPromotedBindingV1::MathContext
                .binding_from_transport_with_allocation_v29(
                    &[],
                    SemanticTypeIdV1::from_index(0),
                    &[],
                    &mut CompilerCarrierAllocationV29::paid(budget)?,
                )?;
            assert!(matches!(binding, SemanticValueBindingV1::MathContext));
            assert_eq!(budget.storage(), 37 + bytes);
            drop(binding);
            budget.release_storage(bytes)?;
            Ok(())
        });
        assert_eq!(result.is_ok(), !work_short && !storage_short);
        if work_short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        }
        if storage_short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
        }
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn cfg_carrier_value_scope_preserves_callback_storage_and_unwinds_scratch() {
    let (carrier, original) = cfg_option_pointer_equation_fixture();
    for behavior in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(37).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_slot_attempt_v29(&mut budget, |budget| {
                with_execution_cfg_carrier_values_v29(
                    &carrier,
                    &original,
                    budget,
                    |values, budget| {
                        assert_eq!(
                            values.iter().map(|value| value.id).collect::<Vec<_>>(),
                            vec![ValueId(11), ValueId(13)]
                        );
                        budget.reserve_storage(53)?;
                        match behavior {
                            1 => Err(invocation_entry_error_v1()),
                            2 => panic!("carrier scope callback"),
                            _ => Ok(17_u32),
                        }
                    },
                )
            })
        }));
        match behavior {
            0 => {
                assert_eq!(result.unwrap().unwrap(), 17);
                let retained = std::mem::size_of::<u32>()
                    + std::mem::size_of::<Result<u32, ProductionSemanticKirErrorV1>>();
                assert_eq!(budget.storage(), 37 + 53 + retained);
                budget.release_storage(53 + retained).unwrap();
            }
            1 => assert!(matches!(
                result.unwrap(),
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            )),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn cfg_carrier_archive_shapes_can_require_but_cannot_supply_source_recipes() {
    let (_, option) = cfg_option_pointer_equation_fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 37);
    budget.reserve_storage(37).unwrap();
    for value in [
        option.clone(),
        SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Unit, option]),
    ] {
        assert!(execution_archive_needs_carrier_v29(&value, &mut 0, &mut budget).unwrap());
    }
    for value in [
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::Value {
            id: ValueId(17),
            ty: Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
        },
    ] {
        assert!(!execution_archive_needs_carrier_v29(&value, &mut 0, &mut budget).unwrap());
    }
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.peak_storage(), 37);
}
