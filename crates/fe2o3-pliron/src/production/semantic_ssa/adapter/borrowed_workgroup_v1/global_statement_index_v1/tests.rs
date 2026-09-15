use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
#[path = "fixture.rs"]
mod fixture;
use fixture::*;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn linear<T>(
    facts: &[GlobalBf16BorrowV1],
    statement: &SemanticStatementKindV1,
    mut accept: impl FnMut(&GlobalBf16BorrowV1, SemanticLocalIdV1) -> Option<T>,
) -> Option<T> {
    facts.iter().find_map(|fact| {
        let local = fact
            .captured_reference(statement)
            .or_else(|| fact.metadata_reference(statement))?;
        accept(fact, local)
    })
}

#[test]
fn indexed_match_agrees_with_original_scan_for_copies_moves_and_mutations() {
    let facts = facts();
    let mut work = budget(MAX_FLOW_WORK);
    let index = GlobalStatementIndex::new(&facts, &mut work).unwrap();
    let mut corpus = vec![SemanticStatementKindV1::Nop];
    for value in [
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, ty(7)))),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, ty(7)))),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(1, ty(6)),
        },
    ] {
        corpus.push(assignment(place(10, ty(7)), ty(7), value));
    }
    for base in [0, 15, 30] {
        corpus.extend((0..10).map(|change| capture(base, change)));
        corpus.extend((0..8).map(|change| metadata(base, change)));
    }
    for (i, statement) in corpus.iter().enumerate() {
        for allow in [false, true] {
            let accept = |fact: &GlobalBf16BorrowV1, local: SemanticLocalIdV1| {
                allow.then_some((fact.pairs(), local))
            };
            assert_eq!(
                index.find(statement, &mut work, accept).unwrap(),
                linear(&facts, statement, accept),
                "corpus {i} allow {allow}"
            );
        }
    }
    for base in [0, 15] {
        assert!(
            index
                .find(&capture(base, 0), &mut work, |_, local| Some(local))
                .unwrap()
                .is_some()
        );
        assert!(
            index
                .find(&capture(base, 1), &mut work, |_, local| Some(local))
                .unwrap()
                .is_some()
        );
        assert!(
            index
                .find(&metadata(base, 0), &mut work, |_, local| Some(local))
                .unwrap()
                .is_some()
        );
        assert!(
            index
                .find(&metadata(base, 1), &mut work, |_, local| Some(local))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn overlap_diagnostics_preserve_cold_results_callbacks_and_all_budget_boundaries() {
    let facts = facts();
    let mut construction = budget(MAX_FLOW_WORK);
    let index = GlobalStatementIndex::new(&facts, &mut construction).unwrap();
    for (case, statement) in [
        capture(0, 0),
        capture(0, 1),
        capture(0, 4),
        metadata(0, 0),
        metadata(0, 1),
    ]
    .into_iter()
    .enumerate()
    {
        for limit in 0..=73 {
            for cap in [0, 1, 1_000] {
                let run = |traced| {
                    let mut work = budget(limit);
                    work.profile.stage = FlowWorkStage::Uses;
                    if traced {
                        work.profile.uses =
                            uses_observation_v1::Observation::for_test(11, facts.len(), cap);
                    }
                    work.profile.uses.statement(
                        11,
                        &statement as *const SemanticStatementKindV1 as usize,
                        0,
                        0,
                    );
                    let mut results = Vec::new();
                    let mut visits = [Vec::new(), Vec::new()];
                    for reader in 0..2 {
                        work.profile.uses.enter(
                            if reader == 0 {
                                uses_observation_v1::Part::GlobalStatement
                            } else {
                                uses_observation_v1::Part::GlobalCapture
                            },
                            0,
                            0,
                        );
                        let result = index.find(&statement, &mut work, |fact, local| {
                            let ordinal = facts.iter().position(|f| std::ptr::eq(f, fact)).unwrap();
                            visits[reader].push(ordinal);
                            (ordinal == if reader == 0 { 0 } else { 2 }).then_some(local)
                        });
                        let failed = result.is_err();
                        results.push(result);
                        if failed {
                            break;
                        }
                    }
                    (
                        work.remaining,
                        results,
                        visits,
                        work.profile.uses.test_counts(),
                    )
                };
                let cold = run(false);
                let traced = run(true);
                assert_eq!(
                    (&cold.0, &cold.1, &cold.2),
                    (&traced.0, &traced.1, &traced.2),
                    "case={case} limit={limit} diagnostic-cap={cap}"
                );
                if matches!(case, 0 | 1 | 3) && limit >= 56 && cap == 1_000 {
                    // Each lookup is 1 + (two keys + 1), followed by 16 per fact.
                    assert_eq!(limit - traced.0, (4 + 16) + (4 + 2 * 16));
                    assert_eq!(traced.2, [vec![0], vec![0, 2]]);
                    assert_eq!(traced.3, ([1, 2], [1, 2], 1, true));
                }
                if matches!(case, 0 | 1 | 3) && limit == 55 && cap == 1_000 {
                    assert_eq!(traced.3, ([1, 1], [1, 1], 1, true));
                    assert_eq!(traced.0, 15);
                }
                if cap == 0 {
                    assert!(!traced.3.3);
                }
            }
        }
    }
}

#[test]
fn a_failed_matcher_charge_does_not_increment_matcher_or_callback_counts() {
    let facts = facts();
    let mut construction = budget(MAX_FLOW_WORK);
    let index = GlobalStatementIndex::new(&facts, &mut construction).unwrap();
    let statement = capture(0, 0);
    let mut work = budget(19);
    work.profile.stage = FlowWorkStage::Uses;
    work.profile.uses = uses_observation_v1::Observation::for_test(11, facts.len(), 100);
    work.profile.uses.statement(
        11,
        &statement as *const SemanticStatementKindV1 as usize,
        0,
        0,
    );
    work.profile
        .uses
        .enter(uses_observation_v1::Part::GlobalStatement, 0, 0);
    assert!(
        index
            .find(&statement, &mut work, |_, _| -> Option<()> {
                panic!("callback after denied matcher charge")
            })
            .is_err()
    );
    assert_eq!(work.remaining, 15);
    assert_eq!(work.profile.uses.test_counts(), ([0, 0], [0, 0], 0, true));
}

#[test]
fn duplicate_fact_order_and_outer_candidate_rejection_are_preserved() {
    let facts = facts();
    let mut work = budget(MAX_FLOW_WORK);
    let index = GlobalStatementIndex::new(&facts, &mut work).unwrap();
    for statement in [capture(0, 0), metadata(0, 0)] {
        let mut visits = Vec::new();
        let result = index
            .find(&statement, &mut work, |fact, local| {
                let ordinal = facts.iter().position(|f| std::ptr::eq(f, fact)).unwrap();
                visits.push(ordinal);
                (ordinal == 2).then_some(local)
            })
            .unwrap();
        assert!(result.is_some());
        assert_eq!(visits, [0, 2]);
        let mut expected_visits = Vec::new();
        let expected = linear(&facts, &statement, |fact, local| {
            let ordinal = facts.iter().position(|f| std::ptr::eq(f, fact)).unwrap();
            expected_visits.push(ordinal);
            (ordinal == 2).then_some(local)
        });
        assert_eq!(result, expected);
        assert_eq!(visits, expected_visits);
    }
}

#[test]
fn necessary_keys_never_replace_exact_matcher_or_owner_type_join() {
    let facts = facts();
    let mut work = budget(MAX_FLOW_WORK);
    let index = GlobalStatementIndex::new(&facts, &mut work).unwrap();
    for statement in (2..10)
        .map(|change| capture(0, change))
        .chain((1..8).map(|change| metadata(0, change)))
    {
        assert!(
            index
                .find(&statement, &mut work, |_, local| Some(local))
                .unwrap()
                .is_none()
        );
    }
    for wrong in [ty(0), ty(21)] {
        let accept =
            |fact: &GlobalBf16BorrowV1, local| (fact.pairs()[2].1 == wrong).then_some(local);
        assert!(
            index
                .find(&capture(0, 0), &mut work, accept)
                .unwrap()
                .is_none()
        );
        assert!(
            index
                .find(&metadata(0, 0), &mut work, accept)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn failed_fact_authentication_cannot_enter_the_index() {
    let mut types = types();
    let callable = callable(0, 62);
    assert!(GlobalBf16BorrowV1::for_callable(&types, &callable).is_some());
    types[7] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([27; 32]),
        SemanticLayoutIdentityV1::from_sha256([27; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(6),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    let facts = GlobalBf16BorrowV1::for_callable(&types, &callable)
        .into_iter()
        .collect::<Vec<_>>();
    assert!(facts.is_empty());
    let mut work = budget(0);
    let index = GlobalStatementIndex::new(&facts, &mut work).unwrap();
    assert!(
        index
            .find(&capture(0, 0), &mut work, |_, local| Some(local))
            .unwrap()
            .is_none()
    );
    assert_eq!(work.remaining, 0);
}

#[test]
fn exact_and_one_short_construction_lookup_and_match_charges_fail_closed() {
    let facts = facts();
    let mut reference = budget(MAX_FLOW_WORK);
    let index = GlobalStatementIndex::new(&facts, &mut reference).unwrap();
    let build_work = MAX_FLOW_WORK - reference.remaining;
    let mut exact = budget(build_work);
    assert!(GlobalStatementIndex::new(&facts, &mut exact).is_ok());
    assert_eq!(exact.remaining, 0);
    let mut short = budget(build_work - 1);
    assert!(GlobalStatementIndex::new(&facts, &mut short).is_err());
    for statement in [
        SemanticStatementKindV1::Nop,
        capture(0, 0),
        metadata(15, 0),
        capture(30, 0),
    ] {
        let mut full = budget(MAX_FLOW_WORK);
        let expected = index
            .find(&statement, &mut full, |_, local| Some(local))
            .unwrap();
        let used = MAX_FLOW_WORK - full.remaining;
        assert!(used > 0);
        let mut exact = budget(used);
        assert_eq!(
            index
                .find(&statement, &mut exact, |_, local| Some(local))
                .unwrap(),
            expected
        );
        assert_eq!(exact.remaining, 0);
        let mut short = budget(used - 1);
        let error = index
            .find(&statement, &mut short, |_, local| Some(local))
            .unwrap_err();
        let error = flow_work_profile_v1::original_error_for_test(error);
        assert!(
            matches!(error, ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits, required, limit,
        } if required == used && limit == used - 1)
        );
    }
}

#[test]
fn unrelated_statement_work_is_not_linear_in_fact_count_and_budget_still_exhausts() {
    let facts = facts();
    let mut build = budget(MAX_FLOW_WORK);
    let index = GlobalStatementIndex::new(&facts, &mut build).unwrap();
    let mut work = budget(8_000);
    let ordinary = assignment(
        place(10, ty(2)),
        ty(2),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(20, ty(2)))),
    );
    for i in 0..8_000 {
        let statement = if i % 2 == 0 {
            &SemanticStatementKindV1::Nop
        } else {
            &ordinary
        };
        assert!(
            index
                .find(statement, &mut work, |_, _: SemanticLocalIdV1| Some(()))
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(work.remaining, 0);
    assert!(
        index
            .find(
                &SemanticStatementKindV1::Nop,
                &mut work,
                |_, _: SemanticLocalIdV1| Some(())
            )
            .is_err()
    );
    assert_eq!(8_000 * facts.len() * 16, 384_000);
    // Synthetic classifier work only, not an actual kernel result or changed ceiling.
}
