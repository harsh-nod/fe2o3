use super::*;
use fe2o3_pliron::ProductionSemanticUnaryOpV2 as Unary;

fn unary_owner_v39() -> ProductionSemanticSsaOwnerV1 {
    let base = private_entry_non_neutral_owner_v20();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    // Root zero reaches the helper; root one owns the same captured write.
    for (index, identity, block_identity) in [(1, 90, 110), (3, 150, 170)] {
        let prior = &functions[index];
        let mut statements = prior.blocks()[0].statements().to_vec();
        let SemanticStatementKindV1::Assign(assignment) = statements[2].kind() else {
            panic!("source fixture must retain its private write");
        };
        assert!(matches!(
            assignment.value().kind(),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                ..
            }
        ));
        let destination = assignment.destination().clone();
        statements[2] = assign(
            destination,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand: SemanticOperandV1::Copy(place(3, U32)),
            },
        );
        let mut replacement = function(
            identity,
            prior.role(),
            prior.abi().clone(),
            prior.locals().to_vec(),
            vec![block(
                block_identity,
                statements,
                SemanticTerminatorKindV1::Return,
            )],
        );
        if let Some(entry) = prior.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        functions[index] = replacement;
    }
    let callables = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn private_source_unary_operator_matrix_preserves_types_and_metadata_refusal() {
    use ProductionSemanticScalarTypeV2 as Scalar;
    assert_eq!(
        private_unary_v39(SemanticUnaryOpV1::Not, Scalar::Bool),
        Some(Unary::Not)
    );
    assert_eq!(
        private_unary_v39(SemanticUnaryOpV1::Negate, Scalar::Bool),
        None
    );
    for bits in [8, 16, 32, 64] {
        for signed in [false, true] {
            let scalar = Scalar::Integer { signed, bits };
            assert_eq!(
                private_unary_v39(SemanticUnaryOpV1::Not, scalar),
                Some(Unary::Not)
            );
            assert_eq!(
                private_unary_v39(SemanticUnaryOpV1::Negate, scalar),
                signed.then_some(Unary::Negate)
            );
            assert_eq!(
                private_unary_v39(SemanticUnaryOpV1::PointerMetadata, scalar),
                None
            );
        }
    }
    for bits in [32, 64] {
        let scalar = Scalar::Float { bits };
        assert_eq!(
            private_unary_v39(SemanticUnaryOpV1::Negate, scalar),
            Some(Unary::Negate)
        );
        assert_eq!(private_unary_v39(SemanticUnaryOpV1::Not, scalar), None);
        assert_eq!(
            private_unary_v39(SemanticUnaryOpV1::PointerMetadata, scalar),
            None
        );
    }
    for scalar in [
        Scalar::Integer {
            signed: true,
            bits: 128,
        },
        Scalar::Integer {
            signed: false,
            bits: 1,
        },
        Scalar::Float { bits: 16 },
    ] {
        for operation in [
            SemanticUnaryOpV1::Not,
            SemanticUnaryOpV1::Negate,
            SemanticUnaryOpV1::PointerMetadata,
        ] {
            assert_eq!(private_unary_v39(operation, scalar), None);
        }
    }
}

#[test]
fn private_source_unary_reconstructs_real_helper_writes_for_every_root() {
    let counts = std::cell::Cell::new([0usize; 2]);
    with_entry_fixture_v18(unary_owner_v39, |original, optimized, budget| {
        let floor = budget.storage();
        scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
            budget.reserve_storage(private_source_completion_headers_v20()?)?;
            let index = OriginalEntryIndexV20::build(original, budget)?;
            for root in 0..2 {
                original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    root,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |leaves, budget| {
                        leaves.with_checked_write_profile_v22(
                            true,
                            budget,
                            |request, budget| {
                                let expression = if request.row.source_write {
                                    let expression = index
                                        .source_write_expression_v22(leaves, request, budget)?;
                                    assert!(matches!(
                                        expression,
                                        ProductionSemanticExpressionV2::Unary {
                                            operation: Unary::Not,
                                            scalar: ProductionSemanticScalarTypeV2::Integer {
                                                signed: false,
                                                bits: 32
                                            },
                                            ..
                                        }
                                    ));
                                    let mut count = counts.get();
                                    count[root] += 1;
                                    counts.set(count);
                                    expression
                                } else {
                                    index.expression(leaves, request, budget)?
                                };
                                request.check_expression(&expression, budget)
                            },
                            |_, _| Ok(()),
                        )
                    },
                )?;
            }
            Ok(())
        })
    })
    .unwrap();
    assert!(counts.get().into_iter().all(|count| count > 0));
}

#[test]
fn private_source_unary_rejects_copied_assignment_and_changed_scalar_owner() {
    for fault in 0..3 {
        let reached = std::cell::Cell::new(false);
        let result = with_entry_fixture_v18(unary_owner_v39, |original, optimized, budget| {
            let floor = budget.storage();
            scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
                budget.reserve_storage(private_source_completion_headers_v20()?)?;
                let index = OriginalEntryIndexV20::build(original, budget)?;
                original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |leaves, budget| {
                        leaves.with_checked_write_profile_v22(
                            true,
                            budget,
                            |request, budget| {
                                if !request.row.source_write {
                                    let expression = index.expression(leaves, request, budget)?;
                                    return request.check_expression(&expression, budget);
                                }
                                let (instance, function) = request.original(budget)?;
                                let SemanticStatementKindV1::Assign(assignment) =
                                    function.blocks()[0].statements()[2].kind()
                                else {
                                    panic!("original private unary assignment");
                                };
                                let copied = assignment.value().clone();
                                let mut remaining =
                                    fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                                let scalar = if fault == 1 {
                                    ProductionSemanticScalarTypeV2::Integer {
                                        signed: true,
                                        bits: 32,
                                    }
                                } else {
                                    request.row.scalar
                                };
                                reached.set(true);
                                index
                                    .private_expression_v22(
                                        leaves.original,
                                        instance,
                                        assignment.value().result_type(),
                                        scalar,
                                        OriginalPrivateInputV22::Rvalue {
                                            block: 0,
                                            statement: if fault == 2 { 1 } else { 2 },
                                            value: if fault == 0 {
                                                &copied
                                            } else {
                                                assignment.value()
                                            },
                                        },
                                        0,
                                        &mut remaining,
                                        budget,
                                    )
                                    .map(|_| ())
                            },
                            |_, _| panic!("invalid owner cannot complete unary source write"),
                        )
                    },
                )
            })
        });
        assert!(reached.get());
        assert!(result.is_err(), "changed unary source owner accepted");
    }
}

#[test]
fn private_source_unary_keeps_exact_and_independent_one_short_query_resources() {
    let (work, storage) =
        expression_boundary_for_v39(unary_owner_v39, 1, ExpressionCutV22::Measure);
    assert!(work > 0);
    assert_eq!(storage, size_of::<ProductionSemanticExpressionV2>());
    assert_eq!(
        expression_boundary_for_v39(
            unary_owner_v39,
            1,
            ExpressionCutV22::Work {
                needed: work,
                short: false
            }
        ),
        (work, storage)
    );
    assert_eq!(
        expression_boundary_for_v39(
            unary_owner_v39,
            1,
            ExpressionCutV22::Storage {
                needed: storage,
                short: false
            }
        ),
        (work, storage)
    );
    expression_boundary_for_v39(
        unary_owner_v39,
        1,
        ExpressionCutV22::Work {
            needed: work,
            short: true,
        },
    );
    expression_boundary_for_v39(
        unary_owner_v39,
        1,
        ExpressionCutV22::Storage {
            needed: storage,
            short: true,
        },
    );
}
