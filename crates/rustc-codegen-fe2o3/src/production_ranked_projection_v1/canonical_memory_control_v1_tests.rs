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
