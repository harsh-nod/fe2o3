use super::*;
use crate::{with_formal_path_observations_v21, with_presburger_queries_v4};

fn run_owned(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    width: FormalIndexWidth,
    repeats: usize,
    work_limit: usize,
    storage_limit: usize,
    limits: PresburgerQueryLimitsV2,
) -> (std::result::Result<Summary, Failure>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let attempts = Cell::new(0);
    let settled = Cell::new(0);
    let mut summary = Summary::default();
    let mut selected = None;
    let session = with_presburger_queries_v4(limits, &mut budget, |queries, budget| {
        for _ in 0..repeats {
            let floor = budget.storage();
            attempts.set(attempts.get() + 1);
            let effects_result = with_canonical_effects_v19(owner, budget, |effects, budget| {
                let report_result = with_canonical_owner_formal_report_v19(
                    owner,
                    0,
                    effects,
                    Launch::Exact(launch(64)),
                    width,
                    ControlFlowLimits::DEFAULT,
                    budget,
                    |report, budget| {
                        let source = report.source_scope_v20(budget)?;
                        let result = with_formal_path_observations_v21(
                            source,
                            queries,
                            budget,
                            |view, _| {
                                assert!(std::ptr::eq(view.original_owner(), owner));
                                assert!(std::ptr::eq(view.analysis(), report.analysis()));
                                assert!(std::ptr::eq(
                                    view.original_function(),
                                    report.original_function()
                                ));
                                assert_eq!(view.root_index(), 0);
                                assert_eq!(view.index_width(), width);
                                assert_eq!(view.launch_input(), Launch::Exact(launch(64)));
                                let conflicts =
                                    report.analysis().obligations().inter_invocation_conflicts();
                                assert_eq!(view.observations().len(), conflicts.len());
                                for (ordinal, row) in view.observations().iter().enumerate() {
                                    assert_eq!(row.ordinal(), ordinal);
                                    assert_eq!(row.requirement(), conflicts[ordinal]);
                                    match row.decision() {
                                        PaidDecision::ExcludedForAllU64Coordinates => {
                                            summary.excluded += 1
                                        }
                                        PaidDecision::NotProved => summary.not_proved += 1,
                                    }
                                }
                                summary.rows += conflicts.len();
                                summary.reasons += report.analysis().incomplete_reasons().len();
                                summary.callbacks += 1;
                                Ok(())
                            },
                        );
                        if let Err(error) = result {
                            selected = Some(Failure::Path(error));
                        }
                        Ok(())
                    },
                );
                if selected.is_none() {
                    if let Err(error) = report_result {
                        selected = Some(Failure::Report(error));
                    }
                }
                Ok(())
            });
            if selected.is_none() {
                if let Err(error) = effects_result {
                    selected = Some(Failure::Effects(error));
                }
            }
            assert_eq!(budget.storage(), floor);
            settled.set(settled.get() + 1);
            if selected.is_some() {
                return Err(QueryError::CallbackRejected);
            }
        }
        summary.queries = queries.usage(budget)?.queries;
        Ok(())
    });
    assert_eq!(
        attempts.get(),
        settled.get(),
        "every attempted report chain must finish cleanup"
    );
    assert_eq!(budget.storage(), FLOOR);
    let result = match selected {
        Some(error) => Err(error),
        None => session.map(|()| summary).map_err(Failure::Query),
    };
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn owned_session_spans_genuine_report_construction_and_retains_all_original_rows() {
    for bound in [1, 2, u64::MAX] {
        with_owner(&fixture(bound), |owner| {
            let expected = normal(owner);
            let actual = run_owned(
                owner,
                FormalIndexWidth::Bits64,
                1,
                LIMIT,
                LIMIT,
                Default::default(),
            )
            .0
            .unwrap();
            assert_eq!(actual, expected);
        });
    }
    let mut module = fixture(1);
    let body = module.functions[0].body.as_mut().unwrap();
    let store = body.blocks[1].operations[0].clone();
    body.blocks[2].operations.push(store);
    with_owner(&module, |owner| {
        assert_eq!(
            run_owned(
                owner,
                FormalIndexWidth::Bits64,
                1,
                LIMIT,
                LIMIT,
                Default::default()
            )
            .0
            .unwrap(),
            normal(owner)
        );
    });
}

#[test]
fn owned_session_repeated_reports_share_query_history_and_exact_local_cap() {
    with_owner(&fixture(1), |owner| {
        let summary = run_owned(
            owner,
            FormalIndexWidth::Bits64,
            3,
            LIMIT,
            LIMIT,
            PresburgerQueryLimitsV2 {
                queries: 6,
                ..Default::default()
            },
        )
        .0
        .unwrap();
        assert_eq!(
            summary,
            Summary {
                rows: 3,
                excluded: 3,
                not_proved: 0,
                reasons: 0,
                queries: 6,
                callbacks: 3
            }
        );
        let error = run_owned(
            owner,
            FormalIndexWidth::Bits64,
            3,
            LIMIT,
            LIMIT,
            PresburgerQueryLimitsV2 {
                queries: 5,
                ..Default::default()
            },
        )
        .0
        .unwrap_err();
        assert_eq!(
            error,
            Failure::Path(PaidError::Query(QueryError::Limit {
                resource: PresburgerQueryResourceV2::Queries,
                actual: 6,
                limit: 5,
            }))
        );
    });
}

#[test]
fn owned_session_preserves_incomplete_reports_and_never_promotes_unknown_width() {
    with_owner(&fixture(1), |owner| {
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Unknown] {
            let summary = run_owned(owner, width, 2, LIMIT, LIMIT, Default::default())
                .0
                .unwrap();
            assert_eq!(summary.excluded, 0);
            assert_eq!(summary.not_proved, summary.rows);
            assert_eq!(summary.queries, 0);
            assert_eq!(summary.callbacks, 2);
        }
    });
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
        let summary = run_owned(
            owner,
            FormalIndexWidth::Bits64,
            1,
            LIMIT,
            LIMIT,
            Default::default(),
        )
        .0
        .unwrap();
        assert_eq!(summary, normal(owner));
        assert!(summary.reasons > 0);
        assert_eq!(summary.excluded, 0);
    });
}

#[test]
fn owned_session_whole_report_chain_has_measured_exact_short_shared_resources() {
    with_owner(&fixture(1), |owner| {
        let (result, work, peak) = run_owned(
            owner,
            FormalIndexWidth::Bits64,
            2,
            LIMIT,
            LIMIT,
            Default::default(),
        );
        let expected = result.unwrap();
        let (result, exact_work, exact_peak) = run_owned(
            owner,
            FormalIndexWidth::Bits64,
            2,
            work,
            peak,
            Default::default(),
        );
        assert_eq!(result.unwrap(), expected);
        assert_eq!((exact_work, exact_peak), (work, peak));
        for (work_limit, storage_limit, is_work) in
            [(work - 1, peak, true), (work, peak - 1, false)]
        {
            let error = run_owned(
                owner,
                FormalIndexWidth::Bits64,
                2,
                work_limit,
                storage_limit,
                Default::default(),
            )
            .0
            .unwrap_err();
            let text = format!("{error:?}");
            // The chain must preserve a typed resource cause, not Panicked or a
            // bare callback sentinel. Exact nested adapter depends on the cut.
            fn resource(
                error: &(dyn std::error::Error + 'static),
                is_work: bool,
                limit: usize,
            ) -> bool {
                if let Some(error) = error.downcast_ref::<LedgerError>() {
                    return match error {
                        LedgerError::Work(error) if is_work => error.limit() == limit,
                        LedgerError::Storage(error) if !is_work => error.limit() == limit,
                        _ => false,
                    };
                }
                if let Some(error) = error.downcast_ref::<QueryError>() {
                    return match error {
                        QueryError::Resource(error) => resource(error, is_work, limit),
                        QueryError::PriorDenial {
                            work: Some(actual), ..
                        } if is_work => *actual > limit,
                        QueryError::PriorDenial {
                            storage: Some(actual),
                            ..
                        } if !is_work => *actual > limit,
                        _ => false,
                    };
                }
                error
                    .source()
                    .is_some_and(|source| resource(source, is_work, limit))
            }
            let found = match &error {
                Failure::Effects(error) => resource(
                    error,
                    is_work,
                    if is_work { work_limit } else { storage_limit },
                ),
                Failure::Report(error) => resource(
                    error,
                    is_work,
                    if is_work { work_limit } else { storage_limit },
                ),
                Failure::Query(error) => resource(
                    error,
                    is_work,
                    if is_work { work_limit } else { storage_limit },
                ),
                Failure::Path(error) => resource(
                    error,
                    is_work,
                    if is_work { work_limit } else { storage_limit },
                ),
            };
            assert!(found, "wrong typed resource refusal: {text}");
        }
    });
}

#[test]
fn owned_session_actual_source_keeps_large_paid_borrowed_backing_outside_solver_cap() {
    with_owner(&fixture(1), |owner| {
        let result = in_source(owner, |source, budget| {
            let completed = Cell::new(false);
            with_presburger_queries_v4(Default::default(), budget, |queries, budget| {
                let floor = budget.storage();
                const BYTES: usize = 80_000;
                budget.charge_work(BYTES + 3).unwrap();
                budget.reserve_storage(BYTES).unwrap();
                let mut backing = Vec::<u8>::new();
                backing.try_reserve_exact(BYTES).unwrap();
                budget.reserve_storage(backing.capacity() - BYTES).unwrap();
                backing.resize(BYTES, 17);
                let bytes = backing.capacity();
                for _ in 0..2 {
                    with_formal_path_observations_v21(source, queries, budget, |view, _| {
                        assert_eq!(view.observations().len(), 1);
                        assert_eq!(
                            view.observations()[0].decision(),
                            PaidDecision::ExcludedForAllU64Coordinates
                        );
                        assert!(std::ptr::eq(view.analysis(), source.report()));
                        Ok(())
                    })
                    .unwrap();
                    assert_eq!(budget.storage(), floor + bytes);
                    assert_eq!(backing[BYTES - 1], 17);
                }
                let usage = queries.usage(budget)?;
                assert_eq!(usage.queries, 4);
                assert!(usage.peak_scratch_bytes < BYTES);
                drop(backing);
                budget.release_storage(bytes).unwrap();
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok(())
            })
            .unwrap();
            assert!(completed.get());
        });
        assert_eq!(result, (Ok(()), Ok(())));
    });
}

#[test]
fn owned_session_source_facade_rejects_foreign_ledger_before_touching_solver() {
    with_owner(&fixture(1), |owner| {
        let result = in_source(owner, |source, budget| {
            let completed = Cell::new(false);
            with_presburger_queries_v4(Default::default(), budget, |queries, budget| {
                let floor = budget.storage();
                let mut foreign_work = Work::new(0);
                let mut foreign = Budget::new(&mut foreign_work, LIMIT);
                assert!(foreign.charge_work(1).is_err());
                let before = (foreign.work(), foreign.storage());
                let rejected =
                    with_formal_path_observations_v21(source, queries, &mut foreign, |_, _| {
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
                with_formal_path_observations_v21(source, queries, budget, |view, _| {
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
            })
            .unwrap();
            assert!(completed.get());
        });
        assert_eq!(result, (Ok(()), Ok(())));
    });
}
