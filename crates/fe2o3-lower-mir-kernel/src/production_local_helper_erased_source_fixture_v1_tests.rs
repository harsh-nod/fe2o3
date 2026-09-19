// Shared genuine semantic fixture for owning N/E and later occurrence/output
// consumers. No authenticated runtime, signed proof, or compiler origin is made.

fn erased_effect_fixture(
    expected: bool,
    root_count: usize,
) -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedSemanticProjectionRootV1>,
) {
    erased_effect_fixture_with_lifetime(expected, root_count, None)
}

fn erased_effect_fixture_with_lifetime(
    expected: bool,
    root_count: usize,
    lifetime: Option<SemanticStatementKindV1>,
) -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedSemanticProjectionRootV1>,
) {
    erased_effect_fixture_mode(expected, root_count, lifetime, false, false, false)
}

fn erased_effect_fixture_with_load_forwarding(
    expected: bool,
    root_count: usize,
) -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedSemanticProjectionRootV1>,
) {
    erased_effect_fixture_mode(expected, root_count, None, true, false, false)
}

fn erased_effect_fixture_with_integer_identity(
    expected: bool,
    root_count: usize,
) -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedSemanticProjectionRootV1>,
) {
    erased_effect_fixture_mode(expected, root_count, None, true, true, false)
}

fn erased_effect_fixture_mode(
    expected: bool,
    root_count: usize,
    lifetime: Option<SemanticStatementKindV1>,
    load_forwarding: bool,
    integer_identity: bool,
    redundant_store: bool,
) -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedSemanticProjectionRootV1>,
) {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionNumericalContractV2, ProductionRankedBlockV1,
        ProductionRankedKernelV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
        ProductionRankedValueV1, ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2,
        ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
    };
    assert!((1..=2).contains(&root_count));
    let (seed, _) = unit_source(UnitCase::CastAssert { expected }, &vec![1; root_count]);
    let source = seed.source_semantic();
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let mut types = source.types().to_vec();
    let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
    let carrier = SemanticTypeIdV1::from_index(types.len() as u32 + 1);
    let pointer_backend = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(1, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    ));
    let properties = SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
        Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        None,
    );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([205; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, pointer_backend, false)
                .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ARRAY_SCALAR,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(properties),
    );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([206; 32]),
            SemanticLayoutIdentityV1::from_sha256([206; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                pointer_backend,
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![pointer]).unwrap()),
        )
        .with_rustc_abi_properties(properties),
    );
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let mut functions = source.functions().to_vec();
    for (ordinal, function) in functions.iter_mut().take(root_count).enumerate() {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([160 + ordinal as u8; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                carrier,
                SemanticAbiPassModeV1::Direct(attributes),
            ))],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ExclusiveOwner])
        .unwrap();
        let mut locals: Vec<_> = [
            UNIT,
            carrier,
            pointer,
            ARRAY_SCALAR,
            ARRAY_SCALAR,
            LOCAL_BOOL,
            LOCAL_U64,
            UNIT,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([50 + (ordinal * 10 + index) as u8; 32]),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                provenance,
            )
        })
        .collect();
        if integer_identity {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([58 + ordinal as u8 * 10; 32]),
                LOCAL_U64,
                SemanticLocalRoleV1::Temporary,
                provenance,
            ));
        }
        let pointer_assignment = assignment(
            2,
            pointer,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), pointer)
                            .unwrap(),
                    ],
                    pointer,
                )
                .unwrap(),
            )),
        );
        let call = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(root_count as u32),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(7, UNIT),
                    edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
        let private_store = SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(3, ARRAY_SCALAR),
                constant(ARRAY_SCALAR, 7, 4),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        let private_load = assignment(
            4,
            ARRAY_SCALAR,
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                place(3, ARRAY_SCALAR),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        let index = assignment(
            6,
            LOCAL_U64,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: value(4, ARRAY_SCALAR),
            },
        );
        let predicate = assignment(
            5,
            LOCAL_BOOL,
            SemanticRvalueKindV1::Binary {
                operation: if expected {
                    SemanticBinaryOpV1::LessThan
                } else {
                    SemanticBinaryOpV1::GreaterOrEqual
                },
                left: value(if integer_identity { 8 } else { 6 }, LOCAL_U64),
                right: constant(LOCAL_U64, 8, 8),
            },
        );
        let assert = SemanticTerminatorKindV1::Assert {
            condition: value(5, LOCAL_BOOL),
            expected,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: constant(LOCAL_U64, 8, 8),
                index: value(if integer_identity { 8 } else { 6 }, LOCAL_U64),
            },
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 2),
            unwind: SemanticUnwindActionV1::Unreachable,
        };
        let global_store = SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(2),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Dereference,
                            ARRAY_SCALAR,
                        )
                        .unwrap(),
                    ],
                    ARRAY_SCALAR,
                )
                .unwrap(),
                constant(ARRAY_SCALAR, 7, 4),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        let (mut private_statements, final_statements) = if load_forwarding {
            (
                vec![private_store, global_store, private_load.clone(), private_load, index, predicate],
                vec![],
            )
        } else {
            (vec![private_store, private_load, index, predicate], vec![global_store])
        };
        if let Some(statement) = &lifetime {
            private_statements.insert(1, SemanticStatementV1::new(provenance, statement.clone()));
        }
        if redundant_store {
            let duplicate = private_statements[0].clone();
            private_statements.insert(if lifetime.is_some() { 2 } else { 1 }, duplicate);
        }
        if integer_identity {
            let before_predicate = private_statements.len() - 1;
            private_statements.insert(before_predicate, assignment(
                8,
                LOCAL_U64,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::BitXor,
                    left: value(6, LOCAL_U64),
                    right: constant(LOCAL_U64, 0, 8),
                },
            ));
        }
        let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
        let old_entry = function.kernel_entry().unwrap();
        let entry = SemanticKernelEntryV1::new(
            old_entry.export_symbol().clone(),
            old_entry.kernel_binding_identity(),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                        .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        );
        *function = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![
                block(170 + ordinal as u8 * 3, vec![pointer_assignment], call),
                block(
                    171 + ordinal as u8 * 3,
                    private_statements,
                    assert,
                ),
                block(
                    172 + ordinal as u8 * 3,
                    final_statements,
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        )
        .unwrap()
        .with_kernel_entry(entry);
    }
    let admitted = InertSemanticMirRequestV1::new(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let names: Vec<_> = admitted
        .roots()
        .iter()
        .map(|root| {
            let entry = admitted.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            (
                std::str::from_utf8(entry.export_symbol().as_bytes())
                    .unwrap()
                    .to_owned(),
                *entry.kernel_binding_identity().as_bytes(),
            )
        })
        .collect();
    let inputs: Vec<_> = names
        .iter()
        .map(|(name, binding)| {
            crate::ProductionSourceLaunchRootInputV1::new(
                name,
                *binding,
                crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(&admitted, &inputs).unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let roots = owner
        .source_launch()
        .roots()
        .iter()
        .map(|root| {
            let layout = root.layout();
            let function = &owner.semantic_ssa().source_semantic().functions()
                [root.selected_root().index() as usize];
            let name =
                std::str::from_utf8(function.kernel_entry().unwrap().export_symbol().as_bytes())
                    .unwrap();
            let local =
                |index| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(index));
            let expression = ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32,
                },
                bits: 7,
            };
            let numerical_contract =
                ProductionNumericalContractV2::exact_for_expression(&expression);
            let kernel = ProductionRankedKernelV1::new(
                name,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![
                        ProductionRankedOperationV1::ExecutionLayout {
                            grid_identity: layout.grid_identity(),
                            global_extents: layout.global_extents(),
                            workgroup_extents: layout.workgroup_extents(),
                            subgroup_size: layout.subgroup_size(),
                            full_physical_workgroups: layout.full_physical_workgroups(),
                        },
                        ProductionRankedOperationV1::View {
                            result: ProductionRankedValueIdV1::new(0),
                            element_width: 32,
                            writable: true,
                            shape: vec![1],
                            dynamic_extents: vec![],
                            allocation_origin: 1,
                            noalias_class: 1,
                        },
                        ProductionRankedOperationV1::IndexConstant {
                            result: ProductionRankedValueIdV1::new(1),
                            value: 0,
                        },
                        ProductionRankedOperationV1::SemanticExpression {
                            result: ProductionRankedValueIdV1::new(2),
                            expression,
                            numerical_contract,
                        },
                        ProductionRankedOperationV1::OwnershipContract {
                            view: local(0),
                            coverage: dialect_kernel::OwnershipCoverageAttr::TotalView,
                            partition: dialect_kernel::OwnershipPartitionAttr::ExactSets,
                        },
                        ProductionRankedOperationV1::ValueAccess {
                            kind: dialect_kernel::AccessKindAttr::Write,
                            view: local(0),
                            indices: vec![local(1)],
                            value: local(2),
                        },
                    ],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel("erased_effect_fixture", kernel).unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            assert!(lowering.all_mandatory_reports_are_clean());
            // Optional private statements can move the fixture's global effect.
            let mut global_sites = function.blocks().iter().enumerate().flat_map(|(block, body)| {
                body.statements().iter().enumerate().filter_map(move |(statement, value)| {
                    matches!(value.kind(), SemanticStatementKindV1::Store(store)
                        if store.destination().local() == SemanticLocalIdV1::from_index(2))
                    .then_some((block as u32, statement as u32))
                })
            });
            let (global_block, global_statement) = global_sites.next().unwrap();
            assert!(global_sites.next().is_none());
            ProductionRankedSemanticProjectionRootV1::new(
                root.selected_root(),
                root.source_rank(),
                lowering,
                "genuine root global store; original helper effects remain retained\n".to_owned(),
                vec![ProductionRankedAccessSourceV1::new(
                    global_block,
                    Some(global_statement),
                    0, 0, 5,
                )],
                vec![],
            )
        })
        .collect();
    (owner, roots)
}

#[test]
fn erased_owner_preserves_genuine_root_private_global_effects_and_assertions() {
    for expected in [true, false] {
        let (original, roots) = erased_effect_fixture(expected, 2);
        let candidate = deletion_module(&original);
        let floor = FLOOR + erased_input_floor(&original, &roots);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (owner, receipt) = ErasedOwner::try_produce_v1(original, roots, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(owner.erased().module(), &candidate);
        let operations: Vec<_> = owner
            .erased()
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|b| &b.blocks)
            .flat_map(|b| &b.operations)
            .collect();
        assert!(operations.iter().any(|op| matches!(&op.kind, OperationKind::Load { access, .. } if access.address_space == AddressSpace::Private)));
        assert!(operations.iter().any(|op| matches!(&op.kind, OperationKind::Store { access, .. } if access.address_space == AddressSpace::Private)));
        assert_eq!(operations.iter().filter(|op| matches!(&op.kind, OperationKind::Store { access, .. } if access.address_space == AddressSpace::Global)).count(), 2);
        assert!(
            owner
                .erased()
                .module()
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks)
                .any(|block| matches!(
                    block.terminator,
                    Some(Terminator::ConditionalBranch { .. })
                ))
        );
        assert!(
            owner
                .functions
                .iter()
                .enumerate()
                .any(|(old, new)| new.is_some_and(|new| new as usize != old)),
            "fixture must exercise a surviving function ordinal shift"
        );
    }
}
