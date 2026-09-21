//! Genuine constructed-source queries; ordinary rustc capture remains separate.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirGuardDistanceV1 as Distance, CanonicalKirGuardedUpdateV1 as Update,
    CanonicalKirInductionFactsV1 as Facts, CanonicalKirInductionOutcomeV1 as Outcome,
    CanonicalKirInventoryV1 as Inventory, CanonicalKirIterationScopeV1 as Iterations,
    CanonicalKirLoopLimitsV1 as Limits, CanonicalKirLoopsV1 as Loops,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirEdgeCoordinateV1 as Edge,
    ScalarType,
};
use fe2o3_lower_mir_kernel::{
    ProductionLoopInductionQueryErrorV1 as QueryError,
    ProductionLoopInductionQueryStorageV1 as QueryStorage, ProductionLoopInductionQueryV1 as Query,
};

fn query<'a>(
    owner: &'a Licm,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> std::result::Result<(Query<'a>, QueryStorage), QueryError> {
    match owner {
        Licm::Direct(v) => v.derive_loop_induction_facts_v1(limits, budget),
        Licm::Erased(v) => v.derive_loop_induction_facts_v1(limits, budget),
    }
}
fn replay_with(
    owner: &Licm,
    report: &Query<'_>,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> std::result::Result<(), QueryError> {
    match owner {
        Licm::Direct(v) => v.replay_loop_induction_facts_v1(report, limits, budget),
        Licm::Erased(v) => v.replay_loop_induction_facts_v1(report, limits, budget),
    }
}
fn with_owner(
    erased: bool,
    profile: Profile,
    mutation: bool,
    run: impl FnOnce(&Licm, &mut Budget<'_>),
) {
    with_prefix(erased, profile, mutation, |prefix, budget| {
        let floor = budget.storage();
        let (native, storage) = prepare(prefix, profile, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_shape(&native.owner, mutation);
        run(&native.owner, budget);
        drop(native);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
fn fresh_agreement(owner: &Licm, report: &Query<'_>, limits: Limits, budget: &mut Budget<'_>) {
    let floor = budget.storage();
    let (inventory, a) = Inventory::derive(owner.output(), budget).unwrap();
    budget.reserve_storage(a.retained_storage()).unwrap();
    let (loops, b) = Loops::derive(&inventory, limits, budget).unwrap();
    budget.reserve_storage(b.retained_storage()).unwrap();
    let (facts, c) = Facts::derive(&loops, limits, budget).unwrap();
    budget.reserve_storage(c.retained_storage()).unwrap();
    facts.replay(&loops, limits, budget).unwrap();
    assert_eq!(report.rows(), facts.rows());
    assert!(std::ptr::eq(report.output(), owner.output()));
    drop(facts);
    budget.release_storage(c.retained_storage()).unwrap();
    drop(loops);
    budget.release_storage(b.retained_storage()).unwrap();
    drop(inventory);
    budget.release_storage(a.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_loop_induction_queries_retain_actual_nonzero_final_recurrences_both_owners_and_profiles()
{
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_owner(erased, profile, true, |owner, budget| {
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let before = owner.output().canonical().canonical_bytes().to_vec();
                let (report, storage) = query(owner, Limits::default(), budget).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(report.retained_storage(), storage.retained_storage());
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert!(!report.grants_authority());
                assert_eq!(report.limits(), Limits::default());
                assert_eq!(
                    report.rows().len(),
                    2,
                    "actual observed final recurrences, never a vacuous source positive"
                );
                for (ordinal, row) in report.rows().iter().enumerate() {
                    let recurrence = row.recurrence();
                    assert_eq!(
                        (recurrence.scalar(), recurrence.step_bits()),
                        (ScalarType::U64, 1)
                    );
                    assert!(recurrence.overflow().is_some());
                    let Definition::BlockArgument {
                        block: header,
                        argument: 0,
                    } = recurrence.parameter()
                    else {
                        panic!("actual source induction parameter: {row:?}");
                    };
                    let Definition::BlockArgument {
                        block: preheader,
                        argument: 0,
                    } = recurrence.initial()
                    else {
                        panic!("actual symbolic preheader initial: {row:?}");
                    };
                    assert_eq!(row.loop_ordinal(), ordinal);
                    assert_eq!(header.function.0, u32::try_from(ordinal).unwrap());
                    assert_eq!(preheader.function, header.function);
                    assert_ne!(preheader, header);
                    assert_eq!(
                        recurrence.initial_edge(),
                        Edge {
                            source: preheader,
                            successor: 0
                        }
                    );
                    let Outcome::Guarded(fact) = row.outcome() else {
                        panic!("actual source guarded fact required: {row:?}");
                    };
                    let argument = if erased { 2 } else { 1 };
                    let bound = Definition::FunctionArgument {
                        function: header.function,
                        argument,
                    };
                    assert_eq!(fact.bound(), bound);
                    assert_eq!(fact.guarded_update(), Update::NonWrapping);
                    assert_eq!(fact.iteration_scope(), Iterations::NormalHeaderCompletion);
                    assert_eq!(
                        fact.guard_distance(),
                        Distance::UnitStride {
                            initial: recurrence.initial(),
                            bound,
                        },
                        "forwarded initial stays symbolic, never an invented literal trip count"
                    );
                    assert_eq!(fact.guard().block, header);
                    assert_eq!(
                        fact.body_edge(),
                        Edge {
                            source: header,
                            successor: 0
                        }
                    );
                    assert_eq!(
                        fact.exit_edge(),
                        Edge {
                            source: header,
                            successor: 1
                        }
                    );
                    let body = owner.output().module().functions[header.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap();
                    let block = &body.blocks[header.block as usize];
                    let guard = operation(owner.output(), fact.guard());
                    assert!(matches!(guard.kind, OperationKind::Compare {
                        predicate: fe2o3_kernel_ir::ComparePredicate::LessThan, lhs, rhs,
                    } if lhs == block.parameters[0].id && rhs == body.parameters[argument as usize]));
                    assert_eq!(guard.results.len(), 1);
                    assert!(
                        matches!(block.terminator, Some(fe2o3_kernel_ir::Terminator::ConditionalBranch {
                        condition, then_target, else_target, ..
                    }) if condition == guard.results[0].id && then_target != else_target)
                    );
                    eprintln!(
                        "SOURCE_LOOP_INDUCTION_ROW erased={erased} profile={profile:?} row={row:?}"
                    );
                }
                report.replay(Limits::default(), budget).unwrap();
                replay_with(owner, &report, Limits::default(), budget).unwrap();
                fresh_agreement(owner, &report, Limits::default(), budget);
                assert_eq!(owner.output().canonical().canonical_bytes(), before);
                assert!(budget.work_ledger_identity_v1() == ledger);
                drop(report);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
#[test]
fn source_loop_induction_query_noop_roster_is_exact_and_replayed_both_owners_and_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_owner(erased, profile, false, |owner, budget| {
                let floor = budget.storage();
                let (report, storage) = query(owner, Limits::default(), budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert!(report.rows().is_empty());
                report.replay(Limits::default(), budget).unwrap();
                fresh_agreement(owner, &report, Limits::default(), budget);
                drop(report);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
#[test]
fn source_loop_induction_report_rejects_equal_graph_foreign_source_owner_and_profile() {
    for erased in [false, true] {
        with_owner(erased, Profile::Gfx942, true, |owner, budget| {
            let (report, storage) = query(owner, Limits::default(), budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            for (other_erased, other_profile) in [
                (erased, Profile::Gfx942),
                (erased, Profile::Gfx950),
                (!erased, Profile::Gfx942),
            ] {
                with_owner(other_erased, other_profile, true, |other, _| {
                    assert!(!std::ptr::eq(owner, other));
                    if other_erased == erased && other_profile == Profile::Gfx942 {
                        assert_eq!(
                            owner.output().canonical().canonical_bytes(),
                            other.output().canonical().canonical_bytes()
                        );
                    } else {
                        assert_ne!(
                            owner.output().canonical().canonical_bytes(),
                            other.output().canonical().canonical_bytes()
                        );
                    }
                    let accepted = budget.work();
                    let peak = budget.peak_storage();
                    assert!(matches!(
                        replay_with(other, &report, Limits::default(), budget),
                        Err(QueryError::ForeignOwner)
                    ));
                    assert_eq!(budget.work(), accepted + 4);
                    assert_eq!((budget.storage(), budget.peak_storage()), (floor, peak));
                });
            }
            report.replay(Limits::default(), budget).unwrap();
            drop(report);
            budget.release_storage(storage.retained_storage()).unwrap();
        });
    }
}
#[test]
fn source_loop_induction_report_rejects_all_changed_limits_before_fresh_source_replay() {
    with_owner(false, Profile::Gfx942, true, |owner, budget| {
        let (report, storage) = query(owner, Limits::default(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        for field in 0..7 {
            for relax in [false, true] {
                let mut limits = Limits::default();
                let value = match field {
                    0 => &mut limits.functions,
                    1 => &mut limits.blocks,
                    2 => &mut limits.edges,
                    3 => &mut limits.definitions,
                    4 => &mut limits.operations,
                    5 => &mut limits.loops,
                    6 => &mut limits.rows,
                    _ => unreachable!(),
                };
                *value = if relax { *value + 1 } else { *value - 1 };
                let accepted = budget.work();
                let peak = budget.peak_storage();
                assert!(matches!(
                    report.replay(limits, budget),
                    Err(QueryError::LimitsMismatch)
                ));
                assert_eq!(budget.work(), accepted + 11);
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, peak));
            }
        }
        report.replay(Limits::default(), budget).unwrap();
        drop(report);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}
#[test]
fn source_loop_induction_query_rejects_actual_output_limits_without_partial_transfer() {
    with_owner(false, Profile::Gfx942, true, |owner, budget| {
        let floor = budget.storage();
        let limits = Limits {
            functions: 0,
            ..Limits::default()
        };
        assert!(matches!(
            query(owner, limits, budget),
            Err(QueryError::Analysis(
                fe2o3_kernel_analysis::CanonicalKirInductionErrorV1::Loops(
                    fe2o3_kernel_analysis::CanonicalKirLoopErrorV1::InputLimit {
                        kind: "functions",
                        actual: 2,
                        limit: 0
                    }
                )
            ))
        ));
        assert_eq!(budget.storage(), floor);
        let (report, storage) = query(owner, Limits::default(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(report.rows().len(), 2);
        drop(report);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[path = "production_pipeline_licm_loop_induction_resources_v1_tests.rs"]
mod resources_tests;
