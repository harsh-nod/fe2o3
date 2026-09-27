mod by_value_reference_carrier_tests_v18 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_mir_model::semantic_mir_v1::*;
    use source_ranked_consumer_resources_v18::{
        ProjectionAllocationV18 as Allocation, SourceAssertionMeterV18 as Meter,
    };
    use std::mem::size_of;

    fn scalar() -> SemanticBackendScalarV1 {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )
    }

    fn properties(info: SemanticAbiPointeeInfoV1) -> SemanticTypeAbiPropertiesV1 {
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_rustc_layout_is_noundef(true)
            .with_scalar_pointee_info(Some(info), None)
    }

    fn declaration(
        index: usize,
        layout: SemanticTypeLayoutV1,
        shape: SemanticTypeShapeV1,
        info: Option<SemanticAbiPointeeInfoV1>,
    ) -> SemanticTypeDeclV1 {
        let row = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(index as u8 + 1)),
            SemanticLayoutIdentityV1::from_sha256(bytes(index as u8 + 1)),
            layout,
            shape,
        );
        match info {
            Some(info) => row.with_rustc_abi_properties(properties(info)),
            None => row,
        }
    }

    fn aggregate(
        index: usize,
        fields: Vec<SemanticTypeIdV1>,
        offsets: Vec<u64>,
        info: SemanticAbiPointeeInfoV1,
    ) -> SemanticTypeDeclV1 {
        declaration(
            index,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::Scalar(scalar()),
                false,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
            Some(info),
        )
    }

    fn fixture(
        depth: usize,
        mutable: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        SemanticTypeIdV1,
        SemanticAbiArgumentV1,
        SemanticAbiPointeeInfoV1,
    ) {
        let info = SemanticAbiPointeeInfoV1::new(
            if mutable {
                SemanticAbiPointeeKindV1::MutableReference { unpin: true }
            } else {
                SemanticAbiPointeeKindV1::SharedReference { frozen: true }
            },
            4,
            4,
        )
        .unwrap();
        let mut types = vec![
            declaration(
                0,
                SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
                None,
            ),
            declaration(
                1,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::Scalar(scalar()),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SemanticTypeIdV1::from_index(0),
                        SemanticPointerKindV1::Reference,
                        if mutable {
                            SemanticMutabilityV1::Mutable
                        } else {
                            SemanticMutabilityV1::Immutable
                        },
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
                Some(info),
            ),
        ];
        let mut current = SemanticTypeIdV1::from_index(1);
        for _ in 0..depth {
            let index = types.len();
            types.push(aggregate(index, vec![current], vec![0], info));
            current = SemanticTypeIdV1::from_index(index as u32);
        }
        let argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            current,
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        ));
        (types, current, argument, info)
    }

    fn query(
        types: &[SemanticTypeDeclV1],
        root: SemanticTypeIdV1,
        argument: &SemanticAbiArgumentV1,
        info: SemanticAbiPointeeInfoV1,
        noalias: bool,
        allocation: &mut Allocation<'_>,
    ) -> Result<Option<AllocationContractV1>, ProductionRankedProjectionErrorV1> {
        source_argument_allocation_contract_v18(
            types,
            root,
            SemanticSourceArgumentOwnershipV1::ByValue,
            argument,
            info,
            allocation_contract_from_pointee(info.kind(), noalias, 7),
            allocation,
        )
    }

    #[test]
    fn by_value_safe_reference_carriers_have_no_whole_argument_allocation_authority() {
        for mutable in [false, true] {
            for depth in [1, 2, 8, 127] {
                let (types, root, argument, info) = fixture(depth, mutable);
                for noalias in [false, true] {
                    assert_eq!(
                        query(
                            &types,
                            root,
                            &argument,
                            info,
                            noalias,
                            &mut Allocation::Legacy
                        )
                        .unwrap(),
                        None
                    );
                }
            }
            let (types, root, argument, info) = fixture(128, mutable);
            assert!(matches!(
                query(&types, root, &argument, info, true, &mut Allocation::Legacy),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "by-value safe-reference carrier has no exact scalar ABI leaf"
                ))
            ));
        }
        // Field ordinals are original source coordinates, not a field-zero rule.
        let (mut types, _, _, info) = fixture(1, false);
        let marker = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(declaration(
            types.len(),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            None,
        ));
        let root = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(aggregate(
            types.len(),
            vec![marker, SemanticTypeIdV1::from_index(1)],
            vec![0, 0],
            info,
        ));
        let argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            root,
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        ));
        assert_eq!(
            query(&types, root, &argument, info, true, &mut Allocation::Legacy).unwrap(),
            None
        );
    }

    #[test]
    fn by_value_safe_reference_carrier_rejects_changed_original_shape_layout_and_abi() {
        for fault in 0..12 {
            let (mut types, root, mut argument, info) = fixture(2, false);
            assert_eq!(
                query(&types, root, &argument, info, true, &mut Allocation::Legacy).unwrap(),
                None
            );
            let leaf = SemanticTypeIdV1::from_index(1);
            match fault {
                0 => types[3] = aggregate(3, vec![leaf], vec![1], info),
                1 => types[3] = aggregate(3, vec![leaf, leaf], vec![0, 0], info),
                2 => types[3] = aggregate(3, vec![SemanticTypeIdV1::from_index(99)], vec![0], info),
                3 => types[3] = aggregate(3, vec![root], vec![0], info),
                4 => {
                    types[1] = declaration(
                        1,
                        types[1].layout().clone(),
                        SemanticTypeShapeV1::Pointer(
                            SemanticPointerTypeV1::new_with_kind(
                                SemanticTypeIdV1::from_index(0),
                                SemanticPointerKindV1::Raw,
                                SemanticMutabilityV1::Immutable,
                                0,
                                64,
                                SemanticPointerMetadataV1::None,
                            )
                            .unwrap(),
                        ),
                        Some(info),
                    )
                }
                5 => {
                    types[1] = declaration(
                        1,
                        types[1].layout().clone(),
                        SemanticTypeShapeV1::Pointer(
                            SemanticPointerTypeV1::new_with_kind(
                                SemanticTypeIdV1::from_index(0),
                                SemanticPointerKindV1::Reference,
                                SemanticMutabilityV1::Mutable,
                                0,
                                64,
                                SemanticPointerMetadataV1::None,
                            )
                            .unwrap(),
                        ),
                        Some(info),
                    )
                }
                6 => {
                    types[2] = types[2].clone().with_rustc_abi_properties(properties(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: false },
                            0,
                            4,
                        )
                        .unwrap(),
                    ))
                }
                7 => {
                    argument = SemanticAbiArgumentV1::source(
                        argument.value().clone().with_pointee_override(info),
                    )
                }
                8 => {
                    argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        root,
                        SemanticAbiPassModeV1::Ignore,
                    ))
                }
                9 => {
                    argument =
                        SemanticAbiArgumentV1::rust_call_tuple_field(0, argument.value().clone())
                }
                10 => {
                    argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        leaf,
                        argument.mode().clone(),
                    ))
                }
                11 => {
                    types[2] = declaration(
                        2,
                        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                        types[2].shape().clone(),
                        Some(info),
                    )
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    query(&types, root, &argument, info, true, &mut Allocation::Legacy),
                    Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "by-value safe-reference carrier has no exact scalar ABI leaf"
                    ))
                ),
                "fault {fault}"
            );
        }
        // The original direct-pointer ownership gate is deliberately unchanged.
        for mutable in [false, true] {
            let (types, root, argument, info) = fixture(0, mutable);
            assert!(matches!(
                query(&types, root, &argument, info, true, &mut Allocation::Legacy),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "source ownership disagrees with rustc ABI pointer provenance"
                ))
            ));
            let ownership = if mutable {
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            } else {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            };
            let contract = allocation_contract_from_pointee(info.kind(), true, 7);
            assert_eq!(
                source_argument_allocation_contract_v18(
                    &types,
                    root,
                    ownership,
                    &argument,
                    info,
                    contract,
                    &mut Allocation::Legacy
                )
                .unwrap(),
                Some(contract)
            );
        }
    }

    fn headers() -> usize {
        size_of::<Option<AllocationContractV1>>()
            + size_of::<Result<Option<AllocationContractV1>, ProductionRankedProjectionErrorV1>>()
            + size_of::<Result<AllocationContractV1, ProductionRankedProjectionErrorV1>>()
            + size_of::<Option<&SemanticTypeDeclV1>>()
            + size_of::<Option<&SemanticTypeShapeV1>>()
            + size_of::<Result<(), ProductionRankedProjectionErrorV1>>()
            + size_of::<(
                Option<&SemanticTypeDeclV1>,
                &SemanticTypeDeclV1,
                &SemanticTypeDeclV1,
                Option<&SemanticTypeDeclV1>,
                Option<&[u64]>,
                &[u64],
                &SemanticBackendScalarV1,
                &SemanticAggregateTypeV1,
                &SemanticPointerTypeV1,
                Option<SemanticTypeIdV1>,
                SemanticTypeIdV1,
                Option<SemanticBackendScalarV1>,
                usize,
                usize,
                std::iter::Enumerate<std::slice::Iter<'_, SemanticTypeIdV1>>,
                Result<&SemanticTypeDeclV1, ProductionRankedProjectionErrorV1>,
                Result<&[u64], ProductionRankedProjectionErrorV1>,
                Result<SemanticTypeIdV1, ProductionRankedProjectionErrorV1>,
                Result<usize, ProductionRankedProjectionErrorV1>,
            )>()
    }

    #[test]
    fn by_value_safe_reference_carrier_has_bounded_exact_and_short_resource_equations() {
        for depth in [1, 2, 8, 127] {
            let (types, root, argument, info) = fixture(depth, false);
            // Dispatch 6 + ABI 8 + 12 per type + 6 per edge + terminal 4.
            let work = 30 + 18 * depth;
            let storage = headers();
            for (work_limit, storage_limit) in
                [(work, storage), (work - 1, storage), (work, storage - 1)]
            {
                let mut ledger = Work::new(work_limit);
                let mut budget = Budget::new(&mut ledger, storage_limit + 31);
                budget.reserve_storage(31).unwrap();
                let result = query(
                    &types,
                    root,
                    &argument,
                    info,
                    true,
                    &mut Allocation::Source(&mut Meter(&mut budget)),
                );
                if (work_limit, storage_limit) == (work, storage) {
                    assert_eq!(result.unwrap(), None);
                    assert_eq!((budget.work(), budget.storage()), (work, storage + 31));
                    budget.release_storage(storage).unwrap();
                    assert_eq!(budget.storage(), 31);
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::SourceOwned(
                                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                                    _
                                )
                            )
                        ))
                    ));
                    if work_limit < work {
                        assert_eq!(budget.failed_work(), Some(work));
                    } else {
                        assert_eq!(budget.failed_storage(), Some(storage + 31));
                    }
                }
            }
        }
    }

    #[test]
    fn by_value_safe_reference_carrier_does_not_seed_a_downstream_memory_contract() {
        let (types, root, argument, info) = fixture(1, false);
        assert_eq!(root, POINTER_TYPE);
        let leaf = SemanticTypeIdV1::from_index(1);
        let local_pointer = |local| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], leaf).unwrap()
        };
        let selected = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), leaf).unwrap()],
            leaf,
        )
        .unwrap();
        let receiver = local_pointer(3);
        let function = projection_function_with_owned_argument(
            vec![
                block(
                    230,
                    vec![
                        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            local_pointer(2),
                            SemanticRvalueV1::new(
                                leaf,
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(selected)),
                            ),
                        ))),
                        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            local_pointer(3),
                            SemanticRvalueV1::new(
                                leaf,
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_pointer(
                                    2,
                                ))),
                            ),
                        ))),
                    ],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(0),
                            vec![SemanticOperandV1::Copy(receiver), constant(0)],
                            Some(SemanticCallDestinationV1::new(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(0),
                                    vec![],
                                    SCALAR_TYPE,
                                )
                                .unwrap(),
                                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(231, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(230, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(231, root, SemanticLocalRoleV1::Argument(0)),
                local(232, leaf, SemanticLocalRoleV1::Temporary),
                local(233, leaf, SemanticLocalRoleV1::Temporary),
            ],
            SemanticSourceArgumentOwnershipV1::ByValue,
        );
        let provenance = local_provenance_v1(&types, &function).unwrap();
        assert_eq!(
            provenance.allocation_origins,
            [None, Some(0), Some(0), Some(0)]
        );
        assert_eq!(
            local_allocation_contracts(&types, &function, &provenance.allocation_origins).unwrap(),
            [None, None, None, None]
        );
        assert_eq!(
            query(&types, root, &argument, info, true, &mut Allocation::Legacy).unwrap(),
            None
        );
        // This is an inert attempted consumer, not an admitted slice source:
        // an aggregate must not acquire allocation authority from its ABI leaf.
        let callables = [compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad {
                element: SCALAR_TYPE,
            },
        )];
        let effects = DefinedCallableEmptyEffectSummariesV1 {
            decisions: Box::new([]),
        };
        let result = project_intrinsic_contracts(
            &callables,
            &effects,
            &types,
            &function,
            None,
            &[None; 4],
            &mut Vec::new(),
            &mut 0,
            &mut String::new(),
        );
        assert!(
            matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a volatile load receiver without one authenticated kernel-argument origin"
                ))
            ),
            "{:?}",
            result.as_ref().err()
        );
    }
}
