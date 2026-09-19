mod masked_shift_ranked_v1_tests {
    use super::*;

    fn masked_source(
        signed: bool,
        bits: u16,
        rightward: bool,
        helper: bool,
        heterogeneous: bool,
    ) -> ProductionPreRankedKirOwnerV1 {
        let base = shifted_source(signed, bits, rightward, 3, helper, false);
        let semantic = base.semantic_ssa().source_semantic();
        let mut types = semantic.types().to_vec();
        let count_ty = if heterogeneous {
            let ty = SemanticTypeIdV1::from_index(types.len() as u32);
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([240; 32]),
                SemanticLayoutIdentityV1::from_sha256([240; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    4,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
            ));
            ty
        } else {
            SCALAR
        };
        let mut functions = semantic.functions().to_vec();
        let old = &functions[usize::from(helper)];
        let mut locals = old.locals().to_vec();
        let mut allocate = |tag| {
            let id = locals.len() as u32;
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                count_ty,
                SemanticLocalRoleV1::Temporary,
                old.source(),
            ));
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(id), vec![], count_ty).unwrap()
        };
        let raw = heterogeneous.then(|| allocate(235));
        let masked = allocate(236);
        let mut blocks = old.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        let index = usize::from(!helper);
        let SemanticStatementKindV1::Assign(assignment) = statements[index].kind() else {
            panic!("shift assignment")
        };
        let SemanticRvalueKindV1::Binary {
            operation, left, ..
        } = assignment.value().kind()
        else {
            panic!("shift value")
        };
        let destination = assignment.destination().clone();
        let operation = *operation;
        let left = left.clone();
        let input =
            SemanticOperandV1::Copy(ranked_wrapping_place(if helper { 2 } else { 3 }, SCALAR));
        let assign = |destination: SemanticPlaceV1, kind| {
            SemanticStatementV1::new(
                old.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination.clone(),
                    SemanticRvalueV1::new(destination.ty(), kind),
                )),
            )
        };
        let mut additions = Vec::new();
        let input = if let Some(raw) = raw {
            additions.push(assign(
                raw.clone(),
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: input,
                },
            ));
            SemanticOperandV1::Copy(raw)
        } else {
            input
        };
        additions.push(assign(
            masked.clone(),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left: input,
                right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    count_ty,
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(
                            u128::from(bits - 1),
                            if heterogeneous { 4 } else { (bits / 8) as u8 },
                        )
                        .unwrap(),
                    ),
                )),
            },
        ));
        additions.push(assign(
            destination,
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right: SemanticOperandV1::Copy(masked),
            },
        ));
        statements.splice(index..=index, additions);
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
            locals,
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

    fn masked_lowering(
        source: &ProductionPreRankedKirOwnerV1,
        signed: bool,
        bits: u16,
        rightward: bool,
        heterogeneous: bool,
        changed_operand: bool,
        changed_mask: bool,
    ) -> Result<ProductionRankedKernelLoweringInputV1, fe2o3_pliron::ProductionRankedKernelErrorV1>
    {
        let old = shifted_lowering(
            source,
            signed,
            bits,
            rightward,
            3,
            false,
            RankedMutation::None,
        );
        let mut operations = old.kernel().blocks()[0].operations().to_vec();
        let ProductionRankedOperationV1::SemanticExpression {
            expression,
            numerical_contract,
            ..
        } = &mut operations[3]
        else {
            panic!("ranked value")
        };
        let scalar = expression.scalar();
        let count_scalar = if heterogeneous {
            ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            }
        } else {
            scalar
        };
        let input = ProductionSemanticExpressionV2::Symbol {
            symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 + if changed_operand { 1 } else { 2 },
            scalar,
        };
        let input = if heterogeneous {
            ProductionSemanticExpressionV2::Cast {
                kind: ProductionSemanticCastV2::Integer,
                source: scalar,
                target: count_scalar,
                operand: Box::new(input),
            }
        } else {
            input
        };
        let ProductionSemanticExpressionV2::Binary { rhs, .. } = expression else {
            panic!("ranked shift")
        };
        **rhs = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::BitAnd,
            scalar: count_scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(input),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant {
                scalar: count_scalar,
                bits: u64::from(bits) - if changed_mask { 2 } else { 1 },
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
        )?;
        let lowering = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("source_masked_shift", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.all_mandatory_reports_are_clean());
        Ok(lowering)
    }

    fn accesses(helper: bool, heterogeneous: bool) -> [ProductionRankedAccessSourceV1; 1] {
        if helper {
            access(0)
        } else {
            [ProductionRankedAccessSourceV1::new(
                0,
                Some(3 + u32::from(heterogeneous)),
                0,
                0,
                5,
            )]
        }
    }

    #[test]
    fn every_fixed_integer_source_mask_replays_to_actual_n_and_consuming_valueaccess() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                for rightward in [false, true] {
                    for helper in [false, true] {
                        for heterogeneous in [false, true] {
                            // A same-type `as u32` is omitted from canonical N;
                            // its source transport is an identity, not a cast.
                            let heterogeneous = heterogeneous && (signed || bits != 32);
                            let source =
                                masked_source(signed, bits, rightward, helper, heterogeneous);
                            let lowering = masked_lowering(
                                &source,
                                signed,
                                bits,
                                rightward,
                                heterogeneous,
                                false,
                                false,
                            )
                            .unwrap();
                            let root = ProductionRankedSemanticProjectionRootV1::new(
                                SemanticFunctionIdV1::from_index(0),
                                1,
                                lowering,
                                "actual source mask and actual dynamic native count\n".into(),
                                accesses(helper, heterogeneous).to_vec(),
                                vec![],
                            );
                            let receipt = ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(source, vec![root]).unwrap();
                            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                            budget.reserve_storage(19).unwrap();
                            let owner = ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(receipt, &mut budget).unwrap();
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
    fn changed_masks_refuse_domains_and_changed_values_refuse_valueaccess() {
        for helper in [false, true] {
            for bits in [8, 64] {
                let source = masked_source(false, bits, true, helper, true);
                for (direction, changed_operand, changed_mask) in [
                    (false, false, false),
                    (true, true, false),
                    (true, false, true),
                ] {
                    let lowering = masked_lowering(
                        &source,
                        false,
                        bits,
                        direction,
                        true,
                        changed_operand,
                        changed_mask,
                    );
                    if changed_mask {
                        assert!(matches!(
                            lowering,
                            Err(fe2o3_pliron::ProductionRankedKernelErrorV1::InvalidSemanticExpression(
                                fe2o3_pliron::ProductionSemanticExpressionErrorV2::IncompleteDomain
                            ))
                        ));
                        continue;
                    }
                    let lowering = lowering.unwrap();
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                    assert!(matches!(
                        validate_mir_pliron_translation_with_semantic_and_budget_v1(
                            Some(source.semantic_ssa().source_semantic()),
                            source.executable().module(),
                            &source.correspondence,
                            NAME,
                            &lowering,
                            &accesses(helper, true),
                            &[],
                            source.limits.max_operations,
                            &mut budget
                        ),
                        Err(ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch { .. })
                    ));
                    assert_eq!(budget.storage(), 0);
                }
                let lowering =
                    masked_lowering(&source, false, bits, true, true, false, false).unwrap();
                let mut module = source.executable().module().clone();
                let count = module.functions.iter_mut().flat_map(|f| f.body.iter_mut())
                    .flat_map(|body| &mut body.blocks).flat_map(|block| &mut block.operations)
                    .find(|operation| matches!(operation.kind, OperationKind::Constant(Constant::U32(n)) if n == u32::from(bits - 1))).unwrap();
                count.kind = OperationKind::Constant(Constant::U32(u32::from(bits - 2)));
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
                        &accesses(helper, true),
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
}
