//! Differential selection tests; the cold oracle is frozen Lane57, not a
//! weakened replacement. Fixtures have no issuer or production proof authority.
use super::super::super::super::{
    GlobalBf16BorrowV1, borrow_components_v1, lane_work_v1, math_capture_flow_v1,
    matrix_access_borrow,
};
use super::*;

#[path = "cold_oracle.rs"]
mod cold_oracle;

type Rows = Vec<(u32, u32, u32, u32, u32, Vec<(u32, u32)>)>;
fn compare_run(
    body: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    cold: bool,
    limit: usize,
) -> Result<(Rows, usize), ProductionSemanticSsaErrorV1> {
    let facts = BTreeMap::from([(
        0,
        MatrixAccessBorrow::for_callable(types, &callables[0]).unwrap(),
    )]);
    let mut budget = Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    };
    let rows = if cold {
        cold_oracle::sites_with_consumers(body, Some(types), &facts, callables, &mut budget)?
            .into_iter()
            .map(|(s, r)| {
                (
                    s.block,
                    s.statement,
                    r.parent,
                    r.reference.index(),
                    r.owned.index(),
                    r.borrow_sites
                        .into_iter()
                        .map(|s| (s.block, s.statement))
                        .collect(),
                )
            })
            .collect()
    } else {
        closed_lane_flow::sites_with_consumers(body, Some(types), &facts, callables, &mut budget)?
            .into_iter()
            .map(|(s, r)| {
                (
                    s.block,
                    s.statement,
                    r.parent,
                    r.reference.index(),
                    r.owned.index(),
                    r.borrow_sites
                        .into_iter()
                        .map(|s| (s.block, s.statement))
                        .collect(),
                )
            })
            .collect()
    };
    Ok((rows, limit - budget.remaining))
}

fn mixed(padding: usize, extra: Option<SemanticStatementKindV1>) -> SemanticFunctionDeclV1 {
    let body = fixture(Mutation::Escape);
    let SemanticStatementKindV1::Assign(a) = body.blocks()[1].statements()[0].kind() else {
        panic!()
    };
    let SemanticRvalueKindV1::Borrow { place: field, .. } = a.value().kind() else {
        panic!()
    };
    let mut statements = body.blocks()[1].statements().to_vec();
    // The first component is terminal-rejected. This second component survives
    // terminal checks and forces the old global full-body statement audit.
    statements.push(borrow(11, 7, field.clone(), SemanticBorrowKindV1::Shared));
    let body = changed(
        &body,
        1,
        statements,
        body.blocks()[1].terminator().kind().clone(),
    );
    let mut statements: Vec<_> =
        std::iter::repeat_n(assign(13, 1, SemanticRvalueKindV1::Use(scalar())), padding).collect();
    if let Some(extra) = extra {
        statements.push(SemanticStatementV1::new(source(), extra));
    }
    changed(&body, 2, statements, SemanticTerminatorKindV1::Return)
}

#[test]
fn surviving_component_selection_matches_every_passive_and_used_lane_case() {
    let types = types();
    for mutation in [
        Mutation::None,
        Mutation::Field,
        Mutation::Mutable,
        Mutation::Dereference,
        Mutation::Escape,
        Mutation::Duplicate,
        Mutation::TwoLeaves,
        Mutation::ParentEscape,
        Mutation::Fork,
    ] {
        let mut types = types.clone();
        if matches!(mutation, Mutation::TwoLeaves) {
            types[9] = aggregate(9, &[7, 7], &[0, 8], 16, 8, true);
        }
        let body = fixture(mutation);
        let callables = [access(true, 64, true)];
        assert_eq!(
            compare_run(&body, &types, &callables, false, 1_048_576)
                .unwrap()
                .0,
            compare_run(&body, &types, &callables, true, 1_048_576)
                .unwrap()
                .0,
            "{mutation:?}"
        );
    }
    for kind in 0..5 {
        for reborrow in [false, true] {
            let body = used_body(kind, reborrow);
            let callables = [access(true, 64, true), leaf_callable(kind, true)];
            for kill in [false, true] {
                let body = if kill {
                    changed(
                        &body,
                        3,
                        vec![SemanticStatementV1::new(
                            source(),
                            SemanticStatementKindV1::Deinitialize(place(14, 7)),
                        )],
                        SemanticTerminatorKindV1::Return,
                    )
                } else {
                    body.clone()
                };
                let old = compare_run(&body, &types, &callables, true, 1_048_576)
                    .unwrap()
                    .0;
                let new = compare_run(&body, &types, &callables, false, 1_048_576)
                    .unwrap()
                    .0;
                assert_eq!(new, old, "kind={kind} reborrow={reborrow} kill={kill}");
                assert_eq!(new.len(), usize::from(!kill));
            }
        }
    }
}

#[test]
fn a_survivor_does_not_force_full_audit_of_scalar_tail() {
    let body = mixed(32_768, None);
    let types = types();
    let callables = [access(true, 64, true)];
    let (old, old_work) = compare_run(&body, &types, &callables, true, 1_048_576).unwrap();
    let (new, new_work) = compare_run(&body, &types, &callables, false, 262_144).unwrap();
    assert_eq!(new, old);
    assert_eq!(new.len(), 1);
    assert_eq!((new[0].0, new[0].1), (1, 4));
    assert!(old_work > 262_144, "old={old_work}");
    assert!(new_work < old_work * 3 / 4, "old={old_work} new={new_work}");
    assert!(matches!(
        compare_run(&body, &types, &callables, true, 262_144),
        Err(ProductionSemanticSsaErrorV1::BorrowFlowWork { .. })
    ));
    eprintln!("mixed live/dead component: old={old_work} new={new_work} unchanged-limit=262144");
}

fn escaped_uses() -> Vec<SemanticStatementKindV1> {
    let lane = place(11, 7);
    let read = SemanticOperandV1::Copy(lane.clone());
    let indexed = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(13),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(11)),
                ty(1),
            )
            .unwrap(),
        ],
        ty(1),
    )
    .unwrap();
    let atomic = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Workgroup,
    );
    let assign_value = |kind| assign(13, 1, kind).kind().clone();
    vec![
        // The destination belongs to the dead component. Its value is live.
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(6, 7),
            read.clone(),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            lane.clone(),
            scalar(),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
        SemanticStatementKindV1::Deinitialize(lane.clone()),
        SemanticStatementKindV1::SetDiscriminant {
            place: lane.clone(),
            variant_index: 0,
        },
        SemanticStatementKindV1::Assume(read.clone()),
        assign_value(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(indexed))),
        assign_value(SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable,
            place: lane.clone(),
        }),
        assign_value(SemanticRvalueKindV1::Length(lane.clone())),
        assign_value(SemanticRvalueKindV1::Discriminant(lane.clone())),
        assign_value(SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            lane.clone(),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
        // A dead/malformed carrier may not hide a live sibling operand.
        assign(
            8,
            9,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![read.clone(), SemanticOperandV1::Copy(place(6, 7))],
                )
                .unwrap(),
            ),
        )
        .kind()
        .clone(),
        SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
            place(13, 1),
            place(6, 7),
            read.clone(),
            SemanticAtomicRmwOpV1::Add,
            atomic,
        )),
        SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
            place(13, 1),
            place(6, 7),
            scalar(),
            read,
            atomic,
            SemanticAtomicOrderingV1::Relaxed,
            false,
        )),
    ]
}

#[test]
fn dead_destinations_index_projections_and_unknown_operands_cannot_hide_live_uses() {
    let types = types();
    let callables = [access(true, 64, true)];
    for (index, statement) in escaped_uses().into_iter().enumerate() {
        let body = mixed(17, Some(statement));
        let old = compare_run(&body, &types, &callables, true, 1_048_576)
            .unwrap()
            .0;
        let new = compare_run(&body, &types, &callables, false, 1_048_576)
            .unwrap()
            .0;
        assert_eq!(new, old, "case={index}");
        assert!(new.is_empty(), "case={index} {new:?}");
    }
}

#[test]
fn late_writes_backedges_and_lifetime_events_match_the_original_all_use_boundary() {
    let types = types();
    let callables = [access(true, 64, true)];
    for event in [
        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(11)),
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(11)),
        SemanticStatementKindV1::Nop,
        SemanticStatementKindV1::Deinitialize(place(11, 7)),
    ] {
        for backedge in [false, true] {
            let body = mixed(128, Some(event.clone()));
            let body = if backedge {
                changed(
                    &body,
                    2,
                    body.blocks()[2].statements().to_vec(),
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(1),
                    )),
                )
            } else {
                body
            };
            let old = compare_run(&body, &types, &callables, true, 1_048_576)
                .unwrap()
                .0;
            let new = compare_run(&body, &types, &callables, false, 1_048_576)
                .unwrap()
                .0;
            assert_eq!(new, old, "event={event:?} backedge={backedge}");
            assert_eq!(
                new.is_empty(),
                matches!(event, SemanticStatementKindV1::Deinitialize(_))
            );
        }
    }
}

#[test]
fn survivor_selection_has_exact_work_boundary_and_does_not_publish_partial_rows() {
    let types = types();
    let callables = [access(true, 64, true)];
    for body in [
        mixed(32, None),
        mixed(
            32,
            Some(SemanticStatementKindV1::Deinitialize(place(11, 7))),
        ),
    ] {
        let (rows, needed) = compare_run(&body, &types, &callables, false, 262_144).unwrap();
        assert_eq!(
            compare_run(&body, &types, &callables, false, needed).unwrap(),
            (rows, needed)
        );
        for limit in [0, 1, 16, needed / 2, needed - 1] {
            let error = compare_run(&body, &types, &callables, false, limit)
                .err()
                .unwrap();
            let ProductionSemanticSsaErrorV1::BorrowFlowWork {
                remaining_work_units,
                error,
                ..
            } = error
            else {
                panic!("{error:?}")
            };
            assert!(remaining_work_units <= limit);
            assert!(
                matches!(*error,ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource:SsaPlannerResourceV1::WorkUnits,required,limit:actual } if actual==limit && required==limit+1)
            );
        }
    }
}
