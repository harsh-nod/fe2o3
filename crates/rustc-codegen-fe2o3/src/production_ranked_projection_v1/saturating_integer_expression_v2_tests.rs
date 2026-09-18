mod saturating_integer_expression_v2_tests {
    use super::*;
    use SemanticSaturatingIntegerOpV1::{Add, Subtract};
    use fe2o3_mir_model::semantic_mir_v1::SemanticSaturatingIntegerOpV1;

    fn scalar_types(signed: bool, bits: u16) -> Vec<SemanticTypeDeclV1> {
        vec![SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([202; 32]),
            SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8)).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
        )]
    }

    fn abi() -> SemanticFunctionAbiV1 {
        let value = || {
            SemanticAbiValueV1::new(
                SCALAR_TYPE,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )
        };
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([204; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            2,
            vec![
                SemanticAbiArgumentV1::source(value()),
                SemanticAbiArgumentV1::source(value()),
            ],
            value(),
        )
        .unwrap()
    }

    fn callable(operation: SemanticSaturatingIntegerOpV1) -> SemanticCallableDeclV1 {
        callable_with_abi(operation, abi())
    }

    fn callable_with_abi(
        operation: SemanticSaturatingIntegerOpV1,
        abi: SemanticFunctionAbiV1,
    ) -> SemanticCallableDeclV1 {
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([205; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([206; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([207; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([208; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([209; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(operation),
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([210; 32]),
        }
    }

    fn place(local: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR_TYPE).unwrap()
    }
    fn copy(local: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(place(local))
    }
    fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
        SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
    }
    fn assign(destination: u32, operand: SemanticOperandV1) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(destination),
            SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(operand)),
        )))
    }
    fn call(
        destination: u32,
        target: u32,
        arguments: Vec<SemanticOperandV1>,
    ) -> SemanticDirectCallV1 {
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(destination),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap()
    }
    fn function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([211; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([212; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([213; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([214; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([215; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(),
            vec![
                local(216, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(217, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(218, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
                local(219, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(220, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }
    fn direct() -> SemanticFunctionDeclV1 {
        function(vec![
            block(
                221,
                vec![],
                SemanticTerminatorKindV1::Call(call(3, 1, vec![copy(1), copy(2)])),
            ),
            block(
                222,
                vec![assign(0, copy(3))],
                SemanticTerminatorKindV1::Return,
            ),
        ])
    }
    fn expression(
        signed: bool,
        bits: u16,
        operation: SemanticSaturatingIntegerOpV1,
    ) -> ProductionSemanticExpressionV2 {
        let types = scalar_types(signed, bits);
        let callables = [callable(operation)];
        let function = direct();
        resolve(&types, &callables, &function, 1, 0).unwrap()
    }
    fn resolve(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        block: usize,
        statement: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let mut resolver = GpuSemanticExpressionResolverV2::new(types, function)
            .unwrap()
            .with_scalar_callables_v1(callables)
            .unwrap();
        let result = resolver.resolve_store_v2(
            function.blocks()[block].statements()[statement].kind(),
            ScalarAssignmentSiteV1 { block, statement },
        );
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
        result
    }

    fn signed(value: u64, bits: u16) -> i128 {
        let value = u128::from(value);
        if value & (1_u128 << (bits - 1)) == 0 {
            value as i128
        } else {
            value as i128 - (1_i128 << bits)
        }
    }
    fn evaluate(expression: &ProductionSemanticExpressionV2, a: u64, b: u64) -> u64 {
        use ProductionSemanticExpressionV2 as E;
        let evaluate = |value| evaluate(value, a, b);
        match expression {
            E::Symbol { symbol, .. } => match symbol
                .checked_sub(fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2)
                .unwrap()
            {
                0 => a,
                1 => b,
                _ => panic!("unexpected symbol"),
            },
            E::Constant { bits, .. } => *bits,
            E::Binary {
                operation,
                scalar: ProductionSemanticScalarTypeV2::Integer { bits, .. },
                overflow,
                lhs,
                rhs,
            } => {
                assert_eq!(*overflow, ProductionOverflowContractV2::Wrapping);
                let mask = ((1_u128 << bits) - 1) as u64;
                let (lhs, rhs) = (evaluate(lhs), evaluate(rhs));
                match operation {
                    ProductionSemanticBinaryOpV2::Add => lhs.wrapping_add(rhs) & mask,
                    ProductionSemanticBinaryOpV2::Subtract => lhs.wrapping_sub(rhs) & mask,
                    ProductionSemanticBinaryOpV2::BitXor => lhs ^ rhs,
                    ProductionSemanticBinaryOpV2::BitAnd => lhs & rhs,
                    _ => panic!("unexpected binary"),
                }
            }
            E::Compare {
                operation: ProductionSemanticComparisonV2::LessThan,
                operand_scalar:
                    ProductionSemanticScalarTypeV2::Integer {
                        signed: is_signed,
                        bits,
                    },
                lhs,
                rhs,
            } => {
                let (lhs, rhs) = (evaluate(lhs), evaluate(rhs));
                u64::from(if *is_signed {
                    signed(lhs, *bits) < signed(rhs, *bits)
                } else {
                    lhs < rhs
                })
            }
            E::Select {
                condition,
                when_true,
                when_false,
                ..
            } => evaluate(if evaluate(condition) != 0 {
                when_true
            } else {
                when_false
            }),
            _ => panic!("unexpected expression {expression:?}"),
        }
    }

    #[test]
    fn saturation_ranked_call_expression_is_exhaustive_for_signed_and_unsigned_eight_bits() {
        for is_signed in [false, true] {
            for operation in [Add, Subtract] {
                let expression = expression(is_signed, 8, operation);
                for a in 0..=255_u64 {
                    for b in 0..=255_u64 {
                        let expected = match (is_signed, operation) {
                            (false, Add) => (a as u8).saturating_add(b as u8),
                            (false, Subtract) => (a as u8).saturating_sub(b as u8),
                            (true, Add) => (a as i8).saturating_add(b as i8) as u8,
                            (true, Subtract) => (a as i8).saturating_sub(b as i8) as u8,
                        };
                        assert_eq!(evaluate(&expression, a, b), u64::from(expected));
                    }
                }
            }
        }
    }

    #[test]
    fn saturation_ranked_wide_boundaries_and_node_counts_are_exact() {
        for is_signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                for operation in [Add, Subtract] {
                    let expression = expression(is_signed, bits, operation);
                    assert_eq!(
                        expression.validate().unwrap().nodes,
                        match (is_signed, operation) {
                            (false, Subtract) => 8,
                            (false, Add) => 10,
                            (true, Subtract) => 21,
                            (true, Add) => 23,
                        }
                    );
                    let mask = ((1_u128 << bits) - 1) as u64;
                    let sign = 1_u64 << (bits - 1);
                    for a in [0, 1, sign - 1, sign, sign + 1, mask - 1, mask] {
                        for b in [0, 1, sign - 1, sign, sign + 1, mask - 1, mask] {
                            let expected = if is_signed {
                                let (a, b) = (signed(a, bits), signed(b, bits));
                                let wide = match operation {
                                    Add => a + b,
                                    Subtract => a - b,
                                };
                                wide.clamp(-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
                                    as u64
                                    & mask
                            } else {
                                match operation {
                                    Add => {
                                        (u128::from(a) + u128::from(b)).min(u128::from(mask)) as u64
                                    }
                                    Subtract => a.saturating_sub(b),
                                }
                            };
                            assert_eq!(
                                evaluate(&expression, a, b),
                                expected,
                                "{is_signed} {bits} {operation:?} {a} {b}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn saturation_call_results_require_exact_type_callee_arity_and_all_path_origin() {
        let types = scalar_types(false, 32);
        let callables = [callable(Subtract)];
        for arguments in [
            vec![copy(1)],
            vec![copy(1), copy(2), copy(1)],
            vec![copy(3), copy(2)],
        ] {
            let function = function(vec![
                block(
                    221,
                    vec![],
                    SemanticTerminatorKindV1::Call(call(3, 1, arguments)),
                ),
                block(
                    222,
                    vec![assign(0, copy(3))],
                    SemanticTerminatorKindV1::Return,
                ),
            ]);
            assert!(resolve(&types, &callables, &function, 1, 0).is_err());
        }
        let mut wrong = callable(Subtract);
        if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut wrong {
            *operation = SemanticCompilerIntrinsicOperationV1::FabsF32;
        }
        assert!(resolve(&types, &[wrong], &direct(), 1, 0).is_err());
        assert!(resolve(&scalar_types(false, 128), &callables, &direct(), 1, 0).is_err());
        let branch = SemanticTerminatorKindV1::SwitchInt {
            discriminant: copy(1),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                )],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        };
        let alternate = function(vec![
            block(221, vec![], branch),
            block(
                222,
                vec![],
                SemanticTerminatorKindV1::Call(call(3, 2, vec![copy(1), copy(2)])),
            ),
            block(
                223,
                vec![assign(0, copy(3))],
                SemanticTerminatorKindV1::Return,
            ),
        ]);
        assert_eq!(
            resolve(&types, &callables, &alternate, 2, 0).unwrap_err(),
            "GPU scalar intrinsic result is not defined on every path"
        );
    }

    #[test]
    fn saturation_call_result_keeps_lifetime_move_and_ambiguity_refusals() {
        let types = scalar_types(false, 32);
        let callables = [callable(Add)];
        let kills = [
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(3),
            )),
            statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(3),
            )),
            statement(SemanticStatementKindV1::Deinitialize(place(3))),
            assign(4, SemanticOperandV1::Move(place(3))),
            assign(3, copy(1)),
        ];
        for kill in kills {
            let function = function(vec![
                block(
                    221,
                    vec![],
                    SemanticTerminatorKindV1::Call(call(3, 1, vec![copy(1), copy(2)])),
                ),
                block(
                    222,
                    vec![kill, assign(0, copy(3))],
                    SemanticTerminatorKindV1::Return,
                ),
            ]);
            // Reassignment is resolved to its actual new value, never the call.
            let result = resolve(&types, &callables, &function, 1, 1);
            assert!(!matches!(
                result,
                Ok(ProductionSemanticExpressionV2::Select { .. })
            ));
        }
        let repeated = function(vec![
            block(
                221,
                vec![],
                SemanticTerminatorKindV1::Call(call(3, 1, vec![copy(1), copy(2)])),
            ),
            block(
                222,
                vec![],
                SemanticTerminatorKindV1::Call(call(3, 2, vec![copy(1), copy(2)])),
            ),
            block(
                223,
                vec![assign(0, copy(3))],
                SemanticTerminatorKindV1::Return,
            ),
        ]);
        assert!(resolve(&types, &callables, &repeated, 2, 0).is_err());
        let storage_outside = function(vec![
            block(
                221,
                vec![statement(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(3),
                ))],
                SemanticTerminatorKindV1::Call(call(3, 1, vec![copy(1), copy(2)])),
            ),
            block(
                222,
                vec![
                    assign(0, copy(3)),
                    statement(SemanticStatementKindV1::StorageDead(
                        SemanticLocalIdV1::from_index(3),
                    )),
                ],
                SemanticTerminatorKindV1::Return,
            ),
        ]);
        assert!(resolve(&types, &callables, &storage_outside, 1, 0).is_ok());
    }

    #[test]
    fn saturation_call_loop_transport_requires_valid_entry_and_no_killing_backedge() {
        let types = scalar_types(false, 32);
        let callables = [callable(Add)];
        for killed in [false, true] {
            let loop_tail = if killed {
                vec![statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(3),
                ))]
            } else {
                vec![]
            };
            let function = function(vec![
                block(
                    221,
                    vec![],
                    SemanticTerminatorKindV1::Call(call(3, 1, vec![copy(1), copy(2)])),
                ),
                block(
                    222,
                    vec![assign(0, copy(3))],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(
                    223,
                    loop_tail,
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
            ]);
            assert_eq!(
                resolve(&types, &callables, &function, 1, 0).is_ok(),
                !killed
            );
        }
    }

    #[test]
    fn saturation_expansion_prepays_all_copies_depth_and_shared_source_work() {
        let types = scalar_types(false, 32);
        let function = direct();
        for signed in [false, true] {
            for operation in [Add, Subtract] {
                let scalar = ProductionSemanticScalarTypeV2::Integer { signed, bits: 32 };
                let leaf = || ProductionSemanticExpressionV2::Constant { scalar, bits: 1 };
                let nodes = match (signed, operation) {
                    (false, Subtract) => 8,
                    (false, Add) => 10,
                    (true, Subtract) => 21,
                    (true, Add) => 23,
                };
                for shortage in [0, 1] {
                    let mut resolver =
                        GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
                    resolver.work = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2
                        - nodes
                        + shortage;
                    assert_eq!(
                        resolver
                            .totalize_saturating_integer_v2(operation, scalar, leaf(), leaf())
                            .is_ok(),
                        shortage == 0
                    );
                }
                let mut deep = leaf();
                for _ in 0..fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 - 1 {
                    deep = ProductionSemanticExpressionV2::Unary {
                        operation: ProductionSemanticUnaryOpV2::Not,
                        scalar,
                        operand: Box::new(deep),
                    };
                }
                let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
                assert!(
                    resolver
                        .totalize_saturating_integer_v2(operation, scalar, deep, leaf())
                        .is_err()
                );
                assert_eq!(resolver.work, 0);
            }
        }
        let callables = [callable(Add)];
        let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
            .unwrap()
            .with_scalar_callables_v1(&callables)
            .unwrap();
        resolver.definitions.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
        assert!(
            resolver
                .resolve_store_v2(
                    function.blocks()[1].statements()[0].kind(),
                    ScalarAssignmentSiteV1 {
                        block: 1,
                        statement: 0
                    }
                )
                .is_err()
        );
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
    }

    #[test]
    fn saturation_index_is_absent_for_empty_or_unrelated_callable_rosters() {
        let types = scalar_types(false, 32);
        let function = direct();
        let resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
        let before = resolver.definitions.work;
        let resolver = resolver.with_scalar_callables_v1(&[]).unwrap();
        assert_eq!(resolver.scalar_calls.capacity(), 0);
        assert_eq!(resolver.definitions.work, before);
        let mut unrelated = callable(Add);
        if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut unrelated {
            *operation = SemanticCompilerIntrinsicOperationV1::FabsF32;
        }
        let callables = [unrelated];
        let resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
        let before = resolver.definitions.work;
        let resolver = resolver.with_scalar_callables_v1(&callables).unwrap();
        assert_eq!(resolver.scalar_calls.capacity(), 0);
        assert_eq!(resolver.definitions.work - before, function.blocks().len());
        assert!(resolver.scalar_callables.is_empty());
    }

    #[test]
    fn saturation_call_rechecks_unwinding_abi_and_exact_operand_type() {
        let types = scalar_types(false, 32);
        let value = || {
            SemanticAbiValueV1::new(
                SCALAR_TYPE,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )
        };
        let unwinding = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([231; 32]),
            SemanticLayoutIdentityV1::from_sha256([232; 32]),
            SemanticCanonAbiV1::Rust,
            true,
            false,
            vec![value(), value()],
            value(),
        )
        .unwrap();
        assert_eq!(
            resolve(
                &types,
                &[callable_with_abi(Add, unwinding)],
                &direct(),
                1,
                0
            )
            .unwrap_err(),
            "GPU saturation requires exact non-unwinding (T, T) -> T"
        );
        let wrong_type = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![],
                SemanticTypeIdV1::from_index(1),
            )
            .unwrap(),
        );
        let function = function(vec![
            block(
                221,
                vec![],
                SemanticTerminatorKindV1::Call(call(3, 1, vec![copy(1), wrong_type])),
            ),
            block(
                222,
                vec![assign(0, copy(3))],
                SemanticTerminatorKindV1::Return,
            ),
        ]);
        assert_eq!(
            resolve(&types, &[callable(Add)], &function, 1, 0).unwrap_err(),
            "GPU saturation requires exact non-unwinding (T, T) -> T"
        );
    }
}
