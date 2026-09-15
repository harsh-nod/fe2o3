use super::*;

fn frame_entry_fixture() -> Fixture {
    let mut fixture = Fixture::new(15);
    fixture.edit(0, |statements, _| {
        statements.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(7)),
        ));
    });
    fixture.edit(1, |statements, _| {
        for local in [8, 9, 10, 11, 12] {
            statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(local)),
            ));
        }
    });
    fixture
}

#[test]
fn shared_capture_frame_entry_lifetime_dominates_later_definition() {
    let fixture = frame_entry_fixture();
    let function = fixture.function();
    let retained = function.clone();
    let result = SemanticAssertProofsV1::analyze(&fixture.types, &function).unwrap();
    for bb in [4, 5, 6, 7] {
        assert!(result[bb], "original checked operation bb{bb}");
    }
    assert_eq!(function, retained);
}

fn storage(local: u32, live: bool) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        if live {
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(local))
        } else {
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local))
        },
    )
}

fn origin(fixture: &Fixture) -> Option<(SemanticOperandV1, ScalarAssignmentSiteV1)> {
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    let SemanticOperandV1::Copy(read) = projection(9, SemanticProjectionKindV1::Dereference, WORD)
    else {
        unreachable!()
    };
    let statement = function.blocks()[2].statements().iter().position(|statement| {
        matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) if place == &read))
    }).unwrap();
    proof
        .shared_scalar_read_source_v1(
            &read,
            ScalarAssignmentSiteV1 {
                block: 2,
                statement,
            },
        )
        .unwrap()
}

#[test]
fn shared_capture_frame_entry_late_storage_live_rejects_before_origin() {
    for (bb, at, local) in [(1, 1, 7), (2, 0, 7), (2, 1, 8)] {
        let mut fixture = frame_entry_fixture();
        assert!(origin(&fixture).is_some());
        fixture.edit(bb, |statements, _| {
            statements.insert(at, storage(local, true))
        });
        assert!(origin(&fixture).is_none());
        fixture.first_rejects();
    }
}

#[test]
fn shared_capture_frame_entry_dead_storage_never_supplies_value() {
    for local in [4, 5, 6, 7, 8, 9] {
        let mut fixture = frame_entry_fixture();
        assert!(origin(&fixture).is_some());
        fixture.edit(1, |statements, _| statements.push(storage(local, false)));
        assert!(origin(&fixture).is_none());
        fixture.first_rejects();
    }
}

#[test]
fn shared_capture_frame_entry_reopened_referent_rejects() {
    let mut fixture = frame_entry_fixture();
    fixture.edit(1, |statements, _| statements.push(storage(4, true)));
    assert!(origin(&fixture).is_none());
    fixture.first_rejects();
}

#[test]
fn shared_capture_frame_entry_joins_bypasses_and_backedges_reject() {
    for mutation in 0..3 {
        let mut fixture = frame_entry_fixture();
        assert!(origin(&fixture).is_some());
        if mutation == 2 {
            fixture.edit(8, |_, terminal| *terminal = goto(1));
        } else {
            fixture.blocks.push(block(10, vec![], goto(2)));
            fixture.edit(1, |_, terminal| {
                *terminal = SemanticTerminatorKindV1::SwitchInt {
                    discriminant: scalar(BOOL, 1, 1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 10),
                    )
                    .unwrap(),
                };
            });
            if mutation == 1 {
                fixture.edit(10, |statements, _| {
                    statements.push(assign(
                        7,
                        CAPTURE,
                        SemanticRvalueKindV1::Use(copy(6, CAPTURE)),
                    ))
                });
            }
        }
        assert!(origin(&fixture).is_none());
        fixture.first_rejects();
    }
}

#[test]
fn shared_capture_frame_entry_unknown_call_and_unwind_stay_rejected() {
    for mutation in 0..3 {
        let mut fixture = frame_entry_fixture();
        fixture.edit(1, |_, terminal| {
            let SemanticTerminatorKindV1::Call(call) = terminal else {
                unreachable!()
            };
            *terminal = SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    if mutation == 0 {
                        vec![copy(7, CAPTURE)]
                    } else {
                        vec![]
                    },
                    call.destination().cloned(),
                    match mutation {
                        0 => SemanticUnwindActionV1::Unreachable,
                        1 => SemanticUnwindActionV1::Continue,
                        _ => {
                            SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 2))
                        }
                    },
                )
                .unwrap(),
            );
        });
        assert!(origin(&fixture).is_none());
        fixture.first_rejects();
    }
}

#[test]
fn shared_capture_frame_entry_preserves_guard_and_checked_operand_obligations() {
    for mask in [15, 63, 64] {
        let mut fixture = frame_entry_fixture();
        fixture.edit(0, |statements, _| {
            statements[2] = assign(
                4,
                WORD,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::BitAnd,
                    left: copy(1, WORD),
                    right: scalar(WORD, mask, 8),
                },
            );
        });
        let results = SemanticAssertProofsV1::analyze(&fixture.types, &fixture.function()).unwrap();
        assert_eq!(results[4], mask <= 63);
        assert_eq!(results[6], mask == 15);
    }
    let mut fixture = frame_entry_fixture();
    fixture.edit(3, |_, terminal| *terminal = goto(4));
    let results = SemanticAssertProofsV1::analyze(&fixture.types, &fixture.function()).unwrap();
    assert!(results[4]);
    assert!(!results[5]);
    assert!(!results[6]);
    assert!(!results[7]);
}

#[test]
fn shared_capture_frame_entry_dominance_uses_shared_budget_and_cache() {
    let fixture = frame_entry_fixture();
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    let SemanticOperandV1::Copy(read) = projection(9, SemanticProjectionKindV1::Dereference, WORD)
    else {
        unreachable!()
    };
    let site = ScalarAssignmentSiteV1 {
        block: 2,
        statement: 2,
    };
    assert!(
        proof
            .shared_scalar_read_source_v1(&read, site)
            .unwrap()
            .is_some()
    );
    assert_eq!(proof.dominance.get(&(1, 2)), Some(&true));
    let previous_work = proof.work;
    assert!(
        proof
            .shared_scalar_read_source_v1(&read, site)
            .unwrap()
            .is_some()
    );
    assert!(proof.work > previous_work);
    assert!(proof.helper_result_ranges.is_none());
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(matches!(
        proof.shared_scalar_read_source_v1(&read, site),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert!(proof.work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[path = "frame_entry/expansion.rs"]
mod expansion;
