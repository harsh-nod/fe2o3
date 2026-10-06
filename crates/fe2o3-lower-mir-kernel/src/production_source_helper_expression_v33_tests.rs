use super::*;
include!("production_source_helper_callable_v34_tests.rs");

#[derive(Clone, Copy)]
enum HelperCase {
    SameReturns,
    DifferentReturns,
    Cycle,
    Store,
    Nested,
    UnusedArgument,
}

fn helper_owner(case: HelperCase) -> ProductionSemanticSsaOwnerV1 {
    let base = call_return_owner();
    let source = base.source_semantic();
    let mut functions = source.functions().to_vec();
    let old = &source.functions()[4];
    let mut helper_abi = old.abi().clone();
    let mut helper_locals = old.locals().to_vec();
    let returned = |tag, value| {
        block(
            tag,
            vec![assign(place(0, U32), SemanticRvalueKindV1::Use(value))],
            SemanticTerminatorKindV1::Return,
        )
    };
    let branch = || SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(1, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(1),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(2),
            ),
        )
        .unwrap(),
    };
    let blocks = match case {
        HelperCase::SameReturns => vec![
            block(230, vec![], branch()),
            returned(231, SemanticOperandV1::Copy(place(1, U32))),
            returned(232, SemanticOperandV1::Copy(place(1, U32))),
        ],
        HelperCase::DifferentReturns => vec![
            block(230, vec![], branch()),
            returned(231, literal(7)),
            returned(232, literal(8)),
        ],
        HelperCase::Cycle => vec![
            block(230, vec![], branch()),
            block(
                231,
                vec![],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            returned(232, SemanticOperandV1::Copy(place(1, U32))),
        ],
        HelperCase::Store => vec![block(
            230,
            vec![
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        place(1, U32),
                        literal(9),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
                assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
        HelperCase::Nested => {
            functions.push(function(
                240,
                SemanticFunctionRoleV1::InternalHelper,
                old.abi().clone(),
                vec![
                    local(241, U32, SemanticLocalRoleV1::Return),
                    local(242, U32, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![returned(243, SemanticOperandV1::Copy(place(1, U32)))],
            ));
            vec![
                block(
                    230,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(5),
                            vec![SemanticOperandV1::Copy(place(1, U32))],
                            Some(SemanticCallDestinationV1::new(
                                place(0, U32),
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::CallReturn,
                                    SemanticBlockIdV1::from_index(1),
                                ),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(231, vec![], SemanticTerminatorKindV1::Return),
            ]
        }
        HelperCase::UnusedArgument => {
            helper_abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([225; 32]),
                SemanticLayoutIdentityV1::from_sha256([250; 32]),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                2,
                vec![
                    old.abi().arguments()[0].clone(),
                    old.abi().arguments()[0].clone(),
                ],
                old.abi().return_value().clone(),
            )
            .unwrap();
            helper_locals.push(local(226, U32, SemanticLocalRoleV1::Argument(1)));
            for root in 0..2 {
                let original = &source.functions()[root];
                let mut root_blocks = original.blocks().to_vec();
                let SemanticTerminatorKindV1::Call(call) = root_blocks[0].terminator().kind()
                else {
                    panic!("missing original call")
                };
                let destination = call.destination().unwrap().clone();
                root_blocks[0] = block(
                    180 + root as u8,
                    vec![assign(
                        place(2, U32),
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Divide,
                            left: SemanticOperandV1::Copy(place(1, U32)),
                            right: literal(2),
                        },
                    )],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(4),
                            vec![
                                SemanticOperandV1::Copy(place(1, U32)),
                                SemanticOperandV1::Copy(place(2, U32)),
                            ],
                            Some(destination),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                );
                functions[root] = rebuild_root(original, original.locals().to_vec(), root_blocks);
            }
            vec![returned(230, SemanticOperandV1::Copy(place(1, U32)))]
        }
    };
    functions[4] = function(
        220,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        helper_locals,
        blocks,
    );
    let callables = (0..functions.len())
        .map(|at| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(at as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        source.roots().to_vec(),
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

fn quote_case(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    depth: usize,
    nodes: usize,
) -> ProductionOptimizerTestResultV18 {
    with_policy11(factory, |original, optimized, budget| {
        for root in 0..2 {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                root,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let (row, value, edge) = source_call(check, budget)?;
                        let function = original.source.instance(root, 0, budget)?.0;
                        let definition = check.index.definition(function, value, budget)?;
                        let source_leaves = leaves.original_leaves(budget)?;
                        let mut remaining = nodes;
                        let expression = check.index.private_call_expression_v33(
                            source_leaves,
                            0,
                            function,
                            definition,
                            value,
                            0,
                            edge,
                            row.ty,
                            row.scalar,
                            depth,
                            &mut remaining,
                            budget,
                        )?;
                        assert_eq!(
                            expression,
                            ProductionSemanticExpressionV2::Symbol {
                                symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2,
                                scalar: row.scalar,
                            }
                        );
                        Ok(())
                    })
                },
            )?;
        }
        Ok(())
    })
}

#[test]
fn source_helper_expression_substitutes_exact_original_arguments_across_roots() {
    quote_case(
        call_return_owner,
        0,
        fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
    )
    .unwrap();
}

#[test]
fn source_helper_expression_covers_every_acyclic_reachable_return() {
    quote_case(
        || helper_owner(HelperCase::SameReturns),
        0,
        fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
    )
    .unwrap();
}

#[test]
fn source_helper_expression_rejects_conflicting_reachable_returns() {
    assert_binding(
        quote_case(
            || helper_owner(HelperCase::DifferentReturns),
            0,
            fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
        ),
        "source helper reachable return expressions differ",
    );
}

#[test]
fn source_helper_expression_preserves_callee_termination_obligation() {
    assert_binding(
        quote_case(
            || helper_owner(HelperCase::Cycle),
            0,
            fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
        ),
        "source helper cyclic control is not modeled",
    );
}

#[test]
fn source_helper_expression_rejects_memory_side_effects() {
    assert_binding(
        quote_case(
            || helper_owner(HelperCase::Store),
            0,
            fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
        ),
        "source helper statement effect is not modeled",
    );
}

#[test]
fn source_helper_expression_checks_nested_defined_calls() {
    quote_case(
        || helper_owner(HelperCase::Nested),
        0,
        fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
    )
    .unwrap();
}

#[test]
fn source_helper_expression_checks_unused_scalar_argument_evaluation() {
    assert_binding(
        quote_case(
            || helper_owner(HelperCase::UnusedArgument),
            0,
            fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
        ),
        "private source expression unsupported arithmetic contract",
    );
}

#[test]
fn source_helper_expression_refuses_an_unbound_explicit_read() {
    let result = with_policy11(call_return_owner, |original, optimized, budget| {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
            budget,
            |leaves, budget| {
                with_check(leaves, budget, |check, budget| {
                    let child = check.index.helper_child_v33(0, 0, 0, budget)?;
                    // Isolated hostile input: this scalar assignment has no actual
                    // Load occurrence. The fabricated read must not create a leaf.
                    let value = SemanticRvalueV1::new(
                        U32,
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            place(1, U32),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    );
                    let mut remaining = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                    check.index.private_helper_discard_v33(
                        leaves.original_leaves(budget)?,
                        child.child,
                        U32,
                        ProductionSemanticScalarTypeV2::Integer {
                            signed: false,
                            bits: 32,
                        },
                        OriginalPrivateInputV22::Rvalue {
                            block: 0,
                            statement: 0,
                            value: &value,
                        },
                        0,
                        &mut remaining,
                        budget,
                    )
                })
            },
        )
    });
    assert_binding(
        result,
        "source expression explicit read lacks its captured occurrence",
    );
}

#[test]
fn source_helper_expression_keeps_existing_depth_and_node_bounds() {
    for (depth, nodes) in [
        (MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1, 64),
        (0, 0),
    ] {
        let result = quote_case(call_return_owner, depth, nodes);
        assert!(
            matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(_)
                ))
            ),
            "{result:?}"
        );
    }
}

fn helper_lookup_cut(short: bool) -> bool {
    let completed = std::cell::Cell::new(None);
    let result = with_policy11(call_return_owner, |original, optimized, budget| {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
            budget,
            |leaves, budget| {
                with_check(leaves, budget, |check, budget| {
                    let count = check.index.helper_calls.len();
                    assert_eq!(check.index.helper_calls.first().unwrap().key, [0, 0, 0]);
                    // The minimum key halves the upper endpoint until zero. The
                    // fixed work is one index query, two terminal checks, one
                    // source-instance query and eight active-sidecar checks.
                    let expected = (usize::BITS - count.leading_zeros()) as usize + 1 + 2 + 1 + 8;
                    let available = expected - usize::from(short);
                    budget
                        .charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - available)?;
                    let before = (budget.work(), budget.storage());
                    match check.index.helper_child_v33(0, 0, 0, budget) {
                        Ok(row) => {
                            assert_eq!(row.key, [0, 0, 0]);
                            assert_eq!(budget.work() - before.0, expected);
                            assert_eq!(budget.storage(), before.1);
                            completed.set(Some(true));
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "helper child exact work stop",
                            ))
                        }
                        Err(error) => {
                            assert!(
                                matches!(
                                    error,
                                    ProductionSourceOwnedViewErrorV18::Resource(
                                        ArgumentResourceV1::Work(_)
                                    )
                                ),
                                "{error:?}"
                            );
                            let after = (budget.work(), budget.storage());
                            let repeated =
                                check.index.helper_child_v33(0, 0, 0, budget).unwrap_err();
                            assert_eq!(repeated.to_string(), error.to_string());
                            assert_eq!((budget.work(), budget.storage()), after);
                            completed.set(Some(false));
                            Err(error)
                        }
                    }
                })
            },
        )
    });
    assert!(result.is_err());
    completed.get().expect("helper lookup checkpoint reached")
}

#[test]
fn source_helper_child_lookup_has_independent_exact_work_and_sticky_one_short() {
    assert!(helper_lookup_cut(false));
    assert!(!helper_lookup_cut(true));
}
