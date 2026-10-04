fn invocation_scratch_types_v29(
    pointee: u32,
    address_space: u32,
    mutability: SemanticMutabilityV1,
) -> Vec<SemanticTypeDeclV1> {
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
    vec![
        plain_bit_scalar_type(
            10,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        plain_bit_scalar_type(
            12,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([14; 32]),
            SemanticLayoutIdentityV1::from_sha256([15; 32]),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(pointee),
                    SemanticPointerKindV1::Raw,
                    mutability,
                    address_space,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
        aggregate(
            16,
            vec![
                SemanticTypeIdV1::from_index(2),
                SemanticTypeIdV1::from_index(0),
            ],
            vec![0, 8],
            16,
        ),
        aggregate(18, vec![SemanticTypeIdV1::from_index(3)], vec![0], 16),
    ]
}

#[test]
fn scratch_transport_preserves_legacy_and_original_source_pointer_modes() {
    let types = invocation_scratch_types_v29(0, 0, SemanticMutabilityV1::Mutable);
    let scratch = SemanticTypeIdV1::from_index(4);
    let scalar = SemanticTypeIdV1::from_index(0);
    let expected = vec![
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        ),
        Type::Scalar(ScalarType::U32),
    ];
    assert_eq!(
        lower_workgroup_collective_scratch_transport_v1(&types, scratch, scalar).unwrap(),
        expected
    );
    for (representation, source_space) in [
        (
            ExecutionCfgRepresentationV29::LegacyAbi,
            AddressSpace::Global,
        ),
        (
            ExecutionCfgRepresentationV29::OriginalSource,
            AddressSpace::Generic,
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(37).unwrap();
        {
            let mut allocation =
                CompilerCarrierAllocationV29::paid_with_representation(&mut budget, representation)
                    .unwrap();
            let components = allocation.ordinary_components(&types, scratch).unwrap();
            assert_eq!(
                components[0].1,
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    source_space,
                    AccessMode::ReadWrite
                )
            );
            let actual = lower_workgroup_collective_scratch_with_allocation_v29(
                &types,
                scratch,
                scalar,
                &mut allocation,
            )
            .unwrap();
            assert_eq!(actual, expected);
        }
        budget.release_storage(budget.storage() - 37).unwrap();
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn scratch_transport_modes_keep_exact_source_pointer_refusals() {
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        for (pointee, space, mutability) in [
            (1, 0, SemanticMutabilityV1::Mutable),
            (0, 1, SemanticMutabilityV1::Mutable),
            (0, 3, SemanticMutabilityV1::Mutable),
            (0, 0, SemanticMutabilityV1::Immutable),
        ] {
            let types = invocation_scratch_types_v29(pointee, space, mutability);
            let mut allocation = CompilerCarrierAllocationV29 {
                budget: None,
                representation,
            };
            let error = lower_workgroup_collective_scratch_with_allocation_v29(
                &types,
                SemanticTypeIdV1::from_index(4),
                SemanticTypeIdV1::from_index(0),
                &mut allocation,
            )
            .unwrap_err();
            assert!(
                matches!(
                    error,
                    ProductionSemanticKirErrorV1::Unsupported {
                        detail: "promoted workgroup scratch source ABI changed",
                        ..
                    }
                ),
                "{error:?}"
            );
        }
    }
}

#[test]
fn scratch_transport_modes_keep_exact_and_one_short_work() {
    let types = invocation_scratch_types_v29(0, 0, SemanticMutabilityV1::Mutable);
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        let run = |limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
            budget.reserve_storage(37).unwrap();
            let result = {
                let mut allocation = CompilerCarrierAllocationV29::paid_with_representation(
                    &mut budget,
                    representation,
                )
                .unwrap();
                lower_workgroup_collective_scratch_with_allocation_v29(
                    &types,
                    SemanticTypeIdV1::from_index(4),
                    SemanticTypeIdV1::from_index(0),
                    &mut allocation,
                )
                .map(|actual| {
                    assert_eq!(
                        actual,
                        vec![
                            Type::pointer(
                                Type::Scalar(ScalarType::U32),
                                AddressSpace::Workgroup,
                                AccessMode::ReadWrite
                            ),
                            Type::Scalar(ScalarType::U32),
                        ]
                    )
                })
            };
            budget.release_storage(budget.storage() - 37).unwrap();
            assert_eq!(budget.storage(), 37);
            let accepted = budget.work();
            drop(budget);
            (result, accepted, work.failed_work())
        };
        let (result, exact, failed) = run(1_000_000);
        result.unwrap();
        assert_eq!(failed, None);
        let (result, accepted, failed) = run(exact);
        result.unwrap();
        assert_eq!((accepted, failed), (exact, None));
        let (result, accepted, failed) = run(exact - 1);
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Work(error))) if error.actual() == exact && error.limit() == exact - 1)
        );
        assert!(accepted <= exact - 1);
        assert_eq!(failed, Some(exact));
    }
}

#[test]
fn cyclic_enum_entry_identity_is_independent_of_pointer_representation() {
    let owner = cyclic_enum_analysis_owner_v1();
    let semantic = owner.source_semantic();
    let function = &semantic.functions()[0];
    let source = owner
        .plan_for_function(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let producers = semantic_option_producers_v1(function, semantic.callables()).unwrap();
    let option = SemanticOptionDominanceV1::analyze(function, &producers).unwrap();
    let mut plan = SemanticControlFlowSsaPlanV1::analyze(
        SemanticSsaTransportInputV1 {
            types: semantic.types(),
            callables: semantic.callables(),
            function,
            semantic_function: SemanticFunctionIdV1::from_index(0),
        },
        source,
        &option,
        &BTreeMap::new(),
        100_000,
        100_000,
    )
    .unwrap();
    let phi = SsaValueV1::BlockArgument {
        block: SsaBlockIdV1::new(0),
        variable: fe2o3_mir_model::SsaVariableIdV1::new(1),
    };
    assert_eq!(plan.entry_definitions.len(), 1);
    assert_eq!(plan.live_in(0), &[1]);
    assert_ne!(plan.entry_definitions[&1], phi);
    let mut prior = None;
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        // This changes only an inert analysis policy, not source or execution authority.
        plan.representation = representation;
        let facts =
            analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, 100_000, 100_000)
                .unwrap();
        assert_eq!(facts.get(&(1, phi)), Some(&0));
        assert!(!facts.contains_key(&(0, phi)));
        if let Some(prior) = &prior {
            assert_eq!(&facts, prior);
        }
        prior = Some(facts);
        let before_scan = plan.promoted.len() + 1;
        assert!(matches!(
            analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, before_scan, 100_000),
            Err(ProductionSemanticKirErrorV1::ResourceLimit { resource: ProductionSemanticKirResourceV1::AnalysisWork, actual, limit })
                if actual == before_scan + 1 && limit == before_scan
        ));
        let saved = plan
            .block_entry_values
            .insert((0, 1), plan.entry_definitions[&1]);
        assert!(matches!(
            analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, 100_000, 100_000),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        match saved {
            Some(value) => {
                plan.block_entry_values.insert((0, 1), value);
            }
            None => {
                plan.block_entry_values.remove(&(0, 1));
            }
        }
        let live = plan.live_in.get_mut(&0).unwrap().clone();
        plan.live_in.get_mut(&0).unwrap().clear();
        assert!(matches!(
            analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, 100_000, 100_000),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        *plan.live_in.get_mut(&0).unwrap() = live;
    }
}
