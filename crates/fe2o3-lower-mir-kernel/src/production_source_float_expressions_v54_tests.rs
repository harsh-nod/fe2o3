use super::*;

fn float_owner_v54(bits: u16) -> ProductionSemanticSsaOwnerV1 {
    assert!(matches!(bits, 32 | 64));
    let base = private_entry_non_neutral_owner_v20();
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let old = &types[U32.index() as usize];
    let bytes = u64::from(bits / 8);
    types[U32.index() as usize] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::float(bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits }),
    );
    let mut functions = semantic.functions().to_vec();
    for (index, identity, block_identity) in [(1, 90, 110), (3, 150, 170)] {
        let old = &functions[index];
        let mut statements = old.blocks()[0].statements().to_vec();
        let SemanticStatementKindV1::Assign(assignment) = statements[2].kind() else {
            panic!("original retained arithmetic write");
        };
        statements[2] = assign(
            assignment.destination().clone(),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(place(3, U32)),
                right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    U32,
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(
                            float_payload_v54(bits).into(),
                            (bits / 8) as u8,
                        )
                        .unwrap(),
                    ),
                )),
            },
        );
        let mut replacement = function(
            identity,
            old.role(),
            old.abi().clone(),
            old.locals().to_vec(),
            vec![block(
                block_identity,
                statements,
                SemanticTerminatorKindV1::Return,
            )],
        );
        if let Some(entry) = old.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        functions[index] = replacement;
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
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

fn float_payload_v54(bits: u16) -> u64 {
    match bits {
        32 => 0x7fc0_0123,
        64 => 0x7ff8_0000_0000_0123,
        _ => panic!("fixture float width"),
    }
}

fn f32_owner_v54() -> ProductionSemanticSsaOwnerV1 {
    float_owner_v54(32)
}
fn f64_owner_v54() -> ProductionSemanticSsaOwnerV1 {
    float_owner_v54(64)
}

#[test]
fn original_float_writes_check_exact_private_and_store_transcripts_on_both_roots() {
    for (factory, bits) in [(f32_owner_v54 as fn() -> _, 32), (f64_owner_v54, 64)] {
        let counts = std::cell::Cell::new([[0usize; 2]; 2]);
        with_entry_fixture_v18(factory, |original, optimized, budget| {
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
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
                                        assert_float_add_v54(&expression, bits);
                                        let mut rows = counts.get();
                                        rows[root][0] += 1;
                                        counts.set(rows);
                                        expression
                                    } else {
                                        index.expression(leaves, request, budget)?
                                    };
                                    request.check_expression(&expression, budget)
                                },
                                |_, _| Ok(()),
                            )?;
                            leaves.visit_store_inputs(budget, |disposition, budget| {
                                let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(
                                    request,
                                ) = disposition
                                else {
                                    return Ok::<_, ProductionSourceOwnedViewErrorV18>(());
                                };
                                let expression =
                                    index.store_source_expression_v23(leaves, request, budget)?;
                                request.original.check_expression(&expression, budget)?;
                                request.check_expression(&expression, budget)?;
                                if matches!(
                                    expression,
                                    ProductionSemanticExpressionV2::Binary { .. }
                                ) {
                                    assert_float_add_v54(&expression, bits);
                                    let mut rows = counts.get();
                                    rows[root][1] += 1;
                                    counts.set(rows);
                                }
                                Ok(())
                            })
                        },
                    )?;
                }
                Ok(())
            })
        })
        .unwrap();
        assert!(counts.get().into_iter().flatten().all(|count| count > 0));
    }
}

fn assert_float_add_v54(expression: &ProductionSemanticExpressionV2, bits: u16) {
    let ProductionSemanticExpressionV2::Binary {
        operation,
        scalar,
        overflow,
        lhs,
        rhs,
    } = expression
    else {
        panic!("exact source Float binary: {expression:?}");
    };
    assert_eq!(*operation, ProductionSemanticBinaryOpV2::Add);
    assert_eq!(*scalar, ProductionSemanticScalarTypeV2::Float { bits });
    assert_eq!(*overflow, ProductionOverflowContractV2::Wrapping);
    assert_eq!(lhs.scalar(), *scalar);
    assert!(
        matches!(rhs.as_ref(), ProductionSemanticExpressionV2::Constant { scalar: ty, bits: payload }
        if *ty == *scalar && *payload == float_payload_v54(bits))
    );
    assert!(matches!(
        fe2o3_pliron::ProductionNumericalContractV2::exact_for_expression(expression),
        fe2o3_pliron::ProductionNumericalContractV2::ExactIeee754OperatorCongruence { .. }
    ));
}

#[test]
fn original_float_store_checker_rejects_opcode_width_order_bits_and_checked_substitution() {
    for factory in [f32_owner_v54 as fn() -> _, f64_owner_v54] {
        for fault in 0..5 {
            let reached = std::cell::Cell::new(false);
            let result = with_entry_fixture_v18(factory, |original, optimized, budget| {
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
                            leaves.visit_store_inputs(budget, |disposition, budget| {
                                let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(
                                    request,
                                ) = disposition
                                else {
                                    return Ok::<_, ProductionSourceOwnedViewErrorV18>(());
                                };
                                let mut expression =
                                    index.store_source_expression_v23(leaves, request, budget)?;
                                request.original.check_expression(&expression, budget)?;
                                request.check_expression(&expression, budget)?;
                                let ProductionSemanticExpressionV2::Binary {
                                    operation,
                                    scalar,
                                    overflow,
                                    lhs,
                                    rhs,
                                } = &mut expression
                                else {
                                    return Ok(());
                                };
                                match fault {
                                    0 => *operation = ProductionSemanticBinaryOpV2::Subtract,
                                    1 => {
                                        *scalar = ProductionSemanticScalarTypeV2::Float {
                                            bits: if scalar.bit_width() == 32 { 64 } else { 32 },
                                        }
                                    }
                                    2 => std::mem::swap(lhs, rhs),
                                    3 => {
                                        let ProductionSemanticExpressionV2::Constant {
                                            bits, ..
                                        } = rhs.as_mut()
                                        else {
                                            panic!("exact original literal");
                                        };
                                        *bits ^= 1;
                                    }
                                    4 => *overflow = ProductionOverflowContractV2::Checked,
                                    _ => unreachable!(),
                                }
                                reached.set(true);
                                request.check_expression(&expression, budget)
                            })
                        },
                    )
                })
            });
            assert!(
                reached.get(),
                "positive original check must precede attack: {result:?}"
            );
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(_))),
                "{result:?}"
            );
        }
    }
}

#[test]
fn original_float_expression_has_exact_and_one_short_existing_scratch_limits() {
    for factory in [f32_owner_v54 as fn() -> _, f64_owner_v54] {
        let measured = expression_boundary_for_v39(factory, 2, ExpressionCutV22::Measure);
        assert!(measured.0 > 0);
        assert_eq!(measured.1, 2 * size_of::<ProductionSemanticExpressionV2>());
        assert_eq!(
            expression_boundary_for_v39(
                factory,
                2,
                ExpressionCutV22::Work {
                    needed: measured.0,
                    short: false
                }
            ),
            measured
        );
        assert_eq!(
            expression_boundary_for_v39(
                factory,
                2,
                ExpressionCutV22::Storage {
                    needed: measured.1,
                    short: false
                }
            ),
            measured
        );
        expression_boundary_for_v39(
            factory,
            2,
            ExpressionCutV22::Work {
                needed: measured.0,
                short: true,
            },
        );
        expression_boundary_for_v39(
            factory,
            2,
            ExpressionCutV22::Storage {
                needed: measured.1,
                short: true,
            },
        );
    }
}

#[test]
fn original_float_operator_selection_is_closed_and_does_not_add_integer_division() {
    use ProductionSemanticBinaryOpV2 as Out;
    for (original, expected) in [
        (SemanticBinaryOpV1::Add, Out::Add),
        (SemanticBinaryOpV1::Subtract, Out::Subtract),
        (SemanticBinaryOpV1::Multiply, Out::Multiply),
        (SemanticBinaryOpV1::Divide, Out::Divide),
        (SemanticBinaryOpV1::Remainder, Out::Remainder),
    ] {
        assert_eq!(private_float_binary_v54(original), Some(expected));
    }
    for original in [
        SemanticBinaryOpV1::BitAnd,
        SemanticBinaryOpV1::BitOr,
        SemanticBinaryOpV1::BitXor,
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
        SemanticBinaryOpV1::Equal,
    ] {
        assert_eq!(private_float_binary_v54(original), None);
    }
    assert_eq!(private_binary_v22(SemanticBinaryOpV1::Divide), None);
    assert_eq!(private_binary_v22(SemanticBinaryOpV1::Remainder), None);
}

#[test]
fn original_float_expression_frames_include_the_closed_operator_selection() {
    let frame = original_shared_capture_headers_v26().unwrap()
        + size_of::<OriginalPrivateInputV22<'_>>()
        + 3 * size_of::<ProductionSemanticExpressionV2>()
        + size_of::<SourceOwnedResultV18<ProductionSemanticExpressionV2>>()
        + size_of::<OriginalEntryDefinitionRowV20>()
        + size_of::<ProductionSourceScalarArgumentV18<'_>>()
        + size_of::<ProductionSemanticScalarTypeV2>()
        + size_of::<ProductionSemanticBinaryOpV2>()
        + size_of::<Option<ProductionSemanticBinaryOpV2>>()
        + size_of::<SemanticUnaryOpV1>()
        + size_of::<fe2o3_pliron::ProductionSemanticUnaryOpV2>()
        + size_of::<Option<fe2o3_pliron::ProductionSemanticUnaryOpV2>>()
        + size_of::<Type>()
        + size_of::<Result<Type, ProductionSemanticKirErrorV1>>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>();
    assert_eq!(
        original_private_expression_headers_v22().unwrap(),
        (MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1) * frame
            + issued_discriminant_query_headers_v31().unwrap()
            + source_call_return_headers_v32().unwrap()
            + source_helper_expression_headers_v33().unwrap()
    );
}
