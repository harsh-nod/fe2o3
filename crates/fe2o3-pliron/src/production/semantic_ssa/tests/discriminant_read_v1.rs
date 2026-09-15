//! Admitted, layout-checked fixtures. No source/profile names are authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1;
mod fixture;
use fixture::*;

fn rejects_at(mir: &AdmittedInertSemanticMirV1, statement: usize) {
    assert!(
        matches!(plan(mir), Err(ProductionSemanticSsaErrorV1::PartialMove {
        local: PAIR, statement: Some(found),
        violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed, ..
    }) if found == statement as u32)
    );
}

#[test]
fn direct_enum_discriminant_after_payload_move_retains_source_and_replays() {
    let mut statements = initialized();
    statements.extend([moved(0), tag(0), tag(1)]);
    let mir = linear(statements);
    let bytes = mir.canonical_encoding().to_vec();
    let function = mir.functions()[0].clone();
    let first = plan(&mir).unwrap();
    assert_eq!(first.partial_move_certificate().projected_moves(), 1);
    assert_eq!(first, plan(&mir).unwrap());
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(mir, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(owner.source_semantic().canonical_encoding(), bytes);
    assert_eq!(owner.source_semantic().functions()[0], function);
}

#[test]
fn direct_enum_tag_does_not_make_payload_or_whole_enum_readable() {
    for read in [payload(0), enumeration(0), whole(PAIR)] {
        let destination = match read.ty().index() {
            1 => VALUE,
            2 => ENUM_COPY,
            3 => PAIR_COPY,
            _ => unreachable!(),
        };
        let mut statements = initialized();
        statements.extend([
            moved(0),
            tag(0),
            test_assign_to(whole(destination), SemanticOperandV1::Copy(read)),
        ]);
        rejects_at(&linear(statements), 4);
    }
}

#[test]
fn direct_enum_tag_equal_move_is_not_a_descendant_payload_move() {
    let mut statements = initialized();
    statements.extend([
        test_assign_to(whole(ENUM_COPY), SemanticOperandV1::Move(enumeration(0))),
        tag(0),
    ]);
    rejects_at(&linear(statements), 3);
}

#[test]
fn direct_enum_tag_ancestor_move_preserves_ssa_kill() {
    let mut statements = initialized();
    statements.extend([
        test_assign_to(whole(PAIR_COPY), SemanticOperandV1::Move(whole(PAIR))),
        tag(0),
    ]);
    assert!(
        matches!(plan(&linear(statements)), Err(ProductionSemanticSsaErrorV1::Planner {
        error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, ..
    }) if variable == SsaVariableIdV1::new(PAIR))
    );
}

#[test]
fn direct_enum_whole_overwrite_restores_tag_and_payload() {
    for whole_pair in [false, true] {
        let mut statements = initialized();
        statements.push(moved(0));
        if whole_pair {
            statements.extend(initialized());
        } else {
            statements.push(construct(enumeration(0)));
        }
        statements.extend([tag(0), moved(0)]);
        plan(&linear(statements)).unwrap();
    }
}

#[test]
fn direct_enum_deinitialize_remains_conservative_for_payload_enum_and_ancestor() {
    for place in [payload(0), enumeration(0), whole(PAIR)] {
        let mut statements = initialized();
        // Trigger partial-move analysis independently of the deinitialized side.
        statements.extend([
            moved(1),
            statement(SemanticStatementKindV1::Deinitialize(place)),
            tag(0),
        ]);
        // Whole-pair deinit first reads the already partially moved pair.
        let expected =
            if statements[3].kind() == &SemanticStatementKindV1::Deinitialize(whole(PAIR)) {
                3
            } else {
                4
            };
        rejects_at(&linear(statements), expected);
    }
}

#[test]
fn direct_enum_set_discriminant_cannot_resurrect_moved_payload_or_enum() {
    for payload_move in [false, true] {
        let mut statements = initialized();
        statements.push(if payload_move {
            moved(0)
        } else {
            test_assign_to(whole(ENUM_COPY), SemanticOperandV1::Move(enumeration(0)))
        });
        statements.extend([
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: enumeration(0),
                variant_index: 1,
            }),
            tag(0),
        ]);
        rejects_at(&linear(statements), 3);
    }
    let mut statements = initialized();
    statements.extend([
        moved(1),
        statement(SemanticStatementKindV1::SetDiscriminant {
            place: enumeration(0),
            variant_index: 0,
        }),
        tag(0),
    ]);
    plan(&linear(statements)).unwrap();
}

#[test]
fn direct_enum_tag_cannot_read_uninitialized_or_reopened_storage() {
    for initialize in [false, true] {
        let mut statements = if initialize { initialized() } else { vec![] };
        if initialize {
            statements.extend([
                moved(0),
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(PAIR),
                )),
                statement(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(PAIR),
                )),
            ]);
        }
        statements.push(tag(0));
        assert!(
            matches!(plan(&linear(statements)), Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, ..
        }) if variable == SsaVariableIdV1::new(PAIR))
        );
    }
}

#[test]
fn direct_enum_join_accepts_only_descendant_holes_on_every_path() {
    for equal in [false, true] {
        let branch = if equal {
            test_assign_to(whole(ENUM_COPY), SemanticOperandV1::Move(enumeration(0)))
        } else {
            moved(1)
        };
        let mir = admit(vec![
            test_block(
                170,
                initialized(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: scalar(0),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            test_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            test_block(
                171,
                vec![moved(0)],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            test_block(
                172,
                vec![branch],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            test_block(173, vec![tag(0)], SemanticTerminatorKindV1::Return),
        ]);
        if equal {
            rejects_at(&mir, 0);
        } else {
            plan(&mir).unwrap();
        }
    }
}

#[test]
fn direct_enum_join_does_not_invent_an_initial_tag() {
    let mir = admit(vec![
        test_block(
            170,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: scalar(0),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        test_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        test_block(
            171,
            initialized(),
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 2)),
        ),
        test_block(172, vec![tag(0)], SemanticTerminatorKindV1::Return),
    ]);
    let result = plan(&mir);
    assert!(
        matches!(&result, Err(ProductionSemanticSsaErrorV1::Planner {
            function,
            error: SsaPlannerErrorV1::UndefinedAtEdge { edge, target, variable },
        }) if function.index() == 0 && edge.source().get() == 0 && edge.ordinal() == 1
            && target.get() == 2 && *variable == SsaVariableIdV1::new(PAIR)),
        "{result:?}"
    );
}

#[test]
fn direct_enum_tag_adapter_budget_remains_inclusive() {
    let mut statements = initialized();
    statements.extend([moved(0), tag(0)]);
    let mir = linear(statements);
    let baseline = plan(&mir).unwrap();
    let storage = baseline.resources().storage_words()
        + baseline.auxiliary_resources.storage_words
        + baseline.partial_moves.state_entries();
    let work = baseline.resources().work_units()
        + baseline.auxiliary_resources.work_units
        + baseline.partial_moves.work_units();
    let limits = |storage_words, work_units| {
        let defaults = SsaPlannerLimitsV1::default();
        ProductionSemanticSsaLimitsV1::new(
            SsaPlannerLimitsV1::try_new(
                defaults.max_variables(),
                defaults.max_blocks(),
                defaults.max_edges(),
                defaults.max_events(),
                defaults.max_edge_definitions(),
                defaults.max_output_items(),
                storage_words,
                work_units,
            )
            .unwrap(),
        )
    };
    let run = |storage, work| {
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &mir.functions()[0],
            mir.types(),
            mir.callables(),
            limits(storage, work),
        )
    };
    assert_eq!(run(storage, work).unwrap(), baseline);
    fn resource(error: ProductionSemanticSsaErrorV1) -> SsaPlannerResourceV1 {
        match error {
            ProductionSemanticSsaErrorV1::ResourceStage { error, .. } => resource(*error),
            ProductionSemanticSsaErrorV1::PartialMoveResourceLimit { resource, .. } => resource,
            other => panic!("expected a precise partial-move resource boundary, got {other:?}"),
        }
    }
    assert_eq!(
        resource(run(storage - 1, work).unwrap_err()),
        SsaPlannerResourceV1::StorageWords
    );
    assert_eq!(
        resource(run(storage, work - 1).unwrap_err()),
        SsaPlannerResourceV1::WorkUnits
    );
}

#[test]
fn direct_enum_tag_requires_authenticated_type_context() {
    let mut statements = initialized();
    statements.extend([moved(0), tag(0)]);
    let mir = linear(statements);
    assert!(
        plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &mir.functions()[0],
            ProductionSemanticSsaLimitsV1::default()
        )
        .is_err()
    );
}

#[test]
fn direct_enum_payload_cannot_overlap_tag_at_canonical_admission() {
    let mut statements = initialized();
    statements.extend([moved(0), tag(0)]);
    let result = request(
        vec![test_block(
            170,
            statements,
            SemanticTerminatorKindV1::Return,
        )],
        true,
    )
    .admit_current_production(SemanticMirLimitsV1::default());
    assert!(matches!(result, Err(SemanticMirErrorV1::InvalidTypeLayout)));
}
