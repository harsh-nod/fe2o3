use super::*;
use crate::{
    FormalPaidPathDecisionV20 as PaidDecision, FormalPaidPathErrorV20 as PaidError,
    PresburgerQueryErrorV2 as QueryError, PresburgerQueryLimitsV2, PresburgerQueryResourceV2,
    with_formal_path_observations_v20, with_presburger_queries_v2,
};
use fe2o3_kernel_ir::{
    CanonicalEffectErrorV19, CanonicalFormalLaunchInputV19 as Launch,
    CanonicalFormalReportErrorV19, CanonicalFormalSourceScopeV20,
    CanonicalKernelIrVerificationResourceErrorV1 as LedgerError, ControlFlowLimits,
    FormalGuardedMemoryResourceErrorV1, SwitchCase, with_canonical_effects_v19,
    with_canonical_owner_formal_report_v19,
};
use std::cell::Cell;

#[path = "formal_path_paid_owned_session_v21_tests.rs"]
mod owned_session_v21_tests;

const LIMIT: usize = 20_000_000;
const FLOOR: usize = 37;

#[derive(Debug, Eq, PartialEq)]
enum Failure {
    Effects(CanonicalEffectErrorV19),
    Report(CanonicalFormalReportErrorV19),
    Query(QueryError),
    Path(PaidError),
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Summary {
    rows: usize,
    excluded: usize,
    not_proved: usize,
    reasons: usize,
    queries: usize,
    callbacks: usize,
}

fn run(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    input: Launch,
    width: FormalIndexWidth,
    repeats: usize,
    work_limit: usize,
    storage_limit: usize,
    limits: PresburgerQueryLimitsV2,
) -> (std::result::Result<Summary, Failure>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut result = None;
    let attempts = Cell::new(0);
    let settled = Cell::new(0);
    let effects = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
        let report = with_canonical_owner_formal_report_v19(
            owner,
            0,
            effects,
            input,
            width,
            ControlFlowLimits::DEFAULT,
            budget,
            |report, budget| {
                let source = report.source_scope_v20(budget)?;
                let mut path = None;
                let session = with_presburger_queries_v2(limits, budget, |queries, budget| {
                    let mut summary = Summary::default();
                    for _ in 0..repeats {
                        let floor = budget.storage();
                        let completed = Cell::new(false);
                        attempts.set(attempts.get() + 1);
                        let observation = with_formal_path_observations_v20(
                            source,
                            queries,
                            budget,
                            |view, _| {
                                assert!(std::ptr::eq(view.original_owner(), owner));
                                assert!(std::ptr::eq(
                                    view.original_function(),
                                    report.original_function()
                                ));
                                assert!(std::ptr::eq(view.analysis(), report.analysis()));
                                assert_eq!(view.root_index(), 0);
                                assert_eq!(view.launch_input(), input);
                                assert_eq!(view.index_width(), width);
                                let original =
                                    report.analysis().obligations().inter_invocation_conflicts();
                                assert_eq!(view.observations().len(), original.len());
                                for (ordinal, row) in view.observations().iter().enumerate() {
                                    assert_eq!(row.ordinal(), ordinal);
                                    assert_eq!(row.requirement(), original[ordinal]);
                                    match row.decision() {
                                        PaidDecision::ExcludedForAllU64Coordinates => {
                                            summary.excluded += 1
                                        }
                                        PaidDecision::NotProved => summary.not_proved += 1,
                                    }
                                }
                                summary.rows += original.len();
                                summary.reasons += report.analysis().incomplete_reasons().len();
                                summary.callbacks += 1;
                                completed.set(true);
                                Ok(())
                            },
                        );
                        assert_eq!(budget.storage(), floor);
                        if observation.is_ok() {
                            assert!(completed.get(), "caught callback assertions cannot pass");
                        }
                        settled.set(settled.get() + 1);
                        match observation {
                            Ok(()) => {}
                            Err(error) => {
                                path = Some(Err(error));
                                return Ok(());
                            }
                        }
                    }
                    summary.queries = queries.usage(budget)?.queries;
                    path = Some(Ok(summary));
                    Ok(())
                });
                result = Some(match session {
                    Err(error) => Err(Failure::Query(error)),
                    Ok(()) => path
                        .expect("the session must finish its attempt")
                        .map_err(Failure::Path),
                });
                Ok(())
            },
        );
        if let Err(error) = report {
            result = Some(Err(Failure::Report(error)));
        }
        Ok(())
    });
    if let Err(error) = effects {
        result = Some(Err(Failure::Effects(error)));
    }
    assert_eq!(
        attempts.get(),
        settled.get(),
        "every attempted path query must finish its post-refund assertions"
    );
    assert_eq!(budget.storage(), FLOOR);
    (
        result.expect("a construction or observation must finish"),
        budget.work(),
        budget.peak_storage(),
    )
}

fn normal(owner: &VerifiedCanonicalKernelIrModuleV18) -> Summary {
    run(
        owner,
        Launch::Exact(launch(64)),
        FormalIndexWidth::Bits64,
        1,
        LIMIT,
        LIMIT,
        Default::default(),
    )
    .0
    .unwrap()
}

fn comparison(module: &mut Module, predicate: ComparePredicate) {
    let OperationKind::Compare {
        predicate: actual, ..
    } = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        panic!("fixture compare");
    };
    *actual = predicate;
}

#[test]
fn paid_actual_source_singleton_keeps_the_exact_original_report_and_full_u64_domain() {
    for (bound, predicate, excluded) in [
        (1, ComparePredicate::LessThan, 1),
        (2, ComparePredicate::LessThan, 0),
        (u64::MAX, ComparePredicate::GreaterThanOrEqual, 1),
        (u64::MAX - 1, ComparePredicate::GreaterThanOrEqual, 0),
    ] {
        let mut module = fixture(bound);
        comparison(&mut module, predicate);
        with_owner(&module, |owner| {
            let summary = normal(owner);
            assert_eq!(
                summary,
                Summary {
                    rows: 1,
                    excluded,
                    not_proved: 1 - excluded,
                    reasons: 0,
                    queries: 2,
                    callbacks: 1
                }
            );
        });
    }
}

#[test]
fn paid_actual_source_joins_all_conflict_rows_and_both_invocation_orders() {
    let mut module = fixture(1);
    let body = module.functions[0].body.as_mut().unwrap();
    let store = body.blocks[1].operations[0].clone();
    body.blocks[2].operations.push(store);
    with_owner(&module, |owner| {
        assert_eq!(
            normal(owner),
            Summary {
                rows: 3,
                excluded: 1,
                not_proved: 2,
                reasons: 0,
                queries: 6,
                callbacks: 1
            }
        );
    });
}

#[test]
fn paid_actual_source_never_promotes_incomplete_width_or_multidimensional_reports() {
    with_owner(&fixture(1), |owner| {
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Unknown] {
            let summary = run(
                owner,
                Launch::Exact(launch(64)),
                width,
                1,
                LIMIT,
                LIMIT,
                Default::default(),
            )
            .0
            .unwrap();
            assert_eq!(summary.excluded, 0);
            assert_eq!(summary.not_proved, summary.rows);
            assert_eq!(summary.queries, 0);
        }
    });
    for (domain, rank, extents) in [
        (
            LaunchDomain::D2 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
            },
            2,
            [64, 2, 1],
        ),
        (
            LaunchDomain::D3 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
                z: LaunchExtent::Dynamic,
            },
            3,
            [64, 1, 2],
        ),
    ] {
        let mut module = fixture(1);
        module.kernels[0].domain = domain;
        with_owner(&module, |owner| {
            let summary = run(
                owner,
                Launch::Exact(ExplicitLaunchExtent::Exact { rank, extents }),
                FormalIndexWidth::Bits64,
                1,
                LIMIT,
                LIMIT,
                Default::default(),
            )
            .0
            .unwrap();
            assert_eq!(
                (summary.excluded, summary.not_proved, summary.queries),
                (0, summary.rows, 0)
            );
        });
    }
    let mut module = fixture(1);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "external".into(),
                    arguments: vec![],
                },
            ),
        );
    module.functions.push(Function::external_import(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_owner(&module, |owner| {
        let summary = normal(owner);
        assert!(summary.reasons > 0);
        assert_eq!(
            (summary.excluded, summary.not_proved, summary.queries),
            (0, summary.rows, 0)
        );
    });
}

#[test]
fn paid_actual_source_refuses_small_launch_no_wrap_assumptions() {
    let mut module = fixture(2);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.insert(
        2,
        value(8, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
    );
    operations.insert(
        3,
        value(
            9,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ),
    );
    let OperationKind::Compare { lhs, .. } = &mut operations[4].kind else {
        unreachable!()
    };
    *lhs = ValueId(9);
    with_owner(&module, |owner| {
        let summary = normal(owner);
        assert_eq!(
            (summary.excluded, summary.not_proved, summary.queries),
            (0, 1, 2)
        );
    });
}

#[test]
fn paid_actual_source_repeated_queries_have_exact_and_one_short_shared_bounds() {
    with_owner(&fixture(1), |owner| {
        let input = Launch::Exact(launch(64));
        let full = run(
            owner,
            input,
            FormalIndexWidth::Bits64,
            3,
            LIMIT,
            LIMIT,
            Default::default(),
        );
        assert_eq!(full.0.as_ref().unwrap().callbacks, 3);
        assert_eq!(full.0.as_ref().unwrap().queries, 6);
        let exact = run(
            owner,
            input,
            FormalIndexWidth::Bits64,
            3,
            full.1,
            full.2,
            Default::default(),
        );
        assert_eq!(exact, full);
        for (work, storage, work_denial) in
            [(full.1 - 1, full.2, true), (full.1, full.2 - 1, false)]
        {
            let denied = run(
                owner,
                input,
                FormalIndexWidth::Bits64,
                3,
                work,
                storage,
                Default::default(),
            );
            match denied.0 {
                Err(Failure::Effects(CanonicalEffectErrorV19::Resource(LedgerError::Work(
                    error,
                )))) if work_denial => {
                    assert!(error.actual() > error.limit());
                    assert_eq!(error.limit(), work);
                }
                Err(Failure::Effects(CanonicalEffectErrorV19::Resource(LedgerError::Storage(
                    error,
                )))) if !work_denial => {
                    assert!(error.actual() > error.limit());
                    assert_eq!(error.limit(), storage);
                }
                other => panic!("wrong one-short original account error: {other:?}"),
            }
        }
        let once = run(
            owner,
            input,
            FormalIndexWidth::Bits64,
            1,
            LIMIT,
            LIMIT,
            Default::default(),
        );
        assert!(full.1 > once.1);
        assert_eq!(full.2, once.2);
    });
}

#[test]
fn paid_actual_source_solver_local_cap_is_an_error_not_a_path_exclusion() {
    with_owner(&fixture(1), |owner| {
        let mut limits = PresburgerQueryLimitsV2::default();
        limits.queries = 1;
        let result = run(
            owner,
            Launch::Exact(launch(64)),
            FormalIndexWidth::Bits64,
            1,
            LIMIT,
            LIMIT,
            limits,
        );
        assert!(matches!(
            result.0,
            Err(Failure::Query(QueryError::Limit {
                resource: PresburgerQueryResourceV2::Queries,
                actual: 2,
                limit: 1
            }))
        ));
    });
}

#[test]
fn paid_actual_source_boolean_switch_backedge_and_reconvergence_keep_cfg_obligations() {
    for mode in 0..6 {
        let mut module = fixture(1);
        let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
        if mode <= 1 {
            blocks[0].operations.push(value(
                8,
                Type::BOOL,
                OperationKind::Unary {
                    op: UnaryOp::Not,
                    operand: ValueId(4),
                },
            ));
            blocks[0].terminator = Some(if mode == 0 {
                Terminator::ConditionalBranch {
                    condition: ValueId(8),
                    then_target: BlockId(2),
                    then_arguments: vec![],
                    else_target: BlockId(1),
                    else_arguments: vec![],
                }
            } else {
                blocks[0].operations.push(value(
                    9,
                    Type::Scalar(ScalarType::U64),
                    OperationKind::Cast {
                        kind: CastKind::ZeroExtend,
                        value: ValueId(8),
                        to: Type::Scalar(ScalarType::U64),
                    },
                ));
                Terminator::Switch {
                    selector: ValueId(9),
                    cases: vec![SwitchCase {
                        value: 1,
                        target: BlockId(2),
                        arguments: vec![],
                    }],
                    default_target: BlockId(1),
                    default_arguments: vec![],
                }
            });
        } else if mode == 2 {
            blocks[1].terminator = Some(Terminator::Branch {
                target: BlockId(0),
                arguments: vec![],
            });
        } else if mode == 3 {
            blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(4),
                then_target: BlockId(1),
                then_arguments: vec![],
                else_target: BlockId(1),
                else_arguments: vec![],
            });
        } else if mode == 4 {
            blocks[2].terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
        } else {
            let mut dead = BasicBlock::new(BlockId(3));
            dead.terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
            blocks.push(dead);
        }
        with_owner(&module, |owner| {
            let result = normal(owner);
            assert_eq!(result.rows, 1);
            assert_eq!(result.excluded, usize::from(mode <= 2));
            assert_eq!(result.not_proved, usize::from(mode > 2));
        });
    }
}

fn in_source<'owner>(
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    consume: impl for<'report, 'work> FnOnce(
        &CanonicalFormalSourceScopeV20<'report, 'owner>,
        &mut Budget<'work>,
    ),
) -> (
    std::result::Result<(), CanonicalEffectErrorV19>,
    std::result::Result<(), CanonicalFormalReportErrorV19>,
) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let completed = Cell::new(false);
    let mut report_result = None;
    let result = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
        report_result = Some(with_canonical_owner_formal_report_v19(
            owner,
            0,
            effects,
            Launch::Exact(launch(64)),
            FormalIndexWidth::Bits64,
            ControlFlowLimits::DEFAULT,
            budget,
            |view, budget| {
                consume(view.source_scope_v20(budget)?, budget);
                completed.set(true);
                Ok(())
            },
        ));
        Ok(())
    });
    assert!(
        completed.get(),
        "negative callback assertions must complete outside catches"
    );
    assert_eq!(budget.storage(), FLOOR);
    (result, report_result.unwrap())
}

#[test]
fn paid_actual_source_rejects_foreign_ledgers_without_poisoning_original_queries() {
    with_owner(&fixture(1), |owner| {
        let result = in_source(owner, |source, budget| {
            let completed = Cell::new(false);
            let session =
                with_presburger_queries_v2(Default::default(), budget, |queries, budget| {
                    let floor = budget.storage();
                    let mut foreign_work = Work::new(0);
                    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
                    assert!(foreign.charge_work(1).is_err());
                    let before = (foreign.work(), foreign.storage());
                    let rejected =
                        with_formal_path_observations_v20(source, queries, &mut foreign, |_, _| {
                            panic!("foreign source must not consume")
                        });
                    assert_eq!(
                        rejected,
                        Err(PaidError::Source(CanonicalFormalReportErrorV19::Resource(
                            FormalGuardedMemoryResourceErrorV1::Accounting
                        )))
                    );
                    assert_eq!((foreign.work(), foreign.storage()), before);
                    let seen = Cell::new(false);
                    with_formal_path_observations_v20(source, queries, budget, |view, _| {
                        assert_eq!(
                            view.observations()[0].decision(),
                            PaidDecision::ExcludedForAllU64Coordinates
                        );
                        seen.set(true);
                        Ok(())
                    })
                    .unwrap();
                    assert!(seen.get());
                    assert_eq!(budget.storage(), floor);
                    completed.set(true);
                    Ok(())
                });
            assert_eq!(session, Ok(()));
            assert!(completed.get());
        });
        assert_eq!(result, (Ok(()), Ok(())));
    });
}

#[test]
fn paid_actual_source_error_panic_and_captured_drop_cleanup_keep_original_floor() {
    struct PanicCapture;
    impl Drop for PanicCapture {
        fn drop(&mut self) {
            panic!("trusted test capture");
        }
    }
    with_owner(&fixture(1), |owner| {
        for mode in 0..3 {
            let result = in_source(owner, |source, budget| {
                let completed = Cell::new(false);
                let session =
                    with_presburger_queries_v2(Default::default(), budget, |queries, budget| {
                        let floor = budget.storage();
                        let observed =
                            with_formal_path_observations_v20(source, queries, budget, |_, _| {
                                if mode == 1 {
                                    panic!("trusted test callback");
                                }
                                if mode == 2 {
                                    drop(PanicCapture);
                                }
                                Err(PaidError::ConsumerRejected)
                            });
                        assert_eq!(
                            observed,
                            Err(if mode == 0 {
                                PaidError::ConsumerRejected
                            } else {
                                PaidError::Panicked
                            })
                        );
                        assert_eq!(budget.storage(), floor);
                        completed.set(true);
                        Ok(())
                    });
                assert_eq!(session, Ok(()));
                assert!(completed.get());
            });
            assert_eq!(result, (Ok(()), Ok(())));
        }
    });
}

#[test]
fn paid_actual_source_prior_work_denial_precedes_later_floor_undercut() {
    with_owner(&fixture(1), |owner| {
        let complete = Cell::new(false);
        let undercut = Cell::new(None);
        let result = in_source(owner, |source, budget| {
            let session =
                with_presburger_queries_v2(Default::default(), budget, |queries, budget| {
                    let floor = budget.storage();
                    let mut exact = None;
                    let path =
                        with_formal_path_observations_v20(source, queries, budget, |_, budget| {
                            let denied = budget.charge_work(LIMIT).unwrap_err();
                            exact = Some(denied);
                            let before = budget.storage();
                            budget.release_storage(1).unwrap();
                            assert_eq!(budget.storage() + 1, before);
                            undercut.set(Some(before - budget.storage()));
                            Ok(())
                        });
                    assert_eq!(path, Err(PaidError::Resource(exact.unwrap())));
                    // This intentionally undercut the path's retained rows, so its
                    // credit was not refunded. Restore only our deliberate byte;
                    // the enclosing original session owns its later cleanup.
                    budget.reserve_storage(1).unwrap();
                    assert!(budget.storage() > floor);
                    complete.set(true);
                    Ok(())
                });
            assert!(matches!(
                session,
                Err(QueryError::PriorDenial {
                    work: Some(_),
                    storage: None
                })
            ));
        });
        assert!(complete.get());
        assert_eq!(undercut.get(), Some(1));
        assert!(result.0.is_err());
        assert!(result.1.is_err());
    });
}

#[test]
fn paid_actual_source_empty_conflict_roster_does_not_erase_incomplete_reasons() {
    let mut module = fixture(1);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .clear();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "external".into(),
                    arguments: vec![],
                },
            ),
        );
    module.functions.push(Function::external_import(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_owner(&module, |owner| {
        let summary = normal(owner);
        assert_eq!(
            (
                summary.rows,
                summary.excluded,
                summary.not_proved,
                summary.queries
            ),
            (0, 0, 0, 0)
        );
        assert!(summary.reasons > 0);
        assert_eq!(summary.callbacks, 1);
        let result = in_source(owner, |source, budget| {
            let counting = CountingSource {
                inner: source,
                blocks: Cell::new(0),
                dominance: Cell::new(0),
            };
            let completed = Cell::new(false);
            let session =
                with_presburger_queries_v2(Default::default(), budget, |queries, budget| {
                    crate::formal_path_conflicts_v1::paid_scope_v20::with_queries(
                        &counting,
                        queries,
                        budget,
                        |view, _| {
                            assert!(view.observations().is_empty());
                            assert!(!view.analysis().is_complete());
                            completed.set(true);
                            Ok(())
                        },
                    )
                    .unwrap();
                    assert_eq!((counting.blocks.get(), counting.dominance.get()), (0, 0));
                    Ok(())
                });
            assert_eq!(session, Ok(()));
            assert!(completed.get());
        });
        assert_eq!(result, (Ok(()), Ok(())));
    });
}

struct CountingSource<'borrow, 'report, 'owner> {
    inner: &'borrow CanonicalFormalSourceScopeV20<'report, 'owner>,
    blocks: Cell<usize>,
    dominance: Cell<usize>,
}
impl<'owner> crate::formal_path_conflicts_v1::paid_engine_v20::Queries<'owner>
    for CountingSource<'_, '_, 'owner>
{
    fn analysis(&self) -> &FormalMemoryObligationAnalysis {
        self.inner.report()
    }
    fn original_owner(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.inner.original_owner()
    }
    fn original_function(&self) -> &Function {
        self.inner.original_function()
    }
    fn root_index(&self) -> usize {
        self.inner.root_index()
    }
    fn launch(&self) -> Launch {
        self.inner.launch_input()
    }
    fn width(&self) -> FormalIndexWidth {
        self.inner.index_width()
    }
    fn check(&self, budget: &mut Budget<'_>) -> std::result::Result<(), PaidError> {
        Ok(self
            .inner
            .check(self.inner.original_owner(), self.inner.root_index(), budget)?)
    }
    fn block_count(&self, budget: &mut Budget<'_>) -> std::result::Result<usize, PaidError> {
        Ok(self.inner.block_count(budget)?)
    }
    fn block_at(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<Option<&'owner BasicBlock>, PaidError> {
        self.blocks.set(self.blocks.get() + 1);
        Ok(self.inner.block_at(ordinal, budget)?)
    }
    fn block_ordinal(
        &self,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<Option<usize>, PaidError> {
        Ok(self.inner.block_ordinal(block, budget)?)
    }
    fn reachable(
        &self,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<bool, PaidError> {
        Ok(self.inner.reachable(block, budget)?)
    }
    fn definition_count(&self, budget: &mut Budget<'_>) -> std::result::Result<usize, PaidError> {
        Ok(self.inner.definition_count(budget)?)
    }
    fn definition(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<Option<(usize, Option<&'owner Operation>)>, PaidError> {
        Ok(self
            .inner
            .definition(value, budget)?
            .map(|(ordinal, _, operation)| (ordinal, operation)))
    }
    fn unique_predecessor_dominates(
        &self,
        source: BlockId,
        target: BlockId,
        access: BlockId,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<bool, PaidError> {
        self.dominance.set(self.dominance.get() + 1);
        Ok(self
            .inner
            .unique_predecessor_dominates(source, target, access, budget)?)
    }
}

#[test]
fn paid_actual_source_wide_accesses_share_one_census_and_one_domain_per_block() {
    for stores in [1, 2, 8, 16] {
        let mut module = fixture(1);
        let block = &mut module.functions[0].body.as_mut().unwrap().blocks[1];
        let store = block.operations[0].clone();
        block.operations.resize(stores, store);
        with_owner(&module, |owner| {
            let result = in_source(owner, |source, budget| {
                let counting = CountingSource {
                    inner: source,
                    blocks: Cell::new(0),
                    dominance: Cell::new(0),
                };
                let completed = Cell::new(false);
                let session =
                    with_presburger_queries_v2(Default::default(), budget, |queries, budget| {
                        let outcome = crate::formal_path_conflicts_v1::paid_scope_v20::with_queries(
                            &counting,
                            queries,
                            budget,
                            |view, _| {
                                assert_eq!(view.observations().len(), stores * (stores + 1) / 2);
                                assert!(view.observations().iter().all(|row| row.decision()
                                    == PaidDecision::ExcludedForAllU64Coordinates));
                                completed.set(true);
                                Ok(())
                            },
                        );
                        assert_eq!(outcome, Ok(()));
                        assert_eq!(counting.blocks.get(), 3);
                        assert_eq!(counting.dominance.get(), 2);
                        Ok(())
                    });
                assert_eq!(session, Ok(()));
                assert!(completed.get());
            });
            assert_eq!(result, (Ok(()), Ok(())));
        });
    }
}

#[test]
fn paid_actual_source_entry_denial_drains_owned_capture_without_replacing_first_error() {
    struct Capture<'a>(&'a Cell<usize>, [u8; 64]);
    impl Capture<'_> {
        fn first(&self) -> u8 {
            self.1[0]
        }
    }
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected owned capture");
        }
    }
    with_owner(&fixture(1), |owner| {
        for existing_work_denial in [false, true] {
            let completed = Cell::new(false);
            let result = in_source(owner, |source, budget| {
                let dropped = Cell::new(0);
                let called = Cell::new(false);
                let session = with_presburger_queries_v2(
                    Default::default(),
                    budget,
                    |queries, budget| {
                        let floor = budget.storage();
                        let padding = if existing_work_denial {
                            assert!(budget.charge_work(LIMIT).is_err());
                            0
                        } else {
                            let padding = LIMIT - floor;
                            budget.reserve_storage(padding).unwrap();
                            padding
                        };
                        let capture = Capture(&dropped, [7; 64]);
                        let called = &called;
                        let refused = with_formal_path_observations_v20(
                            source,
                            queries,
                            budget,
                            move |_, _| {
                                assert_eq!(capture.first(), 7);
                                called.set(true);
                                Ok(())
                            },
                        );
                        if existing_work_denial {
                            assert!(matches!(
                                refused,
                                Err(PaidError::Source(CanonicalFormalReportErrorV19::Resource(
                                    FormalGuardedMemoryResourceErrorV1::Work(_)
                                )))
                            ));
                        } else {
                            assert!(
                                matches!(refused, Err(PaidError::Resource(LedgerError::Storage(error))) if error.actual() > LIMIT && error.limit() == LIMIT)
                            );
                        }
                        assert_eq!(dropped.get(), 1);
                        assert!(!called.get());
                        assert_eq!(budget.storage(), floor + padding);
                        budget.release_storage(padding).unwrap();
                        completed.set(true);
                        Ok(())
                    },
                );
                assert!(matches!(session, Err(QueryError::PriorDenial { .. })));
            });
            assert!(completed.get());
            assert!(result.0.is_err());
            assert!(result.1.is_err());
        }
    });
}

#[test]
fn paid_path_errors_preserve_typed_source_query_and_resource_causes() {
    use std::error::Error as _;
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    let resource = budget.charge_work(1).unwrap_err();
    let source = CanonicalFormalReportErrorV19::Resource(resource.into());
    let query = QueryError::Resource(resource);
    let wrapped_source = PaidError::Source(source.clone());
    let wrapped_query = PaidError::Query(query.clone());
    let wrapped_resource = PaidError::Resource(resource);
    assert_eq!(
        wrapped_source
            .source()
            .unwrap()
            .downcast_ref::<CanonicalFormalReportErrorV19>(),
        Some(&source)
    );
    assert_eq!(
        wrapped_query.source().unwrap().downcast_ref::<QueryError>(),
        Some(&query)
    );
    assert_eq!(
        wrapped_resource
            .source()
            .unwrap()
            .downcast_ref::<LedgerError>(),
        Some(&resource)
    );
    for error in [
        PaidError::InconsistentOriginalReport,
        PaidError::ConsumerRejected,
        PaidError::Panicked,
    ] {
        assert!(error.source().is_none());
    }
}
