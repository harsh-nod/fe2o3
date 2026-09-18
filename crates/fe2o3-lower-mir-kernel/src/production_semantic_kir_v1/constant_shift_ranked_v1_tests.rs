mod constant_shift_ranked_v1_tests {
    use super::*;
    include!("masked_shift_ranked_v1_tests.rs");

    fn shifted_source(
        signed: bool,
        bits: u16,
        rightward: bool,
        count: u64,
        helper: bool,
        heterogeneous: bool,
    ) -> ProductionPreRankedKirOwnerV1 {
        let base = if helper {
            source(signed, bits, false, false)
        } else {
            ranked_wrapping_source(signed, bits, 1)
        };
        let semantic = base.semantic_ssa().source_semantic();
        let mut types = semantic.types().to_vec();
        let rhs_ty = if heterogeneous {
            let ty = SemanticTypeIdV1::from_index(types.len() as u32);
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([240; 32]),
                SemanticLayoutIdentityV1::from_sha256([240; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    4,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(true, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: true,
                    bits: 32,
                }),
            ));
            ty
        } else {
            SCALAR
        };
        let mut functions = semantic.functions().to_vec();
        let old = &functions[usize::from(helper)];
        let mut blocks = old.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        let index = usize::from(!helper);
        let SemanticStatementKindV1::Assign(assignment) = statements[index].kind() else {
            panic!("scalar assignment")
        };
        let SemanticRvalueKindV1::Binary { left, .. } = assignment.value().kind() else {
            panic!("binary fixture")
        };
        statements[index] = SemanticStatementV1::new(
            old.source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                assignment.destination().clone(),
                SemanticRvalueV1::new(
                    SCALAR,
                    SemanticRvalueKindV1::Binary {
                        operation: if rightward {
                            SemanticBinaryOpV1::ShiftRight
                        } else {
                            SemanticBinaryOpV1::ShiftLeft
                        },
                        left: left.clone(),
                        right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            rhs_ty,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(
                                    u128::from(count),
                                    if heterogeneous { 4 } else { (bits / 8) as u8 },
                                )
                                .unwrap(),
                            ),
                        )),
                    },
                ),
            )),
        );
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        let mut updated = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            old.abi().clone(),
            old.locals().to_vec(),
            old.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = old.kernel_entry() {
            updated = updated.with_kernel_entry(entry.clone());
        }
        functions[usize::from(helper)] = updated;
        let admitted = InertSemanticMirRequestV1::new(
            semantic.target(),
            types,
            vec![],
            vec![],
            vec![],
            functions,
            semantic.roots().to_vec(),
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
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap()
    }

    fn shifted_lowering(
        source: &ProductionPreRankedKirOwnerV1,
        signed: bool,
        bits: u16,
        rightward: bool,
        count: u64,
        heterogeneous: bool,
        mutation: RankedMutation,
    ) -> ProductionRankedKernelLoweringInputV1 {
        let old = ranked_wrapping_lowering(source, signed, bits, 1, mutation);
        let mut operations = old.kernel().blocks()[0].operations().to_vec();
        let ProductionRankedOperationV1::SemanticExpression {
            expression,
            numerical_contract,
            ..
        } = &mut operations[3]
        else {
            panic!("ranked expression")
        };
        let scalar = expression.scalar();
        *expression = ProductionSemanticExpressionV2::Binary {
            operation: if rightward {
                ProductionSemanticBinaryOpV2::ShiftRight
            } else {
                ProductionSemanticBinaryOpV2::ShiftLeft
            },
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 + 1,
                scalar,
            }),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant {
                bits: count,
                scalar: if heterogeneous {
                    ProductionSemanticScalarTypeV2::Integer {
                        signed: true,
                        bits: 32,
                    }
                } else {
                    scalar
                },
            }),
        };
        *numerical_contract = ProductionNumericalContractV2::exact_for_expression(expression);
        let kernel = ProductionRankedKernelV1::new(
            NAME,
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("constant_shift", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.all_mandatory_reports_are_clean());
        lowering
    }

    fn sources(helper: bool) -> [ProductionRankedAccessSourceV1; 1] {
        if helper {
            access(0)
        } else {
            [ProductionRankedAccessSourceV1::new(0, Some(2), 0, 0, 5)]
        }
    }

    fn consume(
        source: ProductionPreRankedKirOwnerV1,
        lowering: ProductionRankedKernelLoweringInputV1,
        helper: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
        let root = ProductionRankedSemanticProjectionRootV1::new(
            SemanticFunctionIdV1::from_index(0),
            1,
            lowering,
            "exact constant shift into actual global ValueAccess\n".into(),
            sources(helper).to_vec(),
            vec![],
        );
        let receipt = ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(source, vec![root])?;
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(
            receipt, budget,
        )
    }

    #[test]
    fn fixed_shift_source_and_actual_n_match_consuming_global_valueaccess() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                for rightward in [false, true] {
                    for helper in [false, true] {
                        for heterogeneous in [false, true] {
                            let source =
                                shifted_source(signed, bits, rightward, 3, helper, heterogeneous);
                            let lowering = shifted_lowering(
                                &source,
                                signed,
                                bits,
                                rightward,
                                3,
                                heterogeneous,
                                RankedMutation::None,
                            );
                            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                            budget.reserve_storage(19).unwrap();
                            let owner = consume(source, lowering, helper, &mut budget).unwrap();
                            owner
                                .verify_equivalence_with_budget_v1(&mut budget)
                                .unwrap();
                            assert_eq!(
                                owner.generic_checks[0]
                                    .translation_validation
                                    .value_expressions(),
                                1
                            );
                            assert_eq!(
                                owner.generic_checks[0]
                                    .translation_validation
                                    .memory_effects(),
                                1
                            );
                            assert_eq!(budget.storage(), 19);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn changed_count_direction_sign_width_and_actual_mask_refuse_valueaccess() {
        for helper in [false, true] {
            let source = shifted_source(true, 32, true, 3, helper, true);
            for (rightward, count, mutation) in [
                (true, 2, RankedMutation::None),
                (false, 3, RankedMutation::None),
                (true, 3, RankedMutation::Signedness),
                (true, 3, RankedMutation::Width),
            ] {
                let lowering =
                    shifted_lowering(&source, true, 32, rightward, count, true, mutation);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                assert!(matches!(
                    validate_mir_pliron_translation_with_semantic_and_budget_v1(
                        Some(source.semantic_ssa().source_semantic()),
                        source.executable().module(),
                        &source.correspondence,
                        NAME,
                        &lowering,
                        &sources(helper),
                        &[],
                        source.limits.max_operations,
                        &mut budget
                    ),
                    Err(ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch { .. })
                ));
                assert_eq!(budget.storage(), 0);
            }
            let lowering = shifted_lowering(&source, true, 32, true, 3, true, RankedMutation::None);
            for changed_mask in [30, 63] {
                let mut module = source.executable().module().clone();
                let operation = module
                    .functions
                    .iter_mut()
                    .flat_map(|f| f.body.iter_mut())
                    .flat_map(|b| &mut b.blocks)
                    .flat_map(|b| &mut b.operations)
                    .find(|op| matches!(op.kind, OperationKind::Constant(Constant::I32(31))))
                    .unwrap();
                operation.kind = OperationKind::Constant(Constant::I32(changed_mask));
                verify_module(&module).unwrap();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                assert!(matches!(
                    validate_mir_pliron_translation_with_semantic_and_budget_v1(
                        Some(source.semantic_ssa().source_semantic()),
                        &module,
                        &source.correspondence,
                        NAME,
                        &lowering,
                        &sources(helper),
                        &[],
                        source.limits.max_operations,
                        &mut budget
                    ),
                    Err(ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch { .. })
                ));
                assert_eq!(budget.storage(), 0);
            }
        }
    }

    #[test]
    fn consuming_shift_attachment_exact_limits_and_one_short_restore_floor() {
        let probe = |work_limit, storage_limit| {
            let source = shifted_source(true, 64, true, 63, true, true);
            let lowering =
                shifted_lowering(&source, true, 64, true, 63, true, RankedMutation::None);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(19).unwrap();
            let result = consume(source, lowering, true, &mut budget);
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
