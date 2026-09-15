use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdentityV1, SemanticCallDestinationV1, SemanticControlFlowEdgeV1,
    SemanticTerminatorV1,
};

fn edge(role: SemanticEdgeRoleV1) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(0))
}

fn operand() -> SemanticOperandV1 {
    SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            vec![],
            SemanticTypeIdV1::from_index(0),
        )
        .unwrap(),
    )
}

fn body(
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([231; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

#[test]
fn neutral_transfer_closed_operand_free_family_and_every_nonempty_block() {
    for kind in [
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto)),
        SemanticTerminatorKindV1::Return,
        SemanticTerminatorKindV1::UnwindResume,
        SemanticTerminatorKindV1::UnwindTerminate,
        SemanticTerminatorKindV1::Abort,
        SemanticTerminatorKindV1::Unreachable,
    ] {
        assert_eq!(
            is_identity(&body(vec![], kind.clone()), &mut Budget::new(1)),
            Ok(true)
        );
        for statement in [
            SemanticStatementKindV1::Nop,
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(0)),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(0)),
        ] {
            let statement =
                SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), statement);
            assert_eq!(
                is_identity(&body(vec![statement], kind.clone()), &mut Budget::new(1)),
                Ok(false)
            );
        }
    }
}

#[test]
fn neutral_transfer_calls_moves_drops_assertions_and_false_edges_stay_interpreted() {
    let callee = SemanticFunctionIdV1::from_index(0);
    let SemanticOperandV1::Move(place) = operand() else {
        unreachable!()
    };
    for kind in [
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                callee,
                vec![operand()],
                Some(SemanticCallDestinationV1::new(
                    place.clone(),
                    edge(SemanticEdgeRoleV1::CallReturn),
                )),
                SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind)),
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::TailCall(
            SemanticDirectTailCallV1::new(
                callee,
                vec![operand()],
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: operand(),
            targets: SemanticSwitchTargetsV1::new(
                vec![],
                edge(SemanticEdgeRoleV1::SwitchOtherwise),
            )
            .unwrap(),
        },
        SemanticTerminatorKindV1::Drop {
            place,
            drop_glue: callee,
            target: edge(SemanticEdgeRoleV1::DropReturn),
            unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::DropUnwind)),
        },
        SemanticTerminatorKindV1::Assert {
            condition: operand(),
            expected: true,
            message: SemanticAssertMessageV1::ResumedAfterPanic,
            target: edge(SemanticEdgeRoleV1::AssertSuccess),
            unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::AssertUnwind)),
        },
        SemanticTerminatorKindV1::FalseEdge {
            real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal),
            imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary),
        },
    ] {
        assert_eq!(
            is_identity(&body(vec![], kind), &mut Budget::new(1)),
            Ok(false)
        );
    }
}

#[test]
fn neutral_transfer_classifier_has_exact_nonrefundable_work_and_no_storage() {
    let block = body(vec![], SemanticTerminatorKindV1::Return);
    let before = block.clone();
    let mut budget = Budget::new(1);
    let held = budget.reserve(MAX_STORAGE).unwrap();
    assert_eq!(is_identity(&block, &mut budget), Ok(true));
    assert_eq!(budget.remaining, 0);
    assert_eq!(is_identity(&block, &mut budget), Err(()));
    assert!(budget.work_exhausted);
    drop(held);
    assert_eq!(budget.remaining, 0);
    assert!(budget.reserve(MAX_STORAGE).is_ok());
    assert_eq!(block, before);
}

#[test]
fn neutral_transfer_checked_source_edges_match_original_interpreter_without_normalizing_flow() {
    let owner = global_enum_transport_v1::tests::build_owner();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut visited = 0;
    for (block, body) in view.body().blocks().iter().enumerate() {
        if !is_identity(body, &mut Budget::new(1)).unwrap() {
            continue;
        }
        let reference = Reference {
            target: 1,
            mutable: false,
            borrow: Site {
                block: 0,
                statement: 0,
            },
        };
        let initial = Flow {
            values: BTreeMap::from([(2, Value::Reference(reference))]).into(),
            dead: BTreeSet::from([1]),
            escaped: BTreeSet::from([1]),
        };
        let mut flow = initial.clone();
        let mut old = Analysis::new(owner.source_semantic().types(), view);
        let expected = old.terminator(block, &mut flow).unwrap();
        let mut shared = Analysis::new(owner.source_semantic().types(), view);
        shared.observe_terminator(block);
        assert_eq!(shared.terminator_edges(block, None).unwrap(), expected);
        assert_eq!(shared.observation.last_block, old.observation.last_block);
        assert_eq!(
            shared.observation.last_statement,
            old.observation.last_statement
        );
        assert_eq!(shared.observation.phase, old.observation.phase);
        assert_eq!(shared.observation.terminator, old.observation.terminator);
        assert_eq!(shared.observation.edges, old.observation.edges);
        assert_eq!(
            shared.observation.edges_truncated,
            old.observation.edges_truncated
        );
        assert_eq!(shared.observation.failure, old.observation.failure);
        assert_eq!(
            flow, initial,
            "identity transfer must not manufacture meet normalization"
        );
        visited += 1;
    }
    assert!(visited > 0);
}
