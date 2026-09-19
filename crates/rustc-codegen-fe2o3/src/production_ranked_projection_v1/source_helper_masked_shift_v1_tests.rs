use super::*;

fn source_input(limit: u32, gap: Option<SemanticStatementKindV1>) -> AdmittedInertSemanticMirV1 {
    let mut statements = vec![fixture::assign(
        3,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::BitAnd,
            left: fixture::copy(2),
            right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                fixture::WORD,
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(u128::from(limit), 4).unwrap(),
                ),
            )),
        },
    )];
    if let Some(gap) = gap {
        statements.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            gap,
        ));
    }
    statements.push(fixture::assign(
        0,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::ShiftRight,
            left: fixture::copy(1),
            right: fixture::copy(3),
        },
    ));
    fixture::source(vec![fixture::function(
        60,
        false,
        1,
        vec![fixture::block(
            61,
            statements,
            SemanticTerminatorKindV1::Return,
        )],
    )])
}

#[test]
fn actual_adjacent_masked_helper_returns_the_dynamic_count_to_the_real_resolver() {
    let input = source_input(31, None);
    let function = &input.functions()[0];
    let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
        with_source_helper_values(&input, 0, meter, |context, meter| {
            let mut resolver =
                GpuSemanticExpressionResolverV2::new(input.types(), function).unwrap();
            resolver.helper_semantic = Some(&input);
            resolver.helper_values = Some(context);
            resolver.helper_meter = Some(&mut *meter);
            let mut resolver = resolver
                .with_scalar_callables_v1(input.callables())
                .unwrap();
            let expression = resolver.resolve_store_v2(
                function.blocks()[1].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 0,
                },
            )?;
            let scalar = ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            };
            let base = fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2;
            assert_eq!(
                expression,
                ProductionSemanticExpressionV2::Binary {
                    operation: ProductionSemanticBinaryOpV2::ShiftRight,
                    scalar,
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                        symbol: base,
                        scalar
                    }),
                    rhs: Box::new(ProductionSemanticExpressionV2::Binary {
                        operation: ProductionSemanticBinaryOpV2::BitAnd,
                        scalar,
                        overflow: ProductionOverflowContractV2::Wrapping,
                        lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                            symbol: base + 1,
                            scalar
                        }),
                        rhs: Box::new(constant(31)),
                    }),
                }
            );
            let bytes = resolver.helper_reserved;
            drop(expression);
            drop(resolver);
            meter.release(bytes)
        })
    });
    result.unwrap();
    assert_eq!(floor, 4096);
}

#[test]
fn source_helper_does_not_infer_a_mask_across_wrong_definitions_or_lifetimes() {
    for (limit, gap) in [
        (30, None),
        (63, None),
        (31, Some(SemanticStatementKindV1::Nop)),
        (
            31,
            Some(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(3),
            )),
        ),
        (
            31,
            Some(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                fixture::place(3),
                SemanticRvalueV1::new(fixture::WORD, SemanticRvalueKindV1::Use(fixture::copy(2))),
            ))),
        ),
    ] {
        let input = source_input(limit, gap);
        let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
            assert_eq!(
                source::derive(&input, 1, &[], meter).err(),
                Some(
                    "helper shift requires an in-range literal or its actual adjacent source mask"
                )
            );
            with_source_helper_values(&input, 0, meter, |context, meter| {
                assert_eq!(
                    context
                        .call(&input, &input.functions()[0], 0, call(&input), meter)
                        .err(),
                    Some("unresolved helper value recipe")
                );
                Ok(())
            })
        });
        result.unwrap();
        assert_eq!(floor, 4096);
    }
}

#[test]
fn deriving_masked_helper_respects_exact_shared_work_and_storage_limits() {
    let input = source_input(31, None);
    let probe = |work, storage| {
        run(work, storage, |meter, _| {
            let template = source::derive(&input, 1, &[], meter)?;
            template.destroy(meter)
        })
    };
    let (result, floor, work, peak, _) = probe(1_000_000, 16 << 20);
    result.unwrap();
    assert_eq!(floor, 4096);
    let (result, floor, _, _, _) = probe(work, peak);
    result.unwrap();
    assert_eq!(floor, 4096);
    let (result, floor, _, _, _) = probe(work - 1, peak);
    assert_eq!(result, Err("work"));
    assert_eq!(floor, 4096);
    let (result, floor, _, _, _) = probe(work, peak - 1);
    assert_eq!(result, Err("storage"));
    assert_eq!(floor, 4096);
}

#[test]
fn a_source_mask_does_not_remove_effect_or_control_flow_prerequisites() {
    let effects = source_input(
        31,
        Some(SemanticStatementKindV1::Deinitialize(fixture::place(3))),
    );
    let ordinary = source_input(31, None);
    let statements = ordinary.functions()[1].blocks()[0].statements();
    let split = fixture::source(vec![fixture::function(
        60,
        false,
        1,
        vec![
            fixture::block(
                61,
                vec![statements[0].clone()],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            fixture::block(
                62,
                vec![statements[1].clone()],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )]);
    for (input, expected) in [
        (
            effects,
            "helper contains memory, assumption or unsupported statement effects",
        ),
        (
            split,
            "helper shift requires an in-range literal or its actual adjacent source mask",
        ),
    ] {
        let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
            source::derive(&input, 1, &[], meter).err()
        });
        assert_eq!(result, Some(expected));
        assert_eq!(floor, 4096);
    }
}
