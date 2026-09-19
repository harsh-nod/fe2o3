use super::*;

fn shift_source(operation: SemanticBinaryOpV1, count: Option<u32>) -> AdmittedInertSemanticMirV1 {
    let rhs = count.map_or_else(
        || fixture::copy(2),
        |count| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                fixture::WORD,
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(count.into(), 4).unwrap(),
                ),
            ))
        },
    );
    fixture::source(vec![fixture::function(
        60,
        false,
        0,
        vec![fixture::block(
            61,
            vec![fixture::assign(
                0,
                SemanticRvalueKindV1::Binary {
                    operation,
                    left: fixture::copy(1),
                    right: rhs,
                },
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )])
}

#[test]
fn exact_source_helper_shift_returns_reach_the_real_root_resolver() {
    for operation in [
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
    ] {
        for count in [0, 3, 31] {
            let source = shift_source(operation, Some(count));
            let function = &source.functions()[0];
            let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
                with_source_helper_values(&source, 0, meter, |context, meter| {
                    let mut resolver =
                        GpuSemanticExpressionResolverV2::new(source.types(), function).unwrap();
                    resolver.helper_semantic = Some(&source);
                    resolver.helper_values = Some(context);
                    resolver.helper_meter = Some(&mut *meter);
                    let mut resolver = resolver
                        .with_scalar_callables_v1(source.callables())
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
                    assert_eq!(
                        expression,
                        ProductionSemanticExpressionV2::Binary {
                            operation: if operation == SemanticBinaryOpV1::ShiftLeft {
                                ProductionSemanticBinaryOpV2::ShiftLeft
                            } else {
                                ProductionSemanticBinaryOpV2::ShiftRight
                            },
                            scalar,
                            overflow: ProductionOverflowContractV2::Wrapping,
                            lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                                symbol: fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2,
                                scalar
                            }),
                            rhs: Box::new(constant(u64::from(count))),
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
    }
}

#[test]
fn real_helper_context_refuses_dynamic_and_out_of_range_shift_counts() {
    for count in [None, Some(32), Some(33), Some(u32::MAX)] {
        for operation in [
            SemanticBinaryOpV1::ShiftLeft,
            SemanticBinaryOpV1::ShiftRight,
        ] {
            let source = shift_source(operation, count);
            let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
                with_source_helper_values(&source, 0, meter, |context, meter| {
                    assert_eq!(
                        source::derive(&source, 1, &[], meter).err(),
                        Some(
                            "helper shift requires an in-range literal or its actual adjacent source mask"
                        )
                    );
                    assert_eq!(
                        context
                            .call(&source, &source.functions()[0], 0, call(&source), meter)
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
}
