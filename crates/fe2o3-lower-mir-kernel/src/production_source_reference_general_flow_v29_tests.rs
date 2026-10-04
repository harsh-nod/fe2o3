use super::*;

#[path = "production_assert_failure_effects_v29_tests.rs"]
mod assert_failure_effects_tests;

fn replace_closure(
    functions: &mut [SemanticFunctionDeclV1],
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) {
    functions[2] = function(30, false, CAPTURE, locals, blocks);
}

fn without_unit(function: &SemanticFunctionDeclV1) -> Vec<SemanticStatementV1> {
    function.blocks()[0]
        .statements()
        .iter()
        .filter(|statement| {
            !matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if assignment.destination().local() == SemanticLocalIdV1::from_index(0))
        })
        .cloned()
        .collect()
}

#[test]
fn implicit_unit_returns_preserve_shared_unique_and_reborrow_flow() {
    for case in [Case::Shared, Case::UniqueRead, Case::Reborrow] {
        let owner = owner_with(case, |_, functions| {
            let statements = without_unit(&functions[2]);
            let locals = functions[2].locals().to_vec();
            replace_closure(
                functions,
                locals,
                vec![block(30, statements, SemanticTerminatorKindV1::Return)],
            );
        });
        let reached = std::cell::Cell::new(false);
        run_owner(owner, |plan, _| {
            reached.set(true);
            assert!(!plan.loans.is_empty());
            assert!(plan.nodes.iter().any(
                |node| node.ty == UNIT && node.kind == SourceReferenceNodeKindV29::Plain(None)
            ));
            assert!(plan.loans.iter().all(|loan| loan.source_type == REFERENCE));
            Ok(())
        })
        .unwrap();
        assert!(reached.get());
    }
}

fn append_bool(types: &mut Vec<SemanticTypeDeclV1>) -> SemanticTypeIdV1 {
    let id = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        SemanticLayoutIdentityV1::from_sha256([91; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    id
}

#[test]
fn assertions_check_every_message_and_do_not_move_success_values() {
    for message in 0..8 {
        let owner = owner_with(Case::Shared, |types, functions| {
            let boolean = append_bool(types);
            let value = SemanticOperandV1::Move(place(3, WORD));
            let other = SemanticOperandV1::Copy(place(5, WORD));
            let message = match message {
                0 => SemanticAssertMessageV1::BoundsCheck {
                    length: value,
                    index: other,
                },
                1 => SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: value,
                    right: other,
                },
                2 => SemanticAssertMessageV1::DivisionByZero(value),
                3 => SemanticAssertMessageV1::RemainderByZero(value),
                4 => SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment: value,
                    found_alignment: other,
                },
                5 => SemanticAssertMessageV1::NullPointerDereference,
                6 => SemanticAssertMessageV1::ResumedAfterReturn,
                _ => SemanticAssertMessageV1::ResumedAfterPanic,
            };
            let mut statements = without_unit(&functions[2]);
            statements.push(assign(
                place(5, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
            ));
            let condition = SemanticOperandV1::Constant(SemanticConstantV1::new(
                boolean,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
            ));
            let mut locals = functions[2].locals().to_vec();
            locals.push(local(92, WORD, SemanticLocalRoleV1::Temporary));
            replace_closure(
                functions,
                locals,
                vec![
                    block(
                        30,
                        statements,
                        SemanticTerminatorKindV1::Assert {
                            condition,
                            expected: true,
                            message,
                            target: SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::AssertSuccess,
                                SemanticBlockIdV1::from_index(1),
                            ),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                    block(
                        31,
                        vec![assign(
                            place(3, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
                        )],
                        SemanticTerminatorKindV1::Return,
                    ),
                ],
            );
        });
        let reached = std::cell::Cell::new(false);
        run_owner(owner, |plan, _| {
            reached.set(true);
            assert_eq!(plan.loans.len(), 2);
            assert!(
                plan.loans
                    .iter()
                    .all(|loan| loan.effects.referent_reads > 0)
            );
            Ok(())
        })
        .unwrap();
        assert!(reached.get(), "message {message}");
    }
}

#[test]
fn assertion_failure_operands_still_require_live_holders() {
    let owner = owner_with(Case::Shared, |types, functions| {
        let boolean = append_bool(types);
        let mut statements = without_unit(&functions[2]);
        statements.extend([
            assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(3, WORD),
                },
            ),
            dead(4),
            dead(3),
        ]);
        let locals = functions[2].locals().to_vec();
        replace_closure(
            functions,
            locals,
            vec![
                block(
                    30,
                    statements,
                    SemanticTerminatorKindV1::Assert {
                        condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            boolean,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 1).unwrap(),
                            ),
                        )),
                        expected: true,
                        message: SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Move(
                            place(3, WORD),
                        )),
                        target: SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::AssertSuccess,
                            SemanticBlockIdV1::from_index(1),
                        ),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(31, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    });
    let error = run_owner(owner, |_, _| panic!("dead diagnostic value accepted")).unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference reads a dead or undefined holder",
                ..
            }
        ),
        "{error:?}"
    );
}

fn array_owner(dynamic: bool, distinct: bool, write: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |types, functions| {
        let array = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([92; 32]),
            SemanticLayoutIdentityV1::from_sha256([92; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                16,
                8,
                SemanticFieldsShapeV1::array(8, 2),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                16,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: REFERENCE,
                length: 2,
            },
        ));
        let mut locals = functions[2].locals().to_vec();
        locals.push(local(95, array, SemanticLocalRoleV1::Temporary));
        locals.push(local(96, WORD, SemanticLocalRoleV1::Temporary));
        let mut statements = without_unit(&functions[2]);
        if distinct {
            statements.push(assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)]),
                },
            ));
        }
        statements.extend([
            assign(
                place(5, array),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        vec![
                            SemanticOperandV1::Copy(place(2, REFERENCE)),
                            SemanticOperandV1::Copy(place(if distinct { 4 } else { 2 }, REFERENCE)),
                        ],
                    )
                    .unwrap(),
                ),
            ),
            assign(
                place(6, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    WORD,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
                ))),
            ),
        ]);
        let projection = if dynamic {
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(6))
        } else {
            SemanticProjectionKindV1::ConstantIndex {
                offset: 1,
                minimum_length: 2,
                from_end: true,
            }
        };
        if write {
            statements.push(assign(
                projected(5, &[(projection, REFERENCE)]),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, REFERENCE))),
            ));
        }
        statements.extend([
            assign(
                place(2, REFERENCE),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                    5,
                    &[(projection, REFERENCE)],
                ))),
            ),
            assign(
                place(3, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                    2,
                    &[(SemanticProjectionKindV1::Dereference, WORD)],
                ))),
            ),
        ]);
        replace_closure(
            functions,
            locals,
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        );
    })
}

#[test]
fn constant_array_projection_keeps_nested_loans_and_exact_holder_write() {
    run_owner(array_owner(false, false, true), |plan, _| {
        assert_eq!(plan.loans.len(), 2);
        assert!(
            plan.loans
                .iter()
                .all(|loan| loan.effects.referent_reads >= 2)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn dynamic_array_projection_joins_same_loan_without_erasing_it() {
    run_owner(array_owner(true, false, false), |plan, _| {
        assert_eq!(plan.loans.len(), 2);
        assert!(
            plan.loans
                .iter()
                .all(|loan| loan.effects.referent_reads >= 2)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn dynamic_array_projection_does_not_select_one_distinct_loan() {
    let error = run_owner(array_owner(true, true, false), |_, _| {
        panic!("ambiguous join accepted")
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference CFG merge changes loan identity",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn dynamic_array_reference_holder_write_retains_exact_storage_obligation() {
    let error = run_owner(array_owner(true, false, true), |_, _| {
        panic!("dynamic holder write accepted")
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "projected reference assignment requires exact cell state",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn implicit_unit_rule_does_not_initialize_a_killed_scalar_return() {
    let owner = owner_with(Case::Shared, |_, functions| {
        let mut statements = without_unit(&functions[2]);
        statements.extend([
            assign(
                place(0, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
            ),
            assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(0, WORD),
                },
            ),
            dead(4),
            dead(0),
        ]);
        let mut locals = functions[2].locals().to_vec();
        locals[0] = local(30, WORD, SemanticLocalRoleV1::Return);
        let input = abi(30, false, CAPTURE);
        let output = abi(31, false, WORD);
        let output = output.adjusted_arguments()[0].value().clone();
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([30; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            input.adjusted_arguments().to_vec(),
            output,
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
        functions[2] = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([30; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
            source(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        )
        .unwrap();
        let mut locals = functions[1].locals().to_vec();
        locals.push(local(97, WORD, SemanticLocalRoleV1::Temporary));
        let statements = functions[1].blocks()[0].statements().to_vec();
        functions[1] = function(
            20,
            false,
            WORD,
            locals,
            vec![
                block(
                    20,
                    statements,
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(2),
                            vec![SemanticOperandV1::Move(place(3, CAPTURE))],
                            Some(SemanticCallDestinationV1::new(
                                place(5, WORD),
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
                block(21, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    });
    let error = run_owner(owner, |_, _| panic!("undefined scalar return accepted")).unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference return is undefined",
                ..
            }
        ),
        "{error:?}"
    );
}

fn distinct_array_replacement_owner(kill_replacement: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |types, functions| {
        let array = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([93; 32]),
            SemanticLayoutIdentityV1::from_sha256([93; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                8,
                8,
                SemanticFieldsShapeV1::array(8, 1),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: REFERENCE,
                length: 1,
            },
        ));
        let mut locals = functions[2].locals().to_vec();
        locals.extend([
            local(95, array, SemanticLocalRoleV1::Temporary),
            local(96, WORD, SemanticLocalRoleV1::Temporary),
            local(97, WORD, SemanticLocalRoleV1::Temporary),
        ]);
        let slot = SemanticProjectionKindV1::ConstantIndex {
            offset: 0,
            minimum_length: 1,
            from_end: false,
        };
        let mut statements = without_unit(&functions[2]);
        for local in [6, 7] {
            statements.push(assign(
                place(local, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
            ));
        }
        statements.extend([
            assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(6, WORD),
                },
            ),
            assign(
                place(5, array),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        vec![SemanticOperandV1::Move(place(4, REFERENCE))],
                    )
                    .unwrap(),
                ),
            ),
            assign(
                place(2, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(7, WORD),
                },
            ),
            assign(
                projected(5, &[(slot, REFERENCE)]),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, REFERENCE))),
            ),
            // The sole old loan holder was the array slot. A stale/no-op child
            // update must fail here, even before read-attribution assertions.
            dead(6),
        ]);
        if kill_replacement {
            statements.push(dead(7));
        }
        statements.extend([
            assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                    5,
                    &[(slot, REFERENCE)],
                ))),
            ),
            assign(
                place(3, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                    4,
                    &[(SemanticProjectionKindV1::Dereference, WORD)],
                ))),
            ),
        ]);
        replace_closure(
            functions,
            locals,
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        );
    })
}

#[test]
fn constant_array_replacement_updates_distinct_origin_and_live_referent() {
    let reached = std::cell::Cell::new(false);
    run_owner(distinct_array_replacement_owner(false), |plan, _| {
        reached.set(true);
        let mut old = 0;
        let mut replacement = 0;
        for loan in &plan.loans {
            let origin = &plan.origins[loan.origin];
            match origin.local.index() {
                6 => {
                    old += 1;
                    assert_eq!(
                        loan.effects.referent_reads, 0,
                        "the replaced origin must not receive the later array read"
                    );
                }
                7 => {
                    replacement += 1;
                    assert!(
                        loan.effects.referent_reads > 0,
                        "the extracted array holder must read the replacement origin"
                    );
                }
                _ => {}
            }
        }
        // The real source owner calls the worker twice; each closure instance
        // has its own former and replacement origin, not a function-wide alias.
        assert_eq!((old, replacement), (2, 2));
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
    let error = run_owner(distinct_array_replacement_owner(true), |_, _| {
        panic!("the array retained a reference to a killed replacement")
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference referent storage dies with a live loan",
                ..
            }
        ),
        "{error:?}"
    );
}
