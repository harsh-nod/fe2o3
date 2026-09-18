// Included below the existing genuine wrapping/global-ValueAccess fixture.
mod defined_helper_ranked_correspondence_v1_tests {
    use super::*;
    use fe2o3_kernel_ir::FunctionRole;

    include!("defined_helper_value_route_v1_tests.rs");

    fn source(
        signed: bool,
        bits: u16,
        nested: bool,
        changed_body: bool,
    ) -> ProductionPreRankedKirOwnerV1 {
        let base = ranked_wrapping_source(signed, bits, 1);
        let semantic = base.semantic_ssa().source_semantic();
        let old = &semantic.functions()[0];
        let provenance = old.source();
        let copy = |local| SemanticOperandV1::Copy(ranked_wrapping_place(local, SCALAR));
        let edge = |next| {
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(next),
            )
        };
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                provenance,
                statements,
                SemanticTerminatorV1::new(provenance, terminator),
            )
            .unwrap()
        };
        let call = |target, arguments, destination, next| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new(
                    SemanticFunctionIdV1::from_index(target),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        ranked_wrapping_place(destination, SCALAR),
                        edge(next),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        let root = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            provenance,
            old.abi().clone(),
            old.locals().to_vec(),
            old.entry(),
            vec![
                block(
                    80,
                    vec![old.blocks()[0].statements()[0].clone()],
                    call(1, vec![copy(2), copy(3)], 5, 1),
                ),
                block(
                    81,
                    vec![old.blocks()[0].statements()[2].clone()],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
        let scalar = SemanticAbiValueV1::new(
            SCALAR,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        );
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([90; 32]),
            semantic.target_layout_identity(),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            2,
            vec![SemanticAbiArgumentV1::source(scalar.clone()); 2],
            scalar,
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
        .unwrap();
        let helper = |tag, blocks| {
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                provenance,
                abi.clone(),
                [
                    SemanticLocalRoleV1::Return,
                    SemanticLocalRoleV1::Argument(0),
                    SemanticLocalRoleV1::Argument(1),
                ]
                .into_iter()
                .enumerate()
                .map(|(i, role)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([tag + i as u8 + 1; 32]),
                        SCALAR,
                        role,
                        provenance,
                    )
                })
                .collect(),
                SemanticBlockIdV1::from_index(0),
                blocks,
            )
            .unwrap()
        };
        let arithmetic = SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                ranked_wrapping_place(0, SCALAR),
                SemanticRvalueV1::new(
                    SCALAR,
                    SemanticRvalueKindV1::Binary {
                        operation: if changed_body {
                            SemanticBinaryOpV1::Add
                        } else {
                            SemanticBinaryOpV1::Subtract
                        },
                        left: copy(1),
                        right: copy(2),
                    },
                ),
            )),
        );
        let arithmetic_helper = helper(
            100,
            vec![block(
                101,
                vec![arithmetic],
                SemanticTerminatorKindV1::Return,
            )],
        );
        let functions = if nested {
            vec![
                root,
                helper(
                    90,
                    vec![
                        block(111, vec![], call(2, vec![copy(1), copy(2)], 0, 1)),
                        block(112, vec![], SemanticTerminatorKindV1::Return),
                    ],
                ),
                arithmetic_helper,
            ]
        } else {
            vec![root, arithmetic_helper]
        };
        let admitted = InertSemanticMirRequestV1::new(
            semantic.target(),
            semantic.types().to_vec(),
            vec![],
            vec![],
            vec![],
            functions,
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

    fn access(ordinal: u32) -> [ProductionRankedAccessSourceV1; 1] {
        [ProductionRankedAccessSourceV1::new(
            1,
            Some(0),
            ordinal,
            0,
            5,
        )]
    }
    fn attach(
        source: ProductionPreRankedKirOwnerV1,
        lowering: ProductionRankedKernelLoweringInputV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
        let root = ProductionRankedSemanticProjectionRootV1::new(
            SemanticFunctionIdV1::from_index(0),
            1,
            lowering,
            "exact source/N helper return into a global ValueAccess\n".to_owned(),
            access(0).to_vec(),
            vec![],
        );
        let receipt=ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(source,vec![root])?;
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(
            receipt, budget,
        )
    }

    #[test]
    fn all_eight_integer_abis_replay_direct_and_nested_helper_values_into_actual_global_writes() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                for nested in [false, true] {
                    let source = source(signed, bits, nested, false);
                    let lowering =
                        ranked_wrapping_lowering(&source, signed, bits, 1, RankedMutation::None);
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                    budget.reserve_storage(19).unwrap();
                    let owner = attach(source, lowering, &mut budget).unwrap();
                    assert_eq!(budget.storage(), 19);
                    owner
                        .verify_equivalence_with_budget_v1(&mut budget)
                        .unwrap();
                    assert_eq!(budget.storage(), 19);
                    assert_eq!(
                        owner.generic_checks[0]
                            .translation_validation
                            .memory_effects(),
                        1
                    );
                    assert_eq!(
                        owner.generic_checks[0]
                            .translation_validation
                            .value_expressions(),
                        1
                    );
                    assert!(
                        owner
                            .module()
                            .functions
                            .iter()
                            .flat_map(|f| f.body.iter())
                            .flat_map(|b| &b.blocks)
                            .flat_map(|b| &b.operations)
                            .any(|op| matches!(op.kind, OperationKind::Call { .. }))
                    );
                }
            }
        }
    }

    #[test]
    fn exact_helper_body_ranked_operands_types_and_access_ordinals_are_not_interchangeable() {
        for mutation in [
            RankedMutation::SwapOperands,
            RankedMutation::ReplaceOperand,
            RankedMutation::Operator,
            RankedMutation::Signedness,
            RankedMutation::Width,
            RankedMutation::AccessOrdinal,
        ] {
            let source = source(false, 32, true, false);
            let lowering = ranked_wrapping_lowering(&source, false, 32, 1, mutation);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            let result = validate_mir_pliron_translation_with_semantic_and_budget_v1(
                Some(source.semantic_ssa().source_semantic()),
                source.executable().module(),
                &source.correspondence,
                NAME,
                &lowering,
                &access(u32::from(matches!(mutation, RankedMutation::AccessOrdinal))),
                &[],
                source.limits.max_operations,
                &mut budget,
            );
            assert!(result.is_err(), "{mutation:?}");
            assert_eq!(budget.storage(), 0);
        }
        let source = source(false, 32, true, true);
        let lowering = ranked_wrapping_lowering(&source, false, 32, 1, RankedMutation::None);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        assert!(attach(source, lowering, &mut budget).is_err());
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn freshly_verified_changed_actual_call_callee_and_arguments_refuse_original_ranked_recipe() {
        let source = source(false, 32, true, false);
        let lowering = ranked_wrapping_lowering(&source, false, 32, 1, RankedMutation::None);
        for callee_change in [false, true] {
            let mut module = source.executable().module().clone();
            let root = module
                .functions
                .iter()
                .position(|f| f.id == module.kernels[0].entry)
                .unwrap();
            let alternate = module
                .functions
                .iter()
                .find(|f| {
                    f.role == FunctionRole::InternalHelper
                        && f.body
                            .as_ref()
                            .unwrap()
                            .blocks
                            .iter()
                            .flat_map(|b| &b.operations)
                            .any(|op| matches!(op.kind, OperationKind::Binary { .. }))
                })
                .unwrap()
                .id
                .clone();
            let operation = module.functions[root]
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|b| &mut b.operations)
                .find(|op| matches!(op.kind, OperationKind::Call { .. }))
                .unwrap();
            let OperationKind::Call { callee, arguments } = &mut operation.kind else {
                unreachable!()
            };
            if callee_change {
                *callee = alternate;
            } else {
                arguments.swap(0, 1);
            }
            verify_module(&module).unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            assert!(
                validate_mir_pliron_translation_with_semantic_and_budget_v1(
                    Some(source.semantic_ssa().source_semantic()),
                    &module,
                    &source.correspondence,
                    NAME,
                    &lowering,
                    &access(0),
                    &[],
                    source.limits.max_operations,
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn budgeted_consuming_attachment_and_replay_restore_exact_floor_at_one_short_limits() {
        let probe = |work_limit, storage_limit| {
            let source = source(false, 32, true, false);
            let lowering = ranked_wrapping_lowering(&source, false, 32, 1, RankedMutation::None);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(19).unwrap();
            let result = attach(source, lowering, &mut budget);
            (
                result.is_ok(),
                budget.storage(),
                budget.work(),
                budget.peak_storage(),
            )
        };
        let (ok, floor, work, peak) = probe(WORK, STORAGE);
        assert!(ok);
        assert_eq!(floor, 19);
        assert!(probe(work, peak).0);
        let (ok, floor, _, _) = probe(work - 1, peak);
        assert!(!ok);
        assert_eq!(floor, 19);
        let (ok, floor, _, _) = probe(work, peak - 1);
        assert!(!ok);
        assert_eq!(floor, 19);
    }
}
