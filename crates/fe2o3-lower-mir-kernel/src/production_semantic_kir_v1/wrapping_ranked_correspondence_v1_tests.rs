// This is source/ranked/N correspondence, not rustc projection, optimization,
// protected proof execution, indexed-address qualification, or launch authority.
mod wrapping_ranked_correspondence_v1_tests {
    use super::*;
    use crate::{
        ProductionMaterializedRankedModuleReceiptV1, ProductionPreRankedKirOwnerV1,
        ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
        ProductionSourceLaunchRosterV1,
    };
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
    use fe2o3_mir_model::semantic_mir_v1::*;
    use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};

    const WORK: usize = 100_000_000;
    const STORAGE: usize = 512 * 1024 * 1024;
    const NAME: &str = "ranked_wrapping_store";
    const BINDING: [u8; 32] = [33; 32];
    const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const CARRIER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const OPERATIONS: [(
        SemanticBinaryOpV1,
        ProductionSemanticBinaryOpV2,
        CheckedBinaryOperator,
    ); 3] = [
        (
            SemanticBinaryOpV1::Add,
            ProductionSemanticBinaryOpV2::Add,
            CheckedBinaryOperator::Add,
        ),
        (
            SemanticBinaryOpV1::Subtract,
            ProductionSemanticBinaryOpV2::Subtract,
            CheckedBinaryOperator::Subtract,
        ),
        (
            SemanticBinaryOpV1::Multiply,
            ProductionSemanticBinaryOpV2::Multiply,
            CheckedBinaryOperator::Multiply,
        ),
    ];

    fn ranked_wrapping_types(signed: bool, bits: u16) -> Vec<SemanticTypeDeclV1> {
        assert!(matches!(bits, 8 | 16 | 32 | 64));
        let bytes = u64::from(bits / 8);
        let pointer_backend = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(1, 8, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ));
        let properties = SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        );
        vec![
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([1; 32]),
                SemanticLayoutIdentityV1::from_sha256([1; 32]),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    1,
                    SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    1,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                SemanticTypeShapeV1::Unit,
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([2; 32]),
                SemanticLayoutIdentityV1::from_sha256([2; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(bytes),
                    bytes,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                        SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([3; 32]),
                SemanticLayoutIdentityV1::from_sha256([3; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, pointer_backend, false)
                    .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SCALAR,
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
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([4; 32]),
                SemanticLayoutIdentityV1::from_sha256([4; 32]),
                SemanticTypeLayoutV1::aggregate_with_backend_repr(
                    Some(8),
                    8,
                    pointer_backend,
                    false,
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![POINTER]).unwrap(),
                ),
            )
            .with_rustc_abi_properties(properties),
        ]
    }

    fn ranked_wrapping_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
    }

    fn ranked_wrapping_source(
        signed: bool,
        bits: u16,
        operation: usize,
    ) -> ProductionPreRankedKirOwnerV1 {
        let provenance = SemanticSourceProvenanceV1::unavailable();
        let layout = SemanticLayoutIdentityV1::from_sha256([250; 32]);
        let carrier_attributes = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        let scalar_argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SCALAR,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    if bits < 32 {
                        if signed {
                            SemanticAbiExtensionV1::SignExtend
                        } else {
                            SemanticAbiExtensionV1::ZeroExtend
                        }
                    } else {
                        SemanticAbiExtensionV1::None
                    },
                    0,
                    None,
                )
                .unwrap(),
            ),
        ));
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([20; 32]),
            layout,
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            3,
            vec![
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    CARRIER,
                    SemanticAbiPassModeV1::Direct(carrier_attributes),
                )),
                scalar_argument.clone(),
                scalar_argument,
            ],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ])
        .unwrap();
        let pointer_assignment = SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                ranked_wrapping_place(4, POINTER),
                SemanticRvalueV1::new(
                    POINTER,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(1),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Field(0),
                                    POINTER,
                                )
                                .unwrap(),
                            ],
                            POINTER,
                        )
                        .unwrap(),
                    )),
                ),
            )),
        );
        let arithmetic = SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                ranked_wrapping_place(5, SCALAR),
                SemanticRvalueV1::new(
                    SCALAR,
                    SemanticRvalueKindV1::Binary {
                        operation: OPERATIONS[operation].0,
                        left: SemanticOperandV1::Copy(ranked_wrapping_place(2, SCALAR)),
                        right: SemanticOperandV1::Copy(ranked_wrapping_place(3, SCALAR)),
                    },
                ),
            )),
        );
        let store = SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(4),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR)
                            .unwrap(),
                    ],
                    SCALAR,
                )
                .unwrap(),
                SemanticOperandV1::Copy(ranked_wrapping_place(5, SCALAR)),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        let locals = [
            (UNIT, SemanticLocalRoleV1::Return),
            (CARRIER, SemanticLocalRoleV1::Argument(0)),
            (SCALAR, SemanticLocalRoleV1::Argument(1)),
            (SCALAR, SemanticLocalRoleV1::Argument(2)),
            (POINTER, SemanticLocalRoleV1::Temporary),
            (SCALAR, SemanticLocalRoleV1::Temporary),
        ];
        let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([21; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([22; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([23; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([24; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([25; 32]),
            provenance,
            abi,
            locals
                .into_iter()
                .enumerate()
                .map(|(ordinal, (ty, role))| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([40 + ordinal as u8; 32]),
                        ty,
                        role,
                        provenance,
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([28; 32]),
                    provenance,
                    vec![pointer_assignment, arithmetic, store],
                    SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(NAME.as_bytes().to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(BINDING),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                        .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ));
        let admitted = InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(layout),
            ranked_wrapping_types(signed, bits),
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let launch = ProductionSourceLaunchRosterV1::try_new(
            &admitted,
            &[ProductionSourceLaunchRootInputV1::new(
                NAME,
                BINDING,
                ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
            )],
        )
        .unwrap();
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap()
    }

    #[derive(Clone, Copy, Debug)]
    enum RankedMutation {
        None,
        SwapOperands,
        ReplaceOperand,
        Operator,
        Signedness,
        Width,
        AccessOrdinal,
    }

    fn ranked_wrapping_lowering(
        source: &ProductionPreRankedKirOwnerV1,
        signed: bool,
        bits: u16,
        operation: usize,
        mutation: RankedMutation,
    ) -> ProductionRankedKernelLoweringInputV1 {
        let (ranked_signed, ranked_bits) = match mutation {
            RankedMutation::Signedness => (!signed, bits),
            RankedMutation::Width => (signed, if bits == 64 { 32 } else { 64 }),
            _ => (signed, bits),
        };
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: ranked_signed,
            bits: ranked_bits,
        };
        let symbol = |argument| ProductionSemanticExpressionV2::Symbol {
            symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 + argument,
            scalar,
        };
        let (lhs, rhs) = match mutation {
            RankedMutation::SwapOperands => (symbol(2), symbol(1)),
            RankedMutation::ReplaceOperand => (symbol(1), symbol(1)),
            _ => (symbol(1), symbol(2)),
        };
        let expression = ProductionSemanticExpressionV2::Binary {
            operation: OPERATIONS[if matches!(mutation, RankedMutation::Operator) {
                (operation + 1) % 3
            } else {
                operation
            }]
            .1,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
        let numerical_contract = ProductionNumericalContractV2::exact_for_expression(&expression);
        let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
        let layout = source.source_launch().roots()[0].layout();
        let kernel = ProductionRankedKernelV1::new(
            NAME,
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
                        element_width: u32::from(ranked_bits),
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
                        kind: AccessKindAttr::Write,
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
            ProductionConstructionV1::ranked_kernel("ranked_wrapping_module", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.all_mandatory_reports_are_clean());
        lowering
    }

    fn ranked_wrapping_attach(
        source: ProductionPreRankedKirOwnerV1,
        lowering: ProductionRankedKernelLoweringInputV1,
        mutation: RankedMutation,
    ) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
        let root = ProductionRankedSemanticProjectionRootV1::new(
            SemanticFunctionIdV1::from_index(0),
            1,
            lowering,
            "typed wrapping global-store correspondence fixture\n".to_owned(),
            vec![ProductionRankedAccessSourceV1::new(
                0,
                Some(2),
                u32::from(matches!(mutation, RankedMutation::AccessOrdinal)),
                0,
                5,
            )],
            vec![],
        );
        let receipt = ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(source, vec![root])?;
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt)
    }

    fn ranked_wrapping_checked_operation(source: &ProductionPreRankedKirOwnerV1) -> &Operation {
        let mut operations = source.executable().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Checked(_),
                        ..
                    }
                )
            });
        let operation = operations.next().unwrap();
        assert!(operations.next().is_none());
        operation
    }

    #[test]
    fn source_ranked_global_store_matches_modular_checked_value_for_all_supported_integer_types() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                for operation in 0..OPERATIONS.len() {
                    let source = ranked_wrapping_source(signed, bits, operation);
                    let checked = ranked_wrapping_checked_operation(&source);
                    assert_eq!(checked.results.len(), 2);
                    assert_eq!(checked.results[1].ty, Type::BOOL);
                    assert_eq!(
                        kir_semantic_scalar_v1(&checked.results[0].ty),
                        Some(ProductionSemanticScalarTypeV2::Integer { signed, bits })
                    );
                    assert!(
                        matches!(checked.kind, OperationKind::Binary { op: BinaryOp::Checked(op), .. } if op == OPERATIONS[operation].2)
                    );
                    let value = checked.results[0].id;
                    assert!(source.executable().module().functions[0].body.as_ref().unwrap().blocks.iter()
                        .flat_map(|block| &block.operations)
                        .any(|row| matches!(row.kind, OperationKind::Store { value: actual, access, .. } if actual == value && access.address_space == AddressSpace::Global)));
                    let lowering = ranked_wrapping_lowering(
                        &source,
                        signed,
                        bits,
                        operation,
                        RankedMutation::None,
                    );
                    let report = validate_mir_pliron_translation_with_semantic_v1(
                        Some(source.semantic_ssa().source_semantic()),
                        source.executable().module(),
                        &source.correspondence,
                        NAME,
                        &lowering,
                        &[ProductionRankedAccessSourceV1::new(0, Some(2), 0, 0, 5)],
                        &[],
                        ProductionSemanticKirLimitsV1::default().max_operations,
                    )
                    .unwrap();
                    assert_eq!(report.memory_effects(), 1);
                    assert_eq!(report.value_expressions(), 1);
                    let owner =
                        ranked_wrapping_attach(source, lowering, RankedMutation::None).unwrap();
                    owner.verify_equivalence().unwrap();
                    assert!(owner.pre_ranked_executable().is_some());
                }
            }
        }
    }

    #[test]
    fn source_ranked_wrapping_correspondence_rejects_typed_expression_and_access_map_substitution()
    {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                for mutation in [
                    RankedMutation::SwapOperands,
                    RankedMutation::ReplaceOperand,
                    RankedMutation::Operator,
                    RankedMutation::Signedness,
                    RankedMutation::Width,
                    RankedMutation::AccessOrdinal,
                ] {
                    let source = ranked_wrapping_source(signed, bits, 1);
                    let lowering = ranked_wrapping_lowering(&source, signed, bits, 1, mutation);
                    let error = ranked_wrapping_attach(source, lowering, mutation).unwrap_err();
                    if !matches!(mutation, RankedMutation::AccessOrdinal) {
                        assert!(
                            matches!(error, ProductionSemanticKirErrorV1::MirPlironTranslation(
                            ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch { .. }
                        )),
                            "{signed} {bits} {mutation:?}: {error:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn source_checked_overflow_result_cannot_stand_in_for_the_stored_wrapping_value() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                for operation in 0..OPERATIONS.len() {
                    let source = ranked_wrapping_source(signed, bits, operation);
                    let checked = ranked_wrapping_checked_operation(&source);
                    let OperationKind::Binary { op, .. } = checked.kind else {
                        unreachable!()
                    };
                    assert!(normalize_kir_binary_v1(op, checked, checked.results[0].id).is_some());
                    assert_eq!(
                        normalize_kir_binary_v1(op, checked, checked.results[1].id),
                        None
                    );
                    let overflow = checked.results[1].id;
                    let mut counterfeit = source.executable().module().clone();
                    let store = counterfeit.functions[0]
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.operations)
                        .find(|row| matches!(row.kind, OperationKind::Store { .. }))
                        .unwrap();
                    let OperationKind::Store { value, .. } = &mut store.kind else {
                        unreachable!()
                    };
                    *value = overflow;
                    assert!(
                        verify_module(&counterfeit).is_err(),
                        "the wrong-result module cannot acquire a verified owner"
                    );
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                    assert!(fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                        &counterfeit,
                        &mut budget,
                    ).is_err());
                }
            }
        }
    }
}
