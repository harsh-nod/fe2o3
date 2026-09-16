// Included after canonical-memory test helpers. All owners and optimized O
// endpoints are freshly materialized; only inert claims are adversarially edited.

fn conditional_literal_source_v1(
    bits: u64,
    extra: Option<(SemanticTypeIdV1, u64)>,
    redefine: bool,
) -> (ProductionPreRankedKirOwnerV1, Option<SemanticLocalIdV1>) {
    let seed = ordinary_source_owner_v1(ResultShape::Array, BodyShape::Straight);
    let semantic = seed.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    let extra_local = extra.map(|(ty, _)| {
        let id = SemanticLocalIdV1::from_index(locals.len() as u32);
        locals.push(local(250, ty, SemanticLocalRoleV1::Temporary));
        id
    });
    let mut blocks = root.blocks().to_vec();
    let mut changed = 0;
    let mut statements = blocks[2]
        .statements()
        .iter()
        .map(|statement| {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                && assignment.destination().local().index() == 8
            {
                changed += 1;
                typed_assignment(
                    8,
                    A_U64,
                    SemanticRvalueKindV1::Use(typed_constant(A_U64, u128::from(bits), 8)),
                )
            } else {
                statement.clone()
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(changed, 1);
    if let (Some((ty, bits)), Some(id)) = (extra, extra_local) {
        statements.push(typed_assignment(
            id.index(),
            ty,
            SemanticRvalueKindV1::Use(typed_constant(
                ty,
                u128::from(bits),
                if ty == A_U64 { 8 } else { 4 },
            )),
        ));
    }
    if redefine {
        statements.push(typed_assignment(
            8,
            A_U64,
            SemanticRvalueKindV1::Use(typed_constant(A_U64, u128::from(bits), 8)),
        ));
    }
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        blocks[2].source(),
        statements,
        blocks[2].terminator().clone(),
    )
    .unwrap();
    functions[0] = ordinary_rebuild_v1(root, root.abi().clone(), locals, blocks);
    (
        materialize_ranked_fixture_v1(
            assertion_ssa_functions(semantic.types().to_vec(), functions),
            &[ranked_root_input_1d(A_NAME, 247, 1)],
        )
        .unwrap(),
        extra_local,
    )
}

#[test]
fn conditional_control_source_literals_have_exact_guard_borrows_on_both_profiles() {
    use fe2o3_lower_mir_kernel::ProductionConditionalMemoryIndexLeafV1 as Leaf;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for bits in [0, 3, 17] {
            let (source, _) = conditional_literal_source_v1(bits, None, false);
            let mut completed = false;
            with_canonical_control_candidate_test_v1(
                &source,
                profile,
                |_| {},
                |view, candidates, budget| {
                    let floor = budget.storage();
                    view.with_conditional_memory_control_coverage_v1(
                        candidates,
                        budget,
                        |coverage, budget| {
                            let candidate = &candidates[0];
                            assert!(!candidate.access_sources.is_empty());
                            let claim = candidate
                                .control
                                .arguments
                                .iter()
                                .find(|claim| claim.source_local.index() == 8)
                                .unwrap();
                            assert!(
                                coverage
                                    .index_value(view, candidate, claim.ranked_value, budget)?
                                    .is_none()
                            );
                            let Some(Leaf::Literal(literal)) =
                                coverage.index_leaf(view, candidate, claim.ranked_value, budget)?
                            else {
                                panic!("source literal must not masquerade as a formal");
                            };
                            assert_eq!(literal.source_local(), claim.source_local);
                            assert_eq!(literal.source_block().index(), 2);
                            assert_eq!(literal.source_statement(), 0);
                            assert_eq!(literal.bits(), bits);
                            assert_eq!(
                                literal.scalar(),
                                fe2o3_pliron::ProductionSemanticScalarTypeV2::Integer {
                                    signed: false,
                                    bits: 64
                                }
                            );
                            assert_eq!(literal.guard_use_count(), 1);
                            let used = literal.guard_use(0).unwrap();
                            assert_eq!(used.source_guard().index(), 2);
                            assert!(matches!(
                                used.original_use(),
                                fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                                    operand: 0,
                                    ..
                                }
                            ));
                            assert!(matches!(
                                used.output_use(),
                                fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                                    operand: 0,
                                    ..
                                }
                            ));
                            assert!(literal.guard_use(1).is_none());
                            completed = true;
                            Ok(())
                        },
                    )?;
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
            .unwrap();
            assert!(completed);
        }
    }
}

#[test]
fn conditional_control_literal_wrong_value_equal_value_other_definition_and_type_reject() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for (ty, value) in [(A_U64, 19), (A_U64, 17), (A_U32, 17)] {
            let (source, other) = conditional_literal_source_v1(17, Some((ty, value)), false);
            let mut completed = false;
            let result = with_canonical_control_candidate_test_v1(
                &source,
                profile,
                |claims| {
                    claims
                        .arguments
                        .iter_mut()
                        .find(|claim| claim.source_local.index() == 8)
                        .unwrap()
                        .source_local = other.unwrap();
                },
                |view, candidates, budget| {
                    let floor = budget.storage();
                    let result = view.with_conditional_memory_control_coverage_v1(
                        candidates,
                        budget,
                        |_, _| {
                            completed = true;
                            Ok(())
                        },
                    );
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Invalid(_))),
                        "{result:?}"
                    );
                    assert_eq!(budget.storage(), floor);
                    result
                },
            );
            assert!(result.is_err());
            assert!(!completed);
        }
    }
}

#[test]
fn conditional_control_literal_callback_error_and_panic_keep_source_and_floor_reusable() {
    let (source, _) = conditional_literal_source_v1(17, None, false);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, candidates, budget| {
                let floor = budget.storage();
                for panic in [false, true] {
                    let before = budget.work();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        view.with_conditional_memory_control_coverage_v1(
                            candidates,
                            budget,
                            |_, _| {
                                if panic {
                                    panic!("literal scoped callback");
                                }
                                Err::<(), _>(ProductionSourceOutputErrorV1::Invalid(
                                    "literal scoped callback",
                                ))
                            },
                        )
                    }));
                    assert_eq!(result.is_err(), panic);
                    if let Ok(result) = result {
                        assert!(matches!(
                            result,
                            Err(ProductionSourceOutputErrorV1::Invalid(
                                "literal scoped callback"
                            ))
                        ));
                    }
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                }
                view.with_conditional_memory_control_coverage_v1(
                    candidates,
                    budget,
                    |_, _| Ok(()),
                )?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn conditional_control_literal_call_destination_and_lifetime_kill_are_not_constants() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for killed in [false, true] {
            let (seed, _) = conditional_literal_source_v1(17, None, false);
            let semantic = seed.semantic_ssa().source_semantic();
            let call_result =
                SemanticLocalIdV1::from_index((semantic.functions()[0].locals().len() - 1) as u32);
            let source = if killed {
                let mut functions = semantic.functions().to_vec();
                let root = &functions[0];
                let mut blocks = root.blocks().to_vec();
                let mut statements = blocks[1].statements().to_vec();
                statements.push(statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(8),
                )));
                blocks[1] = SemanticBasicBlockV1::new(
                    blocks[1].identity(),
                    blocks[1].source(),
                    statements,
                    blocks[1].terminator().clone(),
                )
                .unwrap();
                functions[0] =
                    ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
                materialize_ranked_fixture_v1(
                    assertion_ssa_functions(semantic.types().to_vec(), functions),
                    &[ranked_root_input_1d(A_NAME, 247, 1)],
                )
                .unwrap()
            } else {
                seed
            };
            let mut completed = false;
            let result = with_canonical_control_candidate_test_v1(
                &source,
                profile,
                |claims| {
                    if !killed {
                        claims
                            .arguments
                            .iter_mut()
                            .find(|claim| claim.source_local.index() == 8)
                            .unwrap()
                            .source_local = call_result;
                    }
                },
                |view, candidates, budget| {
                    view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                        completed = true;
                        Ok(())
                    })
                },
            );
            assert!(result.is_err());
            assert!(!completed);
        }
    }
}

#[test]
fn conditional_control_equal_literals_with_distinct_incoming_definitions_remain_unsupported() {
    // This genuinely admitted source has a scalar phi at the guard. Rejection
    // may be at the existing projection definition gate before literal-query
    // admission; it is not evidence for a new phi evaluator.
    let (seed, _) = conditional_literal_source_v1(17, None, false);
    let semantic = seed.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut blocks = root.blocks().to_vec();
    let statements = blocks[2].statements().iter().filter(|statement| {
        !matches!(statement.kind(), SemanticStatementKindV1::Assign(value) if value.destination().local().index() == 8)
    }).cloned().collect();
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        blocks[2].source(),
        statements,
        blocks[2].terminator().clone(),
    )
    .unwrap();
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        blocks[0].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[0].terminator().source(),
            helper_call(1, (root.locals().len() - 1) as u32, 5),
        ),
    )
    .unwrap();
    for identity in [204, 205] {
        blocks.push(block(
            identity,
            vec![typed_assignment(
                8,
                A_U64,
                SemanticRvalueKindV1::Use(typed_constant(A_U64, 17, 8)),
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
        ));
    }
    blocks.push(block(
        206,
        vec![typed_assignment(
            6,
            A_BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: typed_operand(3, A_U32),
                right: typed_constant(A_U32, 0, 4),
            },
        )],
        zero_switch(6, A_BOOL, 4, 3),
    ));
    functions[0] = ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
    let source = materialize_ranked_fixture_v1(
        assertion_ssa_functions(semantic.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(result.is_err());
        assert!(!completed);
    }
}

fn conditional_literal_two_guards_v1() -> ProductionPreRankedKirOwnerV1 {
    let (seed, _) = conditional_literal_source_v1(17, None, false);
    let semantic = seed.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    let length = locals.len() as u32;
    locals.push(local(250, A_U64, SemanticLocalRoleV1::Temporary));
    let condition = locals.len() as u32;
    locals.push(local(251, A_BOOL, SemanticLocalRoleV1::Temporary));
    let mut blocks = root.blocks().to_vec();
    let second_store = blocks[1]
        .statements()
        .iter()
        .map(|statement| {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return statement.clone();
            };
            if assignment.destination().local().index() != 1 {
                return statement.clone();
            }
            SemanticStatementV1::new(
                statement.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(2),
                        assignment.destination().projections().to_vec(),
                        assignment.destination().ty(),
                    )
                    .unwrap(),
                    assignment.value().clone(),
                )),
            )
        })
        .collect();
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        blocks[1].source(),
        blocks[1].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[1].terminator().source(),
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
    )
    .unwrap();
    blocks.push(block(
        204,
        vec![
            typed_assignment(
                length,
                A_U64,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: typed_operand(2, G_POINTER),
                },
            ),
            typed_assignment(
                condition,
                A_BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(8, A_U64),
                    right: typed_operand(length, A_U64),
                },
            ),
        ],
        SemanticTerminatorKindV1::Assert {
            condition: typed_operand(condition, A_BOOL),
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: typed_operand(length, A_U64),
                index: typed_operand(8, A_U64),
            },
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 4),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    ));
    blocks.push(block(205, second_store, SemanticTerminatorKindV1::Return));
    functions[0] = ordinary_rebuild_v1(root, root.abi().clone(), locals, blocks);
    materialize_ranked_fixture_v1(
        assertion_ssa_functions(semantic.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap()
}

#[test]
fn conditional_control_one_literal_has_distinct_exact_guard_occurrences() {
    use fe2o3_lower_mir_kernel::ProductionConditionalMemoryIndexLeafV1 as Leaf;
    let source = conditional_literal_two_guards_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(
                    candidates,
                    budget,
                    |coverage, budget| {
                        let candidate = &candidates[0];
                        let claim = candidate
                            .control
                            .arguments
                            .iter()
                            .find(|claim| claim.source_local.index() == 8)
                            .unwrap();
                        let Some(Leaf::Literal(literal)) =
                            coverage.index_leaf(view, candidate, claim.ranked_value, budget)?
                        else {
                            panic!("literal required");
                        };
                        assert_eq!(literal.guard_use_count(), 2);
                        let first = literal.guard_use(0).unwrap();
                        let second = literal.guard_use(1).unwrap();
                        assert_eq!(first.source_guard().index(), 2);
                        assert_eq!(second.source_guard().index(), 3);
                        assert_ne!(first.original_use(), second.original_use());
                        assert_ne!(first.output_use(), second.output_use());
                        assert_eq!(literal.bits(), 17);
                        Ok(())
                    },
                )
            },
        )
        .unwrap();
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |claims| {
                // Same local/value, but the source guard point does not belong to
                // this actual projected boundary. Generic partition checks and
                // the literal occurrence relation both remain mandatory.
                let guard = claims
                    .blocks
                    .iter_mut()
                    .find(|row| row.source_block.index() == 2)
                    .unwrap();
                guard.source_block = SemanticBlockIdV1::from_index(3);
            },
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(result.is_err());
        assert!(!completed);
    }
}

#[test]
fn conditional_control_checks_real_guard_calls_and_store_boundaries_on_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for shape in [BodyShape::Straight, BodyShape::Join, BodyShape::Loop] {
            let source = canonical_same_typed_tuple_store_source_v1(1, shape);
            let mut completed = false;
            with_canonical_control_candidate_test_v1(
                &source,
                profile,
                |_| {},
                |view, candidates, budget| {
                    let floor = budget.storage();
                    let prior = budget.work();
                    view.with_conditional_memory_control_coverage_v1(
                        candidates,
                        budget,
                        |coverage, budget| {
                            assert!(std::ptr::eq(coverage.output(), view.output()));
                            let [candidate] = candidates else {
                                panic!("one genuine root");
                            };
                            assert!(!candidate.access_sources.is_empty());
                            assert!(
                                view.output().module().functions[0]
                                    .body
                                    .as_ref()
                                    .unwrap()
                                    .blocks
                                    .iter()
                                    .flat_map(|block| &block.operations)
                                    .any(|operation| matches!(
                                        operation.kind,
                                        fe2o3_kernel_ir::OperationKind::Call { .. }
                                    ))
                            );
                            for claim in &candidate.control.arguments {
                                let anchor = coverage
                                    .index_value(view, candidate, claim.ranked_value, budget)?
                                    .unwrap();
                                assert_eq!(anchor.source_local(), claim.source_local);
                                assert_eq!(anchor.component(), claim.component);
                            }
                            completed = true;
                            Ok(())
                        },
                    )?;
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > prior);
                    Ok(())
                },
            )
            .unwrap();
            assert!(completed);
        }
    }
}

#[test]
fn conditional_control_rejects_missing_extra_overlapping_and_wrong_source_segments() {
    for fault in 0..6 {
        let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            Profile::Gfx942,
            |claims| {
                assert!(claims.blocks.len() >= 3);
                match fault {
                    0 => {
                        claims.blocks.pop();
                    }
                    1 => claims.blocks[1].first = claims.blocks[0].first,
                    2 => claims.blocks[0].tail += 1,
                    3 => claims.blocks[0].source_block = SemanticBlockIdV1::from_index(u32::MAX),
                    4 => claims.blocks[0].end += 1,
                    _ => claims.blocks.swap(0, 1),
                }
            },
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(result.is_err(), "fault {fault}");
        assert!(!completed, "fault {fault}");
    }
}

#[test]
fn conditional_control_same_typed_source_swap_cannot_borrow_another_formals_identity() {
    use fe2o3_lower_mir_kernel::ProductionProjectionArgumentComponentV1 as Component;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let source = canonical_same_typed_tuple_store_source_with_extra_index_v1(
            0,
            BodyShape::Straight,
            true,
        );
        let other = SemanticLocalIdV1::from_index(
            (source.semantic_ssa().source_semantic().functions()[0]
                .locals()
                .len()
                - 1) as u32,
        );
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |claims| {
                let claim = claims
                    .arguments
                    .iter_mut()
                    .find(|claim| {
                        claim.component == Component::Scalar && claim.source_local.index() == 8
                    })
                    .unwrap();
                assert_ne!(claim.source_local, other);
                claim.source_local = other;
            },
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(result.is_err());
        assert!(!completed);
    }
}

#[test]
fn conditional_control_rejects_unanchored_wrong_typed_and_missing_unknown_leaves() {
    use fe2o3_lower_mir_kernel::ProductionProjectionArgumentComponentV1 as Component;
    for fault in 0..4 {
        let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            Profile::Gfx942,
            |claims| {
                let slot = claims
                    .arguments
                    .iter()
                    .position(|claim| {
                        claim.component == Component::Scalar
                            && claim.source_local.index() == 8
                            && matches!(claim.ranked_value, ProductionRankedValueV1::Local(_))
                    })
                    .unwrap();
                let original = claims.arguments[slot];
                assert_eq!(
                    claims
                        .arguments
                        .iter()
                        .filter(|claim| **claim == original)
                        .count(),
                    1
                );
                match fault {
                    0 => claims.arguments[slot].source_local = SemanticLocalIdV1::from_index(3), // U32, not the U64 index.
                    1 => claims.arguments[slot].source_local = SemanticLocalIdV1::from_index(7), // metadata temporary, not a formal.
                    2 => {
                        claims.arguments[slot].ranked_value =
                            ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(u32::MAX))
                    }
                    _ => {
                        claims.arguments.remove(slot);
                    }
                }
                assert!(!claims.arguments.contains(&original));
            },
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(result.is_err(), "fault {fault}");
        assert!(!completed);
    }
}

#[test]
fn conditional_control_redefined_formal_and_literal_temporary_remain_fail_closed() {
    // A source-local name is not an SSA definition. Reassigning the actual
    // formal must not grant its subsequent unknown the entry parameter value.
    let seed = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    let semantic = seed.semantic_ssa().source_semantic();
    for literal_temporary in [false, true] {
        let source = if literal_temporary {
            // An equal-valued second definition still breaks the exact source
            // single-definition obligation; this is not value propagation.
            conditional_literal_source_v1(17, None, true).0
        } else {
            let mut functions = semantic.functions().to_vec();
            let root = &functions[0];
            let mut blocks = root.blocks().to_vec();
            let mut statements = blocks[0].statements().to_vec();
            statements.push(typed_assignment(
                8,
                A_U64,
                SemanticRvalueKindV1::Use(typed_constant(A_U64, 3, 8)),
            ));
            blocks[0] = SemanticBasicBlockV1::new(
                blocks[0].identity(),
                blocks[0].source(),
                statements,
                blocks[0].terminator().clone(),
            )
            .unwrap();
            functions[0] =
                ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
            materialize_ranked_fixture_v1(
                assertion_ssa_functions(semantic.types().to_vec(), functions),
                &[ranked_root_input_1d(A_NAME, 247, 1)],
            )
            .unwrap()
        };
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            Profile::Gfx942,
            |_| {},
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(result.is_err());
        assert!(!completed);
    }
}

#[test]
fn conditional_control_callback_error_and_unwind_keep_one_ledger_and_reusable_floor() {
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    with_canonical_control_candidate_test_v1(
        &source,
        Profile::Gfx942,
        |_| {},
        |view, candidates, budget| {
            let floor = budget.storage();
            budget.charge_work(7).unwrap();
            let before = budget.work();
            let error =
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    Err::<(), _>(ProductionSourceOutputErrorV1::Invalid(
                        "control test callback failure",
                    ))
                });
            assert!(matches!(
                error,
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "control test callback failure"
                ))
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before);
            let after_error = budget.work();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                view.with_conditional_memory_control_coverage_v1::<()>(
                    candidates,
                    budget,
                    |_, _| panic!("control test callback panic"),
                )
            }));
            assert!(panic.is_err());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > after_error);
            view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, budget| {
                budget
                    .charge_work(1)
                    .map_err(ProductionSourceOutputErrorV1::Resource)
            })?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn conditional_control_rejects_reconstructed_candidate_and_replacement_ledger() {
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    with_canonical_control_candidate_test_v1(&source, Profile::Gfx942, |_| {}, |view, candidates, budget| {
        view.with_conditional_memory_control_coverage_v1(candidates, budget, |coverage, budget| {
            let original = &candidates[0];
            let changed = fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1 {
                selected_root: original.selected_root, selected_function: original.selected_function,
                lowering: original.lowering, access_sources: original.access_sources,
                executable_effect_sources: original.executable_effect_sources, control: original.control,
            };
            let value = original.control.arguments[0].ranked_value;
            assert!(coverage.index_value(view, &changed, value, budget).is_err());
            let mut replacement_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
            let mut replacement = Budget::new(&mut replacement_work, budget.storage());
            replacement.reserve_storage(budget.storage()).unwrap();
            assert!(matches!(coverage.index_value(view, original, value, &mut replacement), Err(ProductionSourceOutputErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
            ))));
            let live = budget.storage();
            budget.release_storage(1).unwrap();
            assert!(matches!(coverage.index_value(view, original, value, budget), Err(ProductionSourceOutputErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
            ))));
            budget.reserve_storage(1).unwrap();
            assert_eq!(budget.storage(), live);
            assert!(coverage.index_value(view, original, value, budget)?.is_some());
            Ok(())
        })
    }).unwrap();
}

#[test]
fn conditional_control_selected_occurrence_precedes_duplicate_target_and_dead_store_handling() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for selected in [0, 1] {
            for duplicate in [false, true] {
                let seed = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
                let semantic = seed.semantic_ssa().source_semantic();
                let mut functions = semantic.functions().to_vec();
                let root = &functions[0];
                let mut blocks = root.blocks().to_vec();
                assert_eq!(blocks.len(), 3);
                let guard = &blocks[2];
                let SemanticTerminatorKindV1::Assert {
                    condition,
                    expected,
                    message,
                    unwind,
                    ..
                } = guard.terminator().kind()
                else {
                    panic!("genuine dynamic bounds guard required");
                };
                blocks[2] = SemanticBasicBlockV1::new(
                    guard.identity(),
                    guard.source(),
                    guard.statements().to_vec(),
                    SemanticTerminatorV1::new(
                        guard.terminator().source(),
                        SemanticTerminatorKindV1::Assert {
                            condition: condition.clone(),
                            expected: *expected,
                            message: message.clone(),
                            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                            unwind: *unwind,
                        },
                    ),
                )
                .unwrap();
                blocks.push(block(
                    204,
                    vec![typed_assignment(
                        6,
                        A_BOOL,
                        SemanticRvalueKindV1::Use(typed_constant(A_BOOL, selected, 1)),
                    )],
                    zero_switch(6, A_BOOL, 1, if duplicate { 1 } else { 4 }),
                ));
                blocks.push(block(
                    205,
                    blocks[1].statements().to_vec(),
                    SemanticTerminatorKindV1::Return,
                ));
                functions[0] =
                    ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
                let source = materialize_ranked_fixture_v1(
                    assertion_ssa_functions(semantic.types().to_vec(), functions),
                    &[ranked_root_input_1d(A_NAME, 247, 1)],
                )
                .unwrap();
                let mut completed = false;
                with_canonical_control_candidate_test_v1(
                    &source,
                    profile,
                    |_| {},
                    |view, candidates, budget| {
                        let [candidate] = candidates else {
                            panic!("one root");
                        };
                        let live = if duplicate || selected == 0 { 1 } else { 4 };
                        let dead = if live == 1 { 4 } else { 1 };
                        assert!(
                            candidate
                                .access_sources
                                .iter()
                                .all(|site| site.semantic_block() == live)
                        );
                        assert!(
                            !candidate
                                .control
                                .blocks
                                .iter()
                                .any(|row| row.source_block.index() == dead)
                        );
                        view.with_conditional_memory_control_coverage_v1(
                            candidates,
                            budget,
                            |_, _| {
                                completed = true;
                                Ok(())
                            },
                        )
                    },
                )
                .unwrap();
                assert!(completed);
            }
        }
    }
}

#[test]
fn conditional_control_live_failed_assert_never_reaches_dead_store_completion() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let seed = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
        let semantic = seed.semantic_ssa().source_semantic();
        let mut functions = semantic.functions().to_vec();
        let root = &functions[0];
        let mut blocks = root.blocks().to_vec();
        let guard = &blocks[2];
        blocks[2] = SemanticBasicBlockV1::new(
            guard.identity(),
            guard.source(),
            guard.statements().to_vec(),
            SemanticTerminatorV1::new(
                guard.terminator().source(),
                assertion_terminator(typed_constant(A_BOOL, 0, 1), true, 1),
            ),
        )
        .unwrap();
        functions[0] =
            ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
        let source = materialize_ranked_fixture_v1(
            assertion_ssa_functions(semantic.types().to_vec(), functions),
            &[ranked_root_input_1d(A_NAME, 247, 1)],
        )
        .unwrap();
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(
            matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::UnprovenAssert { block: 2, .. })
            ),
            "{result:?}"
        );
        assert!(!completed);
    }
}

#[test]
fn conditional_control_does_not_turn_a_no_return_callee_into_termination_evidence() {
    let seed = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    let semantic = seed.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    let leaf = &functions[2];
    assert_eq!(leaf.blocks().len(), 1);
    functions[2] = ordinary_rebuild_v1(
        leaf,
        leaf.abi().clone(),
        leaf.locals().to_vec(),
        vec![block(
            130,
            leaf.blocks()[0].statements().to_vec(),
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 0)),
        )],
    );
    let source = materialize_ranked_fixture_v1(
        assertion_ssa_functions(semantic.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, candidates, budget| {
                let leaf = &view.output().module().functions[2];
                assert!(
                    leaf.body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .all(|block| !matches!(
                            block.terminator,
                            Some(fe2o3_kernel_ir::Terminator::Return { .. })
                        ))
                );
                view.with_conditional_memory_control_coverage_v1(
                    candidates,
                    budget,
                    |coverage, _| {
                        assert!(std::ptr::eq(coverage.output(), view.output()));
                        // The projection may contain a Return; it remains conditional
                        // on the retained actual Calls returning. No termination API is provided.
                        Ok(())
                    },
                )
            },
        )
        .unwrap();
    }
}

fn conditional_compare_message_drift_source_v1(index_drift: bool) -> ProductionPreRankedKirOwnerV1 {
    let (seed, _) = conditional_literal_source_v1(3, None, false);
    let semantic = seed.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    let alternate = locals.len() as u32;
    locals.push(local(250, A_U64, SemanticLocalRoleV1::Temporary));
    let mut blocks = root.blocks().to_vec();
    let mut statements = Vec::new();
    let mut changed = 0;
    for old in blocks[2].statements() {
        if let SemanticStatementKindV1::Assign(assignment) = old.kind()
            && assignment.destination().local().index() == 9
        {
            changed += 1;
            statements.push(typed_assignment(
                alternate,
                A_U64,
                if index_drift {
                    SemanticRvalueKindV1::Use(typed_constant(A_U64, 5, 8))
                } else {
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: typed_operand(2, G_POINTER),
                    }
                },
            ));
            statements.push(typed_assignment(
                9,
                A_BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(if index_drift { alternate } else { 8 }, A_U64),
                    right: typed_operand(if index_drift { 7 } else { alternate }, A_U64),
                },
            ));
        } else {
            statements.push(old.clone());
        }
    }
    assert_eq!(changed, 1);
    let SemanticTerminatorKindV1::Assert {
        message: SemanticAssertMessageV1::BoundsCheck { length, index },
        ..
    } = blocks[2].terminator().kind()
    else {
        panic!("genuine source bounds assertion required");
    };
    assert_eq!(*length, typed_operand(7, A_U64));
    assert_eq!(*index, typed_operand(8, A_U64));
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        blocks[2].source(),
        statements,
        blocks[2].terminator().clone(),
    )
    .unwrap();
    functions[0] = ordinary_rebuild_v1(root, root.abi().clone(), locals, blocks);
    materialize_ranked_fixture_v1(
        assertion_ssa_functions(semantic.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap()
}

// A fixture-only two-node oracle checks the actual optimized operands. It does
// not use a source message as evidence that the Compare has those operands.
fn conditional_compare_message_drift_output_v1(
    source: &ProductionPreRankedKirOwnerV1,
    output: &Owner,
    index_drift: bool,
) {
    use fe2o3_kernel_ir::{CastKind, ComparePredicate, Constant, OperationKind, Type};
    let original = &source.executable().module().functions[0];
    let body = output.module().functions[0].body.as_ref().unwrap();
    let mut comparisons = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            } => Some((lhs, rhs)),
            _ => None,
        });
    let (lhs, rhs) = comparisons
        .next()
        .expect("actual O Compare must be retained");
    assert!(comparisons.next().is_none());
    let operation = |value| {
        let mut definitions = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.results.iter().any(|result| result.id == value));
        let result = definitions
            .next()
            .expect("actual O operand definition required");
        assert!(definitions.next().is_none());
        result
    };
    let mut left = operation(lhs);
    if let OperationKind::Cast {
        kind: CastKind::Bitcast,
        value,
        ref to,
    } = left.kind
    {
        assert_eq!(*to, Type::INDEX);
        left = operation(value);
    }
    let expected = if index_drift { 5 } else { 3 };
    assert!(matches!(left.kind,
        OperationKind::Constant(Constant::U64(value) | Constant::Index(value)) if value == expected));
    // This fixture has exactly two canonical Slice parameters followed by
    // U32, with no aggregate parameter flattening. This is not a general ABI
    // inference rule or a production correspondence substitute.
    assert!(matches!(
        original.signature.parameters.as_slice(),
        [
            Type::Slice(_),
            Type::Slice(_),
            Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)
        ]
    ));
    assert_eq!(original.signature, output.module().functions[0].signature);
    let source_slice = if index_drift { 1 } else { 2 };
    let ordinal = source_slice - 1;
    assert_eq!(
        source.semantic_ssa().source_semantic().functions()[0].locals()[source_slice].role(),
        SemanticLocalRoleV1::Argument(ordinal as u32)
    );
    assert!(
        matches!(operation(rhs).kind, OperationKind::SliceLength { slice }
        if slice == body.parameters[ordinal])
    );
    assert!(
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::Store { .. }))
    );
}

fn conditional_compare_message_drift_rejects_v1(index_drift: bool) {
    let source = conditional_compare_message_drift_source_v1(index_drift);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |_, checked, _| {
            conditional_compare_message_drift_output_v1(&source, checked.owner(), index_drift);
        });
        let mut completed = false;
        let result = with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, candidates, budget| {
                view.with_conditional_memory_control_coverage_v1(candidates, budget, |_, _| {
                    completed = true;
                    Ok(())
                })
            },
        );
        assert!(
            matches!(&result, Err(ProductionRankedProjectionErrorV1::Incomplete(detail))
            if *detail == "a Rust bounds-check message not backed by its exact index < length condition"),
            "actual Compare/message drift must reach the exact source guard gate: {result:?}"
        );
        assert!(!completed);
    }
}

#[test]
fn conditional_control_actual_compare_index_cannot_drift_from_bounds_message() {
    conditional_compare_message_drift_rejects_v1(true);
}

#[test]
fn conditional_control_actual_compare_extent_cannot_drift_to_another_slice() {
    conditional_compare_message_drift_rejects_v1(false);
}

const INVOCATION_MARKER_V1: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const INVOCATION_WITNESS_V1: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const INVOCATION_REFERENCE_V1: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);

fn invocation_coordinate_ssa_v1() -> ProductionSemanticSsaOwnerV1 {
    let seed = genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::Parameter, 1);
    let semantic = seed.semantic_ssa().source_semantic();
    let old = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    assert_eq!(types.len(), INVOCATION_MARKER_V1.index() as usize);
    let raw = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(239)),
        SemanticLayoutIdentityV1::from_sha256(bytes(239)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(240)),
        SemanticLayoutIdentityV1::from_sha256(bytes(240)),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(raw),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![A_U64, INVOCATION_MARKER_V1]).unwrap(),
        ),
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(241)),
            SemanticLayoutIdentityV1::from_sha256(bytes(241)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    INVOCATION_WITNESS_V1,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        8,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let intrinsic = |tag, operation, inputs, output| {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            inputs,
            neutral_plain_direct_abi_value_v1(output),
        )
        .unwrap();
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
                SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(tag)),
        }
    };
    let reference_argument = SemanticAbiValueV1::new(
        INVOCATION_REFERENCE_V1,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                8,
                Some(8),
            )
            .unwrap(),
        ),
    );
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        intrinsic(
            244,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: INVOCATION_WITNESS_V1,
                raw_index: A_U64,
            },
            vec![],
            INVOCATION_WITNESS_V1,
        ),
        intrinsic(
            245,
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness: INVOCATION_WITNESS_V1,
                raw_index: A_U64,
            },
            vec![reference_argument],
            A_U64,
        ),
    ];
    let mut locals = old.locals().to_vec();
    assert_eq!(locals.len(), 14);
    locals.push(local(
        114,
        INVOCATION_WITNESS_V1,
        SemanticLocalRoleV1::Temporary,
    ));
    locals.push(local(
        115,
        INVOCATION_REFERENCE_V1,
        SemanticLocalRoleV1::Temporary,
    ));
    let call = |callee, destination, ty, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                if callee == 1 {
                    vec![]
                } else {
                    vec![typed_operand(15, INVOCATION_REFERENCE_V1)]
                },
                Some(SemanticCallDestinationV1::new(
                    whole(destination, ty),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let mut blocks = old.blocks().to_vec();
    blocks[0] = block(201, vec![], call(1, 14, INVOCATION_WITNESS_V1, 3));
    let guard = &blocks[2];
    assert!(matches!(guard.statements()[0].kind(),
        SemanticStatementKindV1::Assign(assignment)
        if assignment.destination().local().index() == 8));
    blocks[2] = SemanticBasicBlockV1::new(
        guard.identity(),
        guard.source(),
        guard.statements()[1..].to_vec(),
        guard.terminator().clone(),
    )
    .unwrap();
    blocks.push(block(
        204,
        vec![typed_assignment(
            15,
            INVOCATION_REFERENCE_V1,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: whole(14, INVOCATION_WITNESS_V1),
            },
        )],
        call(2, 8, A_U64, 2),
    ));
    let function = ordinary_rebuild_v1(old, old.abi().clone(), locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn with_invocation_coordinate_source_v1(
    body: impl FnOnce(&ProductionPreRankedKirOwnerV1, &mut Budget<'_>),
) {
    let mut ssa = invocation_coordinate_ssa_v1();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(PREFIX).unwrap();
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let roots = [ranked_root_input_1d(A_NAME, 247, 1)];
    let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &roots).unwrap();
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let bytes = source.executable_storage().retained_storage()
        + source.assert_origin_storage().payload_storage();
    budget.reserve_storage(bytes).unwrap();
    let floor = budget.storage();
    body(&source, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(source);
    budget.release_storage(bytes).unwrap();
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}

#[test]
fn invocation_coordinate_fixture_authenticates_borrow_callreturn_and_actual_n() {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
        ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    with_invocation_coordinate_source_v1(|source, _| {
        let ssa = source.semantic_ssa();
        let rows = ssa.occurrences_v1().unwrap().function(ROOT).unwrap();
        assert!(std::ptr::eq(rows.owner(), ssa));
        let borrow = rows
            .events()
            .iter()
            .filter(|row| {
                row.site()
                    == Site::Statement {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(3),
                        statement: 0,
                    }
                    && row.operand() == Operand::RvaluePlace
                    && row.role() == Role::BaseUse
            })
            .collect::<Vec<_>>();
        assert_eq!(borrow.len(), 1);
        assert!(borrow[0].is_reachable() && borrow[0].is_promoted());
        assert!(
            matches!(borrow[0].resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Use {variable, ..})
            if variable.get() == 14)
        );
        for (block, local) in [(0, 14), (3, 8)] {
            let definitions = rows
                .edge_definitions()
                .iter()
                .filter(|row| row.edge().source().get() == block && row.variable().get() == local)
                .collect::<Vec<_>>();
            assert_eq!(definitions.len(), 1);
            assert!(definitions[0].is_reachable() && definitions[0].is_promoted());
            assert!(definitions[0].value().is_some());
            let successor = rows
                .successors()
                .iter()
                .find(|edge| edge.id() == definitions[0].edge())
                .unwrap();
            assert_eq!(successor.edge().role(), SemanticEdgeRoleV1::CallReturn);
        }
        let body = source.executable().module().functions[0]
            .body
            .as_ref()
            .unwrap();
        let intrinsics = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Intrinsic(value)
                if value.kind == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d().kind)
            })
            .collect::<Vec<_>>();
        assert_eq!(intrinsics.len(), 1);
        assert_eq!(intrinsics[0].results.len(), 1);
        assert_eq!(intrinsics[0].results[0].ty, fe2o3_kernel_ir::Type::INDEX);
        assert!(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(operation.kind, fe2o3_kernel_ir::OperationKind::Store { .. })
                })
        );
    });
}
