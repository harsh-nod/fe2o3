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
    invocation_coordinate_ssa_with_receiver_mode_v1(false)
}

fn invocation_coordinate_ssa_with_receiver_mode_v1(
    moved_receiver: bool,
) -> ProductionSemanticSsaOwnerV1 {
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
                } else if moved_receiver {
                    vec![SemanticOperandV1::Move(whole(15, INVOCATION_REFERENCE_V1))]
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
    with_invocation_coordinate_source_mode_v1(false, body)
}

fn with_invocation_coordinate_source_mode_v1(
    moved_receiver: bool,
    body: impl FnOnce(&ProductionPreRankedKirOwnerV1, &mut Budget<'_>),
) {
    let mut ssa = if moved_receiver {
        invocation_coordinate_ssa_with_receiver_mode_v1(true)
    } else {
        invocation_coordinate_ssa_v1()
    };
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

#[test]
fn invocation_fixture_records_distinct_exact_guard_and_store_index_occurrences() {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
        ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    with_invocation_coordinate_source_v1(|source, _| {
        let rows = source
            .semantic_ssa()
            .occurrences_v1()
            .unwrap()
            .function(ROOT)
            .unwrap();
        for (site, operand, role) in [
            (
                Site::Terminator {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(2),
                },
                Operand::AssertMessage(1),
                Role::BaseUse,
            ),
            (
                Site::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(1),
                    statement: 0,
                },
                Operand::Destination,
                Role::ProjectionIndexUse(1),
            ),
        ] {
            let mut found = rows
                .events()
                .iter()
                .filter(|row| row.site() == site && row.operand() == operand && row.role() == role);
            let row = found
                .next()
                .expect("the exact own-use occurrence must be captured");
            assert!(found.next().is_none());
            assert!(row.is_reachable() && row.is_promoted());
            assert!(
                matches!(row.resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Use { variable, .. })
                if variable.get() == 8)
            );
        }
    });
}

// Source/N/O shape prerequisite only: no ranked/refinement proof is constructed.
mod identity_getter_shape_tests {
    use super::*;
    use fe2o3_kernel_ir::{OperationKind as Op, Terminator as Term, Type, ValueId};
    use fe2o3_mir_model::semantic_mir_v1::*;

    const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
    const WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
    const RAW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
    const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
    const RECEIVER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
    const ELEMENT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
    const OPTION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);

    fn declaration(
        tag: u8,
        layout: SemanticTypeLayoutV1,
        shape: SemanticTypeShapeV1,
    ) -> SemanticTypeDeclV1 {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            layout,
            shape,
        )
    }

    fn pointer_scalar(reference: bool) -> SemanticBackendScalarV1 {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(u128::from(reference), u64::MAX.into()),
        )
    }

    fn pointer_type(
        tag: u8,
        pointee: SemanticTypeIdV1,
        reference: bool,
        size: u64,
        alignment: u64,
    ) -> SemanticTypeDeclV1 {
        declaration(
            tag,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(pointer_scalar(reference)),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee,
                    if reference {
                        SemanticPointerKindV1::Reference
                    } else {
                        SemanticPointerKindV1::Raw
                    },
                    SemanticMutabilityV1::Mutable,
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
                        if reference {
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                        } else {
                            SemanticAbiPointeeKindV1::Raw
                        },
                        size,
                        alignment,
                    )
                    .unwrap(),
                ),
                None,
            ),
        )
    }

    fn types(discriminator: SemanticTypeIdV1) -> Vec<SemanticTypeDeclV1> {
        let mut types = assertion_types();
        let integer = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        types.push(declaration(
            237,
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ));
        types.push(declaration(
            238,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(integer),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![A_U64, MARKER]).unwrap(),
            ),
        ));
        types.push(pointer_type(239, A_U32, false, 0, 1));
        types.push(
            declaration(
                240,
                SemanticTypeLayoutV1::aggregate_with_backend_repr(
                    Some(16),
                    8,
                    SemanticBackendReprV1::ScalarPair {
                        first: pointer_scalar(false),
                        second: integer,
                    },
                    false,
                    SemanticAggregateLayoutV1::new(vec![0, 8, 0], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![RAW, A_U64, MARKER]).unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                    None,
                ),
            ),
        );
        types.push(pointer_type(241, SLICE, true, 16, 8));
        types.push(pointer_type(242, A_U32, true, 4, 4));
        let niche = SemanticLayoutNicheV1::new(
            0,
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )
        .unwrap();
        let nullable = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, 0),
        );
        let variant = |index, offsets: Vec<u64>, repr, niche| {
            let order = (0..offsets.len() as u32).collect();
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                8,
                8,
                SemanticFieldsShapeV1::arbitrary(offsets.clone(), order).unwrap(),
                repr,
                niche,
                false,
                None,
                8,
                u64::from(index),
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        };
        types.push(
            declaration(
                243,
                SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                    8,
                    8,
                    SemanticBackendReprV1::scalar(nullable),
                    false,
                    SemanticEnumLayoutV1::new(
                        vec![
                            variant(0, vec![], SemanticBackendReprV1::memory(true), None),
                            variant(
                                1,
                                vec![0],
                                SemanticBackendReprV1::scalar(pointer_scalar(true)),
                                Some(niche),
                            ),
                        ],
                        SemanticEnumEncodingV1::Niche(
                            SemanticNicheEnumEncodingV1::new(
                                0,
                                SemanticNicheSourceV1::new(
                                    vec![SemanticNichePathComponentV1::Field(0)],
                                    0,
                                )
                                .unwrap(),
                                niche,
                                nullable,
                                1,
                                0,
                                0,
                                0,
                            )
                            .unwrap(),
                        ),
                    )
                    .unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::enum_type(
                    discriminator,
                    vec![
                        SemanticEnumVariantV1::new(
                            0,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            1,
                            SemanticAggregateTypeV1::new(vec![ELEMENT_REF]).unwrap(),
                        ),
                    ],
                )
                .unwrap(),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                            0,
                            4,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
        assert_eq!(types.len(), 14);
        types
    }

    fn attributes(
        reference: bool,
        size: u64,
        alignment: Option<u64>,
    ) -> SemanticAbiValueAttributesV1 {
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(reference, None, reference, false, false, true),
            SemanticAbiExtensionV1::None,
            size,
            alignment,
        )
        .unwrap()
    }

    fn source_ssa(value: u32) -> ProductionSemanticSsaOwnerV1 {
        source_ssa_mode(value, false)
    }

    pub(super) fn source_ssa_mode(
        value: u32,
        collected_modes: bool,
    ) -> ProductionSemanticSsaOwnerV1 {
        source_ssa_options(value, collected_modes, 1)
    }

    pub(super) fn source_ssa_options(
        value: u32,
        collected_modes: bool,
        stores: usize,
    ) -> ProductionSemanticSsaOwnerV1 {
        let discriminator = if collected_modes {
            SemanticTypeIdV1::from_index(14)
        } else {
            A_U64
        };
        let mut source_types = types(discriminator);
        if collected_modes {
            assert_eq!(source_types.len(), 14);
            source_types.push(declaration(
                244,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(true, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: true,
                    bits: 64,
                }),
            ));
        }
        let call = |callee, arguments, destination, ty, target| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        whole(destination, ty),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        let payload = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), OPTION).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ELEMENT_REF).unwrap(),
            ],
            ELEMENT_REF,
        )
        .unwrap();
        let destination = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(6),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, A_U32).unwrap()],
            A_U32,
        )
        .unwrap();
        let mut blocks = vec![
            block(191, vec![], call(1, vec![], 2, WITNESS, 1)),
            block(
                192,
                vec![typed_assignment(
                    3,
                    RECEIVER,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: whole(1, SLICE),
                    },
                )],
                call(
                    2,
                    vec![
                        SemanticOperandV1::Move(whole(3, RECEIVER)),
                        if collected_modes {
                            typed_operand(2, WITNESS)
                        } else {
                            SemanticOperandV1::Move(whole(2, WITNESS))
                        },
                    ],
                    4,
                    OPTION,
                    2,
                ),
            ),
            block(
                193,
                vec![typed_assignment(
                    5,
                    discriminator,
                    SemanticRvalueKindV1::Discriminant(whole(4, OPTION)),
                )],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: if collected_modes {
                        SemanticOperandV1::Move(whole(5, discriminator))
                    } else {
                        typed_operand(5, discriminator)
                    },
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 4),
                            ),
                            SemanticSwitchTargetV1::new(
                                1,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            ),
                        ],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                    )
                    .unwrap(),
                },
            ),
            block(
                194,
                vec![
                    typed_assignment(
                        6,
                        ELEMENT_REF,
                        SemanticRvalueKindV1::Use(if collected_modes {
                            SemanticOperandV1::Copy(payload)
                        } else {
                            SemanticOperandV1::Move(payload)
                        }),
                    ),
                    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        destination,
                        SemanticRvalueV1::new(
                            A_U32,
                            SemanticRvalueKindV1::Use(typed_constant(A_U32, value.into(), 4)),
                        ),
                    ))),
                ],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(195, vec![], SemanticTerminatorKindV1::Return),
            block(196, vec![], SemanticTerminatorKindV1::Unreachable),
        ];
        assert!(stores == 1 || stores == 2);
        if stores == 2 {
            let mut statements = blocks[3].statements().to_vec();
            statements.push(statements[1].clone());
            blocks[3] = SemanticBasicBlockV1::new(
                blocks[3].identity(),
                blocks[3].source(),
                statements,
                blocks[3].terminator().clone(),
            )
            .unwrap();
        }
        let root = assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (SLICE, SemanticLocalRoleV1::Argument(0)),
                (WITNESS, SemanticLocalRoleV1::Temporary),
                (RECEIVER, SemanticLocalRoleV1::Temporary),
                (OPTION, SemanticLocalRoleV1::Temporary),
                (discriminator, SemanticLocalRoleV1::Temporary),
                (ELEMENT_REF, SemanticLocalRoleV1::Temporary),
            ],
            vec![SLICE],
            blocks,
            false,
        );
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(188)),
            SemanticLayoutIdentityV1::from_sha256(bytes(188)),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SLICE,
                SemanticAbiPassModeV1::Pair {
                    first: attributes(false, 0, None),
                    second: attributes(false, 0, None),
                },
            ))],
            SemanticAbiValueV1::new(A_UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ExclusiveOwner])
        .unwrap();
        let mut root =
            ordinary_rebuild_v1(&root, abi, root.locals().to_vec(), root.blocks().to_vec());
        if collected_modes {
            let entry = root.kernel_entry().unwrap();
            let changed = SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"other_identity_entry".to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(246)),
                entry.source_contract(),
            );
            root = root.with_kernel_entry(changed);
        }
        let intrinsic =
            |tag, inputs, output, operation| SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
                    SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
                    SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticFunctionAbiV1::new(
                        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
                        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
                        SemanticCanonAbiV1::Rust,
                        false,
                        false,
                        inputs,
                        output,
                    )
                    .unwrap(),
                ),
                operation,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(tag)),
            };
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
            source_types,
            vec![],
            vec![],
            vec![],
            vec![root],
            vec![
                SemanticCallableDeclV1::defined(ROOT),
                intrinsic(
                    189,
                    vec![],
                    neutral_plain_direct_abi_value_v1(WITNESS),
                    SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                        index_witness: WITNESS,
                        raw_index: A_U64,
                    },
                ),
                intrinsic(
                    190,
                    vec![
                        SemanticAbiValueV1::new(
                            RECEIVER,
                            SemanticAbiPassModeV1::Direct(attributes(true, 16, Some(8))),
                        ),
                        neutral_plain_direct_abi_value_v1(WITNESS),
                    ],
                    SemanticAbiValueV1::new(
                        OPTION,
                        SemanticAbiPassModeV1::Direct(attributes(false, 0, Some(4))),
                    ),
                    SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                        disjoint_slice: SLICE,
                        index_witness: WITNESS,
                        element: A_U32,
                        raw_index: A_U64,
                    },
                ),
            ],
            vec![ROOT],
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
    pub(super) fn redefined_witness_source() -> ProductionSemanticSsaOwnerV1 {
        let seed = source_ssa(53);
        let semantic = seed.source_semantic();
        let root = &semantic.functions()[0];
        let mut blocks = root.blocks().to_vec();
        let SemanticTerminatorKindV1::Call(call) = blocks[0].terminator().kind() else {
            unreachable!()
        };
        let destination = call.destination().unwrap();
        let first = SemanticDirectCallV1::new_callable(
            call.callee(),
            vec![],
            Some(SemanticCallDestinationV1::new(
                destination.place().clone(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 6),
            )),
            call.unwind(),
        )
        .unwrap();
        let second = blocks[0].terminator().kind().clone();
        blocks[0] = block(191, vec![], SemanticTerminatorKindV1::Call(first));
        blocks.push(block(197, vec![], second));
        let root = ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
            semantic.types().to_vec(),
            vec![],
            vec![],
            vec![],
            vec![root],
            semantic.callables().to_vec(),
            vec![ROOT],
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

    fn captured_source(source: &ProductionPreRankedKirOwnerV1) {
        use fe2o3_pliron::{
            ProductionSemanticSsaEventRoleV1 as Role,
            ProductionSemanticSsaOccurrenceSiteV1 as Site,
            ProductionSemanticSsaOperandRoleV1 as Operand,
        };
        let ssa = source.semantic_ssa();
        let rows = ssa.occurrences_v1().unwrap().function(ROOT).unwrap();
        assert!(std::ptr::eq(rows.owner(), ssa));
        for (block, local) in [(0, 2), (1, 4)] {
            let definitions = rows
                .edge_definitions()
                .iter()
                .filter(|row| row.edge().source().get() == block && row.variable().get() == local)
                .collect::<Vec<_>>();
            assert_eq!(definitions.len(), 1);
            let definition = definitions[0];
            assert!(definition.is_reachable() && definition.is_promoted());
            assert!(definition.value().is_some());
            assert_eq!(definition.edge().ordinal(), 0);
            let edge = rows
                .successors()
                .iter()
                .find(|row| row.id() == definition.edge())
                .unwrap();
            assert_eq!(edge.edge().role(), SemanticEdgeRoleV1::CallReturn);
            assert_eq!(edge.edge().target().index(), block + 1);
        }
        for (site, operand, local) in [
            (
                Site::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(1),
                    statement: 0,
                },
                Operand::RvaluePlace,
                1,
            ),
            (
                Site::Terminator {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(1),
                },
                Operand::CallArgument(0),
                3,
            ),
            (
                Site::Terminator {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(1),
                },
                Operand::CallArgument(1),
                2,
            ),
            (
                Site::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(2),
                    statement: 0,
                },
                Operand::RvaluePlace,
                4,
            ),
            (
                Site::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(3),
                    statement: 0,
                },
                Operand::RvalueOperand(0),
                4,
            ),
            (
                Site::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(3),
                    statement: 1,
                },
                Operand::Destination,
                6,
            ),
        ] {
            let events = rows
                .events()
                .iter()
                .filter(|row| {
                    row.site() == site && row.operand() == operand && row.role() == Role::BaseUse
                })
                .collect::<Vec<_>>();
            assert_eq!(events.len(), 1, "exact source use {site:?}/{operand:?}");
            assert!(events[0].is_reachable() && events[0].is_promoted());
            assert!(
                matches!(events[0].resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Use { variable, .. })
                if variable.get() == local)
            );
        }
        for argument in [0, 1] {
            assert_eq!(
                rows.events()
                    .iter()
                    .filter(|row| row.site()
                        == Site::Terminator {
                            block: fe2o3_mir_model::SsaBlockIdV1::new(1),
                        }
                        && row.operand() == Operand::CallArgument(argument)
                        && row.role() == Role::MoveKill)
                    .count(),
                1
            );
        }
        let function = &ssa.source_semantic().functions()[0];
        let SemanticTerminatorKindV1::SwitchInt { targets, .. } =
            function.blocks()[2].terminator().kind()
        else {
            panic!("source Option discriminant switch is absent");
        };
        assert_eq!(
            targets
                .values()
                .iter()
                .map(|row| (row.value(), row.edge().target().index()))
                .collect::<Vec<_>>(),
            vec![(0, 4), (1, 3)]
        );
        assert_eq!(targets.otherwise().target().index(), 5);
        assert!(matches!(
            function.blocks()[5].terminator().kind(),
            SemanticTerminatorKindV1::Unreachable
        ));
        let SemanticStatementKindV1::Assign(payload) = function.blocks()[3].statements()[0].kind()
        else {
            panic!("payload assignment absent");
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) = payload.value().kind()
        else {
            panic!("payload Move absent");
        };
        assert_eq!(place.local().index(), 4);
        assert_eq!(
            place
                .projections()
                .iter()
                .map(|projection| projection.kind())
                .collect::<Vec<_>>(),
            vec![
                SemanticProjectionKindV1::Downcast(1),
                SemanticProjectionKindV1::Field(0)
            ]
        );
    }

    fn defining(
        body: &fe2o3_kernel_ir::FunctionBody,
        value: ValueId,
    ) -> &fe2o3_kernel_ir::Operation {
        let mut found = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.results.iter().any(|result| result.id == value));
        let operation = found.next().expect("actual result definition is absent");
        assert!(found.next().is_none());
        operation
    }

    // Test oracle for this acyclic fixture's identity edge transport only.
    fn origin(body: &fe2o3_kernel_ir::FunctionBody, value: ValueId, depth: usize) -> ValueId {
        assert!(depth <= body.blocks.len());
        let parameter = body.blocks.iter().find_map(|block| {
            block
                .parameters
                .iter()
                .position(|parameter| parameter.id == value)
                .map(|slot| (block.id, slot))
        });
        let Some((target, slot)) = parameter else {
            return value;
        };
        let mut incoming = Vec::new();
        for block in &body.blocks {
            let mut edge = |to, arguments: &[ValueId]| {
                if to == target {
                    incoming.push(arguments[slot]);
                }
            };
            match block.terminator.as_ref().unwrap() {
                Term::Branch { target, arguments } => edge(*target, arguments),
                Term::ConditionalBranch {
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                    ..
                } => {
                    edge(*then_target, then_arguments);
                    edge(*else_target, else_arguments);
                }
                Term::Switch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                } => {
                    for case in cases {
                        edge(case.target, &case.arguments);
                    }
                    edge(*default_target, default_arguments);
                }
                Term::Return { .. } | Term::Unreachable => {}
                Term::IntegerSwitch { .. } => panic!("unexpected typed switch in fixture"),
            }
        }
        assert_eq!(
            incoming.len(),
            1,
            "fixture transport must have one exact incoming edge"
        );
        origin(body, incoming[0], depth + 1)
    }

    pub(super) fn branch_path_shape(
        body: &fe2o3_kernel_ir::FunctionBody,
        mut target: fe2o3_kernel_ir::BlockId,
    ) -> (usize, bool) {
        let mut stores = 0;
        for _ in 0..body.blocks.len() {
            let block = body.blocks.iter().find(|block| block.id == target).unwrap();
            stores += block
                .operations
                .iter()
                .filter(|operation| matches!(operation.kind, Op::Store { .. }))
                .count();
            match block.terminator.as_ref().unwrap() {
                Term::Branch { target: next, .. } => target = *next,
                Term::Return { .. } => return (stores, false),
                Term::Unreachable => return (stores, true),
                other => panic!("unexpected conditional fixture continuation: {other:?}"),
            }
        }
        panic!("fixture continuation is cyclic");
    }

    pub(super) fn selected_offset_shape(
        body: &fe2o3_kernel_ir::FunctionBody,
        offset: ValueId,
    ) -> (ValueId, ValueId) {
        let selected = defining(body, offset);
        assert_eq!(selected.results.len(), 1);
        assert_eq!(selected.results[0].id, offset);
        assert_eq!(selected.results[0].ty, Type::INDEX);
        let Op::Select {
            condition,
            true_value,
            false_value,
        } = selected.kind
        else {
            panic!("own GEP offset must be the preserved Select")
        };
        assert_ne!(offset, true_value);
        assert_ne!(true_value, false_value);
        let zero = defining(body, false_value);
        assert_eq!(zero.results.len(), 1);
        assert_eq!(zero.results[0].id, false_value);
        assert_eq!(zero.results[0].ty, Type::INDEX);
        assert!(matches!(
            zero.kind,
            Op::Constant(fe2o3_kernel_ir::Constant::Index(0))
        ));
        let raw = defining(body, true_value);
        assert_eq!(raw.results.len(), 1);
        assert_eq!(raw.results[0].ty, Type::INDEX);
        assert!(matches!(&raw.kind, Op::Intrinsic(intrinsic)
            if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d()));
        let compare = defining(body, condition);
        assert_eq!(compare.results.len(), 1);
        assert_eq!(compare.results[0].ty, Type::BOOL);
        let Op::Compare {
            predicate: fe2o3_kernel_ir::ComparePredicate::LessThan,
            lhs,
            rhs,
        } = compare.kind
        else {
            panic!("selected offset must use its exact getter bound")
        };
        assert_eq!(lhs, true_value);
        let length = defining(body, rhs);
        assert_eq!(length.results.len(), 1);
        assert_eq!(length.results[0].ty, Type::INDEX);
        assert!(matches!(length.kind, Op::SliceLength { slice } if slice == body.parameters[0]));
        (condition, true_value)
    }

    fn graph_shape(owner: &Owner, value: u32, label: &str) -> (ValueId, ValueId) {
        let module = owner.module();
        assert_eq!(module.kernels.len(), 1);
        assert_eq!(module.functions.len(), 1);
        let function = &module.functions[0];
        assert_eq!(function.role, fe2o3_kernel_ir::FunctionRole::KernelEntry);
        assert_eq!(function.signature.parameters.len(), 1);
        assert!(
            matches!(&function.signature.parameters[0], Type::Slice(slice)
            if slice.element.as_ref() == &Type::Scalar(fe2o3_kernel_ir::ScalarType::U32))
        );
        let body = function.body.as_ref().unwrap();
        let operations = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        let single = |predicate: fn(&Op) -> bool| {
            let found = operations
                .iter()
                .copied()
                .filter(|op| predicate(&op.kind))
                .collect::<Vec<_>>();
            assert_eq!(
                found.len(),
                1,
                "{label}: required actual operation multiplicity"
            );
            found[0]
        };
        assert!(
            !operations
                .iter()
                .any(|operation| matches!(operation.kind, Op::Call { .. }))
        );
        let global = single(
            |op| matches!(op, Op::Intrinsic(intrinsic) if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d()),
        );
        assert_eq!(global.results.len(), 1);
        assert_eq!(global.results[0].ty, Type::INDEX);
        let length = single(|op| matches!(op, Op::SliceLength { .. }));
        let Op::SliceLength { slice } = length.kind else {
            unreachable!()
        };
        assert_eq!(origin(body, slice, 0), body.parameters[0]);
        let compare = single(|op| {
            matches!(
                op,
                Op::Compare {
                    predicate: fe2o3_kernel_ir::ComparePredicate::LessThan,
                    ..
                }
            )
        });
        let Op::Compare { lhs, rhs, .. } = compare.kind else {
            unreachable!()
        };
        assert_eq!(origin(body, lhs, 0), global.results[0].id);
        assert_eq!(origin(body, rhs, 0), length.results[0].id);
        assert_eq!(compare.results[0].ty, Type::BOOL);
        let data = single(|op| matches!(op, Op::SliceData { .. }));
        let Op::SliceData { slice } = data.kind else {
            unreachable!()
        };
        assert_eq!(origin(body, slice, 0), body.parameters[0]);
        let gep = single(|op| matches!(op, Op::GetElementPointer { .. }));
        let Op::GetElementPointer { base, offset } = gep.kind else {
            unreachable!()
        };
        assert_eq!(origin(body, base, 0), data.results[0].id);
        let selected = single(|op| matches!(op, Op::Select { .. }));
        assert_eq!(offset, selected.results[0].id);
        let (condition, true_value) = selected_offset_shape(body, offset);
        assert_eq!(condition, compare.results[0].id);
        assert_eq!(true_value, global.results[0].id);
        assert!(matches!(&gep.results[0].ty, Type::Pointer(pointer)
            if pointer.pointee.as_ref() == &Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)
            && pointer.address_space == fe2o3_kernel_ir::AddressSpace::Global));
        let store = single(|op| matches!(op, Op::Store { .. }));
        let Op::Store {
            pointer,
            value: stored,
            ..
        } = store.kind
        else {
            unreachable!()
        };
        assert_eq!(origin(body, pointer, 0), gep.results[0].id);
        assert!(matches!(defining(body, origin(body, stored, 0)).kind,
            Op::Constant(fe2o3_kernel_ir::Constant::U32(actual)) if actual == value));
        let mut switches = 0;
        for block in &body.blocks {
            match block.terminator.as_ref().unwrap() {
                Term::Switch {
                    selector,
                    cases,
                    default_target,
                    ..
                } => {
                    switches += 1;
                    assert_eq!(
                        cases.iter().map(|case| case.value).collect::<Vec<_>>(),
                        vec![0, 1]
                    );
                    let cast = defining(body, origin(body, *selector, 0));
                    assert!(
                        matches!(&cast.kind, Op::Cast { kind: fe2o3_kernel_ir::CastKind::ZeroExtend, value, to }
                        if *to == Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)
                        && origin(body, *value, 0) == compare.results[0].id)
                    );
                    assert_eq!(branch_path_shape(body, cases[0].target), (0, false));
                    assert_eq!(branch_path_shape(body, cases[1].target), (1, false));
                    assert_eq!(branch_path_shape(body, *default_target), (0, true));
                    eprintln!("identity-getter {label}: {:?}", block.terminator);
                }
                Term::ConditionalBranch {
                    condition,
                    then_target,
                    else_target,
                    ..
                } => {
                    switches += 1;
                    assert_eq!(origin(body, *condition, 0), compare.results[0].id);
                    assert_eq!(branch_path_shape(body, *then_target), (1, false));
                    assert_eq!(branch_path_shape(body, *else_target), (0, false));
                    eprintln!("identity-getter {label}: {:?}", block.terminator);
                }
                _ => {}
            }
        }
        assert_eq!(switches, 1, "{label}: exact live Option selector");
        (compare.results[0].id, gep.results[0].id)
    }

    #[test]
    fn identity_getter_source_capture_and_actual_policy3_shape_checkpoint() {
        for value in [17, 29] {
            let mut ssa = source_ssa(value);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget.reserve_storage(PREFIX).unwrap();
            let capture = ssa
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            budget.reserve_storage(capture.retained_storage()).unwrap();
            let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
            let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs).unwrap();
            let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                ssa,
                launch,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .unwrap();
            let source_bytes = source.executable_storage().retained_storage()
                + source.assert_origin_storage().payload_storage();
            budget.reserve_storage(source_bytes).unwrap();
            captured_source(&source);
            let (present, pointer) = graph_shape(source.executable(), value, "N");
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let floor = budget.storage();
                let bound = dialect_amdgcn::bind_production_target_v1(
                    source.executable().module(),
                    profile,
                )
                .unwrap();
                let (input, input_storage) = Owner::from_module_ref_with_verification_budget_v12(
                    bound.module(),
                    &mut budget,
                )
                .unwrap();
                budget
                    .reserve_storage(input_storage.retained_storage())
                    .unwrap();
                graph_shape(&input, value, "B");
                let observed =
                    fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut budget)
                        .unwrap();
                budget
                    .reserve_storage(observed.storage().retained_storage())
                    .unwrap();
                let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
                let output_storage = checked.storage().retained_storage();
                budget.reserve_storage(output_storage).unwrap();
                let (_, output_pointer) = graph_shape(checked.owner(), value, "O");
                let coordinate_bytes;
                {
                    let (coordinates, storage) =
                        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                            source.executable(),
                            &input,
                            profile,
                            &mut budget,
                        )
                        .unwrap();
                    coordinate_bytes = storage.retained_storage();
                    budget.reserve_storage(coordinate_bytes).unwrap();
                    let (view, storage) =
                        fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                            &source,
                            &coordinates,
                            &checked,
                            &mut budget,
                        )
                        .unwrap();
                    budget.reserve_storage(storage.retained_storage()).unwrap();
                    let fe2o3_lower_mir_kernel::ProductionSourceOutputBlockV1::Materialized {
                        original,
                        ..
                    } = view
                        .block(ROOT, ROOT, SemanticBlockIdV1::from_index(1), &mut budget)
                        .unwrap()
                    else {
                        panic!("source getter N block is absent");
                    };
                    let body = source.executable().module().functions[original.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap();
                    let block = &body.blocks[original.block as usize];
                    let Term::Branch { target, arguments } = block.terminator.as_ref().unwrap()
                    else {
                        panic!("getter CallReturn did not lower to its actual N branch");
                    };
                    let fe2o3_lower_mir_kernel::ProductionSourceOutputBlockV1::Materialized {
                        original: continuation,
                        ..
                    } = view
                        .block(ROOT, ROOT, SemanticBlockIdV1::from_index(2), &mut budget)
                        .unwrap()
                    else {
                        panic!("source discriminant N block is absent");
                    };
                    assert_eq!(continuation.function, original.function);
                    assert_eq!(*target, body.blocks[continuation.block as usize].id);
                    let target = body
                        .blocks
                        .iter()
                        .find(|block| block.id == *target)
                        .unwrap();
                    assert_eq!(arguments.len(), target.parameters.len());
                    let plan = source
                        .semantic_ssa()
                        .plan_for_function(ROOT)
                        .unwrap()
                        .plan();
                    let planned_arguments = plan
                        .edge_arguments(fe2o3_mir_model::SsaEdgeIdV1::new(
                            fe2o3_mir_model::SsaBlockIdV1::new(1),
                            0,
                        ))
                        .unwrap();
                    if arguments.is_empty() {
                        assert!(planned_arguments.is_empty());
                        assert!(
                            plan.transport_variables(fe2o3_mir_model::SsaBlockIdV1::new(2))
                                .unwrap()
                                .is_empty()
                        );
                    }
                    let option_is_transported = planned_arguments
                        .iter()
                        .any(|argument| argument.variable().get() == 4);
                    // Dominating bindings need no physical block-parameter tuple.
                    for (value, ty) in [
                        (present, Type::BOOL),
                        (pointer, defining(body, pointer).results[0].ty.clone()),
                    ] {
                        assert!(block.operations.iter().any(|operation| {
                            operation
                                .results
                                .iter()
                                .any(|result| result.id == value && result.ty == ty)
                        }));
                        let mut transported = 0;
                        for (argument, parameter) in arguments.iter().zip(&target.parameters) {
                            if origin(body, *argument, 0) == value {
                                assert_eq!(parameter.ty, ty);
                                assert_eq!(origin(body, parameter.id, 0), value);
                                transported += 1;
                            }
                        }
                        if option_is_transported {
                            assert_eq!(transported, 1);
                        }
                    }
                    let Term::Switch { selector, .. } = target.terminator.as_ref().unwrap() else {
                        panic!("source discriminant N continuation lacks its integer switch");
                    };
                    let discriminant = defining(body, origin(body, *selector, 0));
                    assert!(
                        target
                            .operations
                            .iter()
                            .any(|operation| std::ptr::eq(operation, discriminant))
                    );
                    assert!(matches!(&discriminant.kind,
                        Op::Cast { kind: fe2o3_kernel_ir::CastKind::ZeroExtend, value, to }
                        if origin(body, *value, 0) == present
                            && *to == Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)));
                    let fe2o3_lower_mir_kernel::ProductionSourceOutputGlobalAccessV1::Retained {
                        original,
                        operation,
                        source_argument,
                        value: store_value,
                        result,
                        executable,
                        ..
                    } = view
                        .global_access(ROOT, ROOT, 3, Some(1), 0, &mut budget)
                        .unwrap()
                    else {
                        panic!("the own source payload Store has no retained N/O occurrence");
                    };
                    assert_eq!(source_argument, 0);
                    assert!(store_value.is_some() && result.is_none() && executable);
                    for (owner, coordinate, expected_pointer) in [
                        (source.executable(), original, pointer),
                        (checked.owner(), operation, output_pointer),
                    ] {
                        let body = owner.module().functions[coordinate.block.function.0 as usize]
                            .body
                            .as_ref()
                            .unwrap();
                        let store = &body.blocks[coordinate.block.block as usize].operations
                            [coordinate.operation as usize];
                        let Op::Store { pointer, .. } = store.kind else {
                            panic!("exact source Store occurrence did not name an actual Store");
                        };
                        assert_eq!(origin(body, pointer, 0), expected_pointer);
                    }
                    let live = budget.storage();
                    let query_bytes;
                    {
                        let catalog = view.input_pipeline_catalog(&mut budget).unwrap();
                        let (inventory, inventory_storage) =
                            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(
                                source.executable(),
                                &mut budget,
                            )
                            .unwrap();
                        budget
                            .reserve_storage(inventory_storage.retained_storage())
                            .unwrap();
                        let (catalog, catalog_storage) =
                            fe2o3_kernel_analysis::check_kernel_ir_contract_catalog_v1(
                                &inventory,
                                catalog,
                                &mut budget,
                            )
                            .unwrap();
                        budget
                            .reserve_storage(catalog_storage.retained_storage())
                            .unwrap();
                        let (replayed, replay_storage) = fe2o3_lower_mir_kernel::check_supplied_native_materialization_consistency_v1(
                            source.semantic_ssa(), source.source_launch(), source.executable(), &catalog,
                            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(), &mut budget).unwrap();
                        budget
                            .reserve_storage(replay_storage.retained_storage())
                            .unwrap();
                        query_bytes = inventory_storage.retained_storage()
                            + catalog_storage.retained_storage()
                            + replay_storage.retained_storage();
                        assert!(!replayed.grants_authority());
                    }
                    budget.release_storage(query_bytes).unwrap();
                    assert_eq!(budget.storage(), live);
                    drop(view);
                    budget.release_storage(storage.retained_storage()).unwrap();
                }
                budget.release_storage(coordinate_bytes).unwrap();
                drop(checked);
                budget.release_storage(output_storage).unwrap();
                drop(input);
                budget
                    .release_storage(input_storage.retained_storage())
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            }
            drop(source);
            budget.release_storage(source_bytes).unwrap();
            budget.release_storage(capture.retained_storage()).unwrap();
            assert_eq!(budget.storage(), PREFIX);
        }
    }
}

mod identity_getter_completed_d_tests {
    use super::*;
    use fe2o3_lower_mir_kernel::{
        ProductionCanonicalMemoryAnalysisCandidateV1 as Candidate,
        ProductionScopedCanonicalStoreAnalysisV1 as Analysis,
    };

    fn with_fixture(
        value: u32,
        collected_modes: bool,
        profile: Profile,
        body: impl FnOnce(
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut CanonicalMemoryProjectedRootV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        with_fixture_stores(value, collected_modes, profile, 1, body)
    }

    fn with_fixture_stores(
        value: u32,
        collected_modes: bool,
        profile: Profile,
        stores: usize,
        body: impl FnOnce(
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut CanonicalMemoryProjectedRootV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let ssa = identity_getter_shape_tests::source_ssa_options(value, collected_modes, stores);
        with_admitted_fixture(ssa, collected_modes, profile, body)
    }

    fn with_admitted_fixture(
        mut ssa: ProductionSemanticSsaOwnerV1,
        collected_modes: bool,
        profile: Profile,
        body: impl FnOnce(
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut CanonicalMemoryProjectedRootV1,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.reserve_storage(PREFIX).unwrap();
        let capture = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let inputs = [ranked_root_input_1d(
            if collected_modes {
                "other_identity_entry"
            } else {
                A_NAME
            },
            if collected_modes { 246 } else { 247 },
            64,
        )];
        let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs).unwrap();
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let source_bytes = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(source_bytes).unwrap();
        let source_floor = budget.storage();
        let result;
        {
            let bound =
                dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile)
                    .unwrap();
            let (input, storage) =
                Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                    .unwrap();
            let input_bytes = storage.retained_storage();
            budget.reserve_storage(input_bytes).unwrap();
            let observed =
                fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut budget)
                    .unwrap();
            budget
                .reserve_storage(observed.storage().retained_storage())
                .unwrap();
            let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
            let output_bytes = checked.storage().retained_storage();
            budget.reserve_storage(output_bytes).unwrap();
            let coordinate_bytes;
            {
                let (coordinates, storage) =
                    dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                        source.executable(),
                        &input,
                        profile,
                        &mut budget,
                    )
                    .unwrap();
                coordinate_bytes = storage.retained_storage();
                budget.reserve_storage(coordinate_bytes).unwrap();
                let (view, storage) =
                    fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                        &source,
                        &coordinates,
                        &checked,
                        &mut budget,
                    )
                    .unwrap();
                let view_bytes = storage.retained_storage();
                budget.reserve_storage(view_bytes).unwrap();
                assert!(std::ptr::eq(view.output(), checked.owner()));
                let projection = RankedProjectionSourceV1::from_legacy(&source).unwrap();
                let references =
                    crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
                let floor = budget.storage();
                result = with_ranked_root_preparation_v1(
                    &projection,
                    &inputs,
                    &references,
                    |effects, partition| {
                        checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                            &view,
                            &mut budget,
                            |session| {
                                session.with_canonical_memory_scope_v1(|session| {
                            let source_root = projection.source_launch().roots()[0];
                            let selection = projection.semantic_ssa().source_semantic()
                                .select_kernel_body_for_root_v1(source_root.selected_root()).unwrap();
                            let mut root = {
                                let mut facts = session.for_source(source_root.selected_root(), selection.body());
                                project_canonical_memory_root_v1(
                                    projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                                    &partition[0], &mut facts,
                                )?
                            };
                            session.with_output_occurrences_v1(|view, budget| {
                                body(view, &mut root, budget).map_err(|error|
                                    ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Output(error),
                                    ))
                            })
                        })
                            },
                        )
                    },
                );
                assert_eq!(budget.storage(), floor);
                drop(view);
                budget.release_storage(view_bytes).unwrap();
            }
            budget.release_storage(coordinate_bytes).unwrap();
            drop(checked);
            budget.release_storage(output_bytes).unwrap();
            drop(input);
            budget.release_storage(input_bytes).unwrap();
        }
        assert_eq!(budget.storage(), source_floor);
        drop(source);
        budget.release_storage(source_bytes).unwrap();
        budget.release_storage(capture.retained_storage()).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        result
    }

    fn candidate(root: &CanonicalMemoryProjectedRootV1) -> Candidate<'_> {
        Candidate {
            selected_root: root.selected_root,
            selected_function: root.selected_function,
            lowering: &root.lowering,
            access_sources: &root.access_sources,
            executable_effect_sources: &root.executable_effect_sources,
            control: root.control.candidate(),
        }
    }

    fn complete(
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        root: &CanonicalMemoryProjectedRootV1,
        budget: &mut Budget<'_>,
        body: impl FnOnce(
            &Analysis<'_, '_, '_>,
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        let candidates = [candidate(root)];
        view.with_conditional_memory_control_coverage_v1(&candidates, budget, |control, budget| {
            view.with_canonical_store_analysis_v1(
                &candidates,
                control,
                budget,
                |analyses, budget| {
                    assert_eq!(analyses.len(), 1);
                    body(&analyses[0], budget)
                },
            )
        })
    }

    #[test]
    fn actual_identity_getter_completes_nonempty_d_for_both_profiles_and_discriminator_modes() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for collected_modes in [false, true] {
                for value in [17, 29] {
                    let mut completed = false;
                    with_fixture(value, collected_modes, profile, |view, root, budget| {
                        let floor = budget.storage();
                        assert_eq!(root.access_sources.len(), 1);
                        assert!(root.executable_effect_sources.is_empty());
                        let source = &root.access_sources[0];
                        assert_eq!(source.semantic_block(), 3);
                        assert_eq!(source.semantic_statement(), Some(1));
                        assert_eq!(source.semantic_access_ordinal(), 0);
                        complete(view, root, budget, |analysis, budget| {
                            completed = true;
                            assert!(std::ptr::eq(analysis.output(), view.output()));
                            assert_eq!(analysis.selected_root(), ROOT);
                            assert_eq!(analysis.selected_function(), ROOT);
                            assert_eq!(analysis.access_count(), 1);
                            let access = analysis.access(0, budget)?.unwrap();
                            assert!(access.store_value().is_some());
                            let coordinate = access.operation();
                            let operation = &analysis.output().module().functions
                                [coordinate.block.function.0 as usize]
                                .body
                                .as_ref()
                                .unwrap()
                                .blocks[coordinate.block.block as usize]
                                .operations[coordinate.operation as usize];
                            assert!(matches!(
                                operation.kind,
                                fe2o3_kernel_ir::OperationKind::Store { .. }
                            ));
                            assert!(analysis.access(1, budget)?.is_none());
                            Ok(())
                        })?;
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    })
                    .unwrap();
                    assert!(completed);
                }
            }
        }
    }

    #[test]
    fn exact_identity_d_rejects_foreign_witness_extent_missing_call_and_default_claims() {
        use fe2o3_lower_mir_kernel::ProductionProjectionArgumentComponentV1 as Component;
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for case in 0..6 {
                with_fixture(23, true, profile, |view, root, budget| {
                    let claims = root.control.candidate_mut();
                    match case {
                        0 => {
                            claims
                                .arguments
                                .iter_mut()
                                .find(|row| row.source_local.index() == 2)
                                .unwrap()
                                .source_local = SemanticLocalIdV1::from_index(4)
                        }
                        1 => {
                            claims
                                .arguments
                                .iter_mut()
                                .find(|row| row.component == Component::SliceLength)
                                .unwrap()
                                .source_local = SemanticLocalIdV1::from_index(3)
                        }
                        2 => claims.arguments.retain(|row| row.source_local.index() != 2),
                        3 => {
                            claims
                                .blocks
                                .iter_mut()
                                .find(|row| row.source_block.index() == 2)
                                .unwrap()
                                .source_block = SemanticBlockIdV1::from_index(5)
                        }
                        4 => {
                            claims
                                .arguments
                                .iter_mut()
                                .find(|row| row.source_local.index() == 2)
                                .unwrap()
                                .component = Component::SliceLength
                        }
                        5 => {
                            let duplicate = *claims
                                .arguments
                                .iter()
                                .find(|row| row.source_local.index() == 2)
                                .unwrap();
                            claims.arguments.push(duplicate);
                        }
                        _ => unreachable!(),
                    }
                    let floor = budget.storage();
                    let mut reached = false;
                    let result = complete(view, root, budget, |_, _| {
                        reached = true;
                        Ok(())
                    });
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Invalid(_))),
                        "case {case}: {result:?}"
                    );
                    assert!(!reached);
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                })
                .unwrap();
            }
        }
    }

    #[test]
    fn multiple_identity_stores_have_distinct_source_and_output_occurrences() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture_stores(43, true, profile, 2, |view, root, budget| {
                let floor = budget.storage();
                assert_eq!(root.access_sources.len(), 2);
                assert_eq!(
                    root.access_sources
                        .iter()
                        .map(|row| (
                            row.semantic_block(),
                            row.semantic_statement(),
                            row.semantic_access_ordinal()
                        ))
                        .collect::<Vec<_>>(),
                    vec![(3, Some(1), 0), (3, Some(2), 0)]
                );
                complete(view, root, budget, |analysis, budget| {
                    assert_eq!(analysis.access_count(), 2);
                    let first = analysis.access(0, budget)?.unwrap();
                    let second = analysis.access(1, budget)?.unwrap();
                    assert!(first.store_value().is_some() && second.store_value().is_some());
                    assert_ne!(first.operation(), second.operation());
                    assert!(analysis.access(2, budget)?.is_none());
                    Ok(())
                })?;
                assert_eq!(budget.storage(), floor);
                let second = root.access_sources[1];
                root.access_sources[1] =
                    fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1::new(
                        second.semantic_block(),
                        Some(1),
                        0,
                        second.ranked_block(),
                        second.ranked_operation(),
                    );
                let mut completed = false;
                assert!(
                    complete(view, root, budget, |_, _| {
                        completed = true;
                        Ok(())
                    })
                    .is_err()
                );
                assert!(!completed);
                assert_eq!(budget.storage(), floor);
                Ok(())
            })
            .unwrap();
        }
    }

    // This component adapter supplies no checked control or output authority.
    // It only retains the legacy dynamic source-projection behavior and meters
    // its optional inert recorder on the already live test ledger.
    struct LegacyFacts<'a, 'w>(&'a mut Budget<'w>);
    impl ProjectedAssertionFactsV1 for LegacyFacts<'_, '_> {
        fn charge_private_array_work(
            &mut self,
            amount: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            self.0.charge_work(amount).map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
                )
            })
        }
        fn reserve_checked_control_storage_v1(
            &mut self,
            bytes: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            self.0.reserve_storage(bytes).map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
                )
            })
        }
        fn private_array_access(
            &mut self,
            _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
            _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            Ok(false)
        }
        fn private_array_constant_index(
            &mut self,
            _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
            _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
            Ok(None)
        }
        fn is_materialized_block(
            &mut self,
            _: usize,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            Ok(true)
        }
        fn condition(
            &mut self,
            _: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<
            canonical_assertion_facts_v1::ProjectedAssertionConditionV1,
            ProductionRankedProjectionErrorV1,
        > {
            Ok(canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Dynamic)
        }
    }

    #[test]
    fn legacy_expression_recorder_remains_observational_and_disabled_facts_cannot_retain_identity_store()
     {
        with_fixture(47, false, Profile::Gfx942, |view, _, budget| {
            let floor = budget.storage();
            {
                let projection = RankedProjectionSourceV1::from_legacy(view.source()).unwrap();
                let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
                let references =
                    crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
                with_ranked_root_preparation_v1(
                    &projection,
                    &inputs,
                    &references,
                    |effects, partition| {
                        let root = projection.source_launch().roots()[0];
                        let semantic = projection.semantic_ssa().source_semantic();
                        let selection = semantic
                            .select_kernel_body_for_root_v1(root.selected_root())
                            .unwrap();
                        let function = &semantic.functions()[selection.body().index() as usize];
                        let mut facts = LegacyFacts(budget);
                        assert!(!facts.checked_control_enabled_v1());
                        let legacy = prepare_projected_ranked_geometry_v1(
                            projection.semantic_ssa(),
                            effects,
                            selection,
                            &inputs[0],
                            root,
                            &partition[0],
                            &mut facts,
                            ProjectedGlobalWriteValuesV1::Expression,
                            None,
                        )?;
                        let mut recorder =
                            canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(
                                &mut facts,
                            )?;
                        let recorded = prepare_projected_ranked_geometry_v1(
                            projection.semantic_ssa(),
                            effects,
                            selection,
                            &inputs[0],
                            root,
                            &partition[0],
                            &mut facts,
                            ProjectedGlobalWriteValuesV1::Expression,
                            Some(&mut recorder),
                        )?;
                        assert_eq!(
                            format_ranked_cfg(A_NAME, &legacy.blocks)?,
                            format_ranked_cfg(A_NAME, &recorded.blocks)?
                        );
                        assert_eq!(legacy.sources, recorded.sources);
                        assert_eq!(
                            legacy.executable_effect_sources,
                            recorded.executable_effect_sources
                        );
                        assert_eq!(legacy.argument_count, recorded.argument_count);
                        assert!(legacy.incomplete.is_none() && recorded.incomplete.is_none());
                        let identity = recorder.identity_slice(
                            function,
                            semantic.callables(),
                            &recorded.intrinsic,
                            &[],
                            &mut facts,
                        )?;
                        let before = facts.0.work();
                        let error = retain_canonical_identity_slice_v1(
                            function,
                            &recorded.intrinsic.local_contracts.checked_references,
                            &identity,
                            &mut [],
                            &mut facts,
                        )
                        .unwrap_err();
                        assert!(matches!(
                            error,
                            ProductionRankedProjectionErrorV1::Incomplete(
                                "canonical identity retention requires checked output facts"
                            )
                        ));
                        assert_eq!(facts.0.work() - before, 2);
                        Ok(())
                    },
                )
                .unwrap();
            }
            // All source-domain projections and recorder Vecs are now dropped.
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn exact_identity_d_rejects_wrong_source_store_site_and_ordinal() {
        for case in 0..3 {
            with_fixture(31, false, Profile::Gfx942, |view, root, budget| {
                let source = &root.access_sources[0];
                let changed = fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1::new(
                    if case == 0 {
                        4
                    } else {
                        source.semantic_block()
                    },
                    if case == 1 {
                        Some(0)
                    } else {
                        source.semantic_statement()
                    },
                    if case == 2 {
                        1
                    } else {
                        source.semantic_access_ordinal()
                    },
                    source.ranked_block(),
                    source.ranked_operation(),
                );
                root.access_sources[0] = changed;
                let floor = budget.storage();
                let mut reached = false;
                assert!(
                    complete(view, root, budget, |_, _| {
                        reached = true;
                        Ok(())
                    })
                    .is_err()
                );
                assert!(!reached);
                assert_eq!(budget.storage(), floor);
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn completed_identity_d_preserves_balanced_callback_errors_panics_and_reentry() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(37, true, profile, |view, root, budget| {
                let floor = budget.storage();
                for panic in [false, true] {
                    let before = budget.work();
                    let mut entered = false;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        complete(view, root, budget, |analysis, budget| {
                            entered = true;
                            assert_eq!(analysis.access_count(), 1);
                            assert!(analysis.access(0, budget)?.is_some());
                            if panic {
                                panic!("identity D callback");
                            }
                            Err(ProductionSourceOutputErrorV1::Invalid(
                                "identity D callback",
                            ))
                        })
                    }));
                    assert!(entered);
                    assert_eq!(result.is_err(), panic);
                    match result {
                        Ok(result) => assert!(matches!(
                            result,
                            Err(ProductionSourceOutputErrorV1::Invalid(
                                "identity D callback"
                            ))
                        )),
                        Err(payload) => {
                            let message = payload
                                .downcast_ref::<&str>()
                                .copied()
                                .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
                            assert_eq!(message, Some("identity D callback"));
                        }
                    }
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                }
                complete(view, root, budget, |analysis, budget| {
                    assert!(analysis.access(0, budget)?.is_some());
                    Ok(())
                })?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn fresh_admitted_redefined_witness_cannot_enter_completed_d() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let ssa = identity_getter_shape_tests::redefined_witness_source();
            let calls = ssa.source_semantic().functions()[0]
                .blocks()
                .iter()
                .filter(|block| {
                    matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                    if call.callee().index() == 1)
                })
                .count();
            assert_eq!(calls, 2);
            let mut reached = false;
            let result = with_admitted_fixture(ssa, false, profile, |view, root, budget| {
                complete(view, root, budget, |_, _| {
                    reached = true;
                    Ok(())
                })
            });
            assert!(
                result.is_err(),
                "fresh source producer redefinition must remain closed"
            );
            assert!(!reached);
        }
    }

    #[test]
    fn actual_identity_d_denial_restores_original_floor_without_resetting_ledger() {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        for late in [false, true] {
            let mut observed = false;
            let result = with_fixture(41, true, Profile::Gfx942, |view, root, budget| {
                let floor = budget.storage();
                if late {
                    let result = complete(view, root, budget, |analysis, budget| {
                        assert_eq!(analysis.access_count(), 1);
                        assert!(budget.storage() > floor);
                        budget.charge_work(LIMIT - budget.work()).unwrap();
                        let denial = budget.charge_work(1).unwrap_err();
                        assert!(matches!(denial, Resource::Work(_)));
                        observed = true;
                        Err(ProductionSourceOutputErrorV1::Resource(denial))
                    });
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::Resource(Resource::Work(_)))
                    ));
                    assert_eq!(budget.storage(), floor);
                    result
                } else {
                    // Entry charges 6, then scope charges 1 before reserving its
                    // header. Leave no storage: denial precedes any new allocation.
                    let held = STORAGE_LIMIT - floor;
                    budget.reserve_storage(held).unwrap();
                    let before = budget.work();
                    let result = complete(view, root, budget, |_, _| {
                        panic!("storage denial reached D")
                    });
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::Resource(Resource::Storage(
                            _
                        )))
                    ));
                    assert_eq!(budget.work() - before, 7);
                    assert_eq!(budget.storage(), floor + held);
                    budget.release_storage(held).unwrap();
                    observed = true;
                    Ok(())
                }
            });
            assert!(observed);
            assert_eq!(result.is_err(), late);
        }
    }
}
