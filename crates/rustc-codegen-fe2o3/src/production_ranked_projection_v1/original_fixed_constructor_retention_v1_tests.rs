// Synthetic constructor/resource controls only. No genuine Fixed or F2 claim.
mod original_fixed_constructor_retention_controls {
    use super::*;
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::original_fixed_constructor_retention::*;
    use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::with_rich_tables_for_test_v1;
    use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work};
    use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as QueryError;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    const LIMIT: usize = 256 * 1024 * 1024;
    const FLOOR: usize = 43;
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Mode {
        Original,
        Retained,
        Retry,
    }
    #[derive(Debug)]
    struct Report {
        entered: bool,
        error: Option<String>,
        result: Option<Snapshot>,
        prefix_work: usize,
        prefix_storage: usize,
        work: usize,
        owned: usize,
        failed_work: bool,
        failed_storage: bool,
        witness: Witness,
        checkpoints: usize,
        last: ConstructorStage,
        same_panic: bool,
        outer_panic: bool,
        retained_after_postflight: bool,
        floor_restored: bool,
    }
    fn fixture(kind: usize) -> SemanticFunctionDeclV1 {
        if kind == 0 {
            fixed_guard_function(FixedGuardOptions {
                extent: 4,
                ..Default::default()
            })
        } else {
            checked_arithmetic_chain_function(false)
        }
    }
    fn run(
        kind: usize,
        mode: Mode,
        work_limit: usize,
        storage_limit: usize,
        panic_at: Option<usize>,
        wrong_graph: bool,
        foreign_function: bool,
    ) -> Report {
        let types = assertion_proof_types();
        let function = fixture(kind);
        let other_function = fixture(kind);
        let mut graph = projected_loop_cfg_graph_v1(&function).unwrap();
        if wrong_graph {
            graph.entry = function.blocks().len();
        }
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let address = &budget as *const Budget<'_> as usize;
        let mut owned = 0;
        let mut retired = None;
        let mut cuts = ConstructorCuts::new(panic_at);
        let mut witness = Witness::default();
        let mut saved_error = None;
        let mut result = None;
        let mut entered = false;
        let mut prefix_work = 0;
        let mut prefix_storage = 0;
        let mut core_work = 0;
        let mut same_panic = false;
        let outer =
            with_rich_tables_for_test_v1(&[], &types, &function, &mut budget, |rich, budget| {
                entered = true;
                prefix_work = budget.work();
                prefix_storage = budget.storage();
                let selected = if foreign_function {
                    &other_function
                } else {
                    &function
                };
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                    if mode == Mode::Original {
                        original_reference(&types, selected, &graph, rich, &mut resources)
                    } else {
                        retained_constructor(
                            &types,
                            selected,
                            &graph,
                            rich,
                            &mut resources,
                            &mut retired,
                            &mut cuts,
                            &mut witness,
                            mode == Mode::Retry,
                        )
                    }
                }));
                core_work = budget.work() - prefix_work;
                assert_eq!(budget.storage(), prefix_storage + owned);
                match outcome {
                    Ok(Ok(value)) => {
                        result = Some(value);
                        Ok(())
                    }
                    Ok(Err(error)) => {
                        // Preserve actual backend error as owned component DATA;
                        // this synthetic outer mapping is not a genuine route claim.
                        saved_error = Some(error);
                        Err(QueryError::Unavailable(
                            "original constructor component refusal",
                        ))
                    }
                    Err(payload) => {
                        same_panic = Some(payload.as_ref() as *const (dyn std::any::Any + Send)
                            as *const () as usize)
                            == cuts.original_address
                            && payload.downcast_ref::<[u64; 2]>()
                                == Some(&[0x6f726967696e616c, 0x72657461696e6564]);
                        assert!(same_panic, "same original constructor checkpoint panic box");
                        assert!(witness.installed);
                        assert_eq!(witness.before, witness.after);
                        resume_unwind(payload)
                    }
                }
            });
        let outer_panic = matches!(outer, Err(QueryError::CallbackPanicked));
        assert_eq!(address, &budget as *const Budget<'_> as usize);
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), FLOOR + owned);
        let retained_after_postflight = retired.is_some();
        if let Some(payload) = &retired {
            assert_eq!(
                Some(payload.snapshot(witness.before.unwrap().phase)),
                witness.before
            );
            assert_eq!(witness.before, witness.after);
        }
        let failed_work = budget.failed_work().is_some();
        let failed_storage = budget.failed_storage().is_some();
        let error = saved_error.as_ref().map(|error| format!("{error:?}"));
        drop(saved_error);
        drop(retired);
        let checkpoints = cuts.seen;
        let last = cuts.last;
        drop(cuts);
        budget.release_storage(owned).unwrap();
        let floor_restored = budget.storage() == FLOOR;
        Report {
            entered,
            error,
            result,
            prefix_work,
            prefix_storage,
            work: core_work,
            owned,
            failed_work,
            failed_storage,
            witness,
            checkpoints,
            last,
            same_panic,
            outer_panic,
            retained_after_postflight,
            floor_restored,
        }
    }
    fn shape(
        value: Snapshot,
    ) -> (
        usize,
        usize,
        Vec<(usize, usize)>,
        Option<(u8, usize, usize)>,
        Option<(u8, usize, usize)>,
        [bool; 6],
    ) {
        assert!(value.complete_rows);
        (
            value.checked.1,
            value.checked.2,
            value.rows[..value.checked.1]
                .iter()
                .map(|r| (r.1, r.2))
                .collect(),
            value.dominance.map(|(kind, _, len, cap)| (kind, len, cap)),
            value.zero.map(|(kind, _, len, cap)| (kind, len, cap)),
            value.side,
        )
    }
    fn totals(events: &[Debit]) -> (usize, usize) {
        events
            .iter()
            .fold((0, 0), |(work, storage), event| match event {
                Debit::Work(n) => (work + n, storage),
                Debit::Storage(n) => (work, storage + n),
            })
    }
    #[test]
    fn original_retained_constructor_matches_original_success_and_complete_debit_totals() {
        for kind in 0..2 {
            let original = run(kind, Mode::Original, LIMIT, LIMIT, None, false, false);
            let retained = run(kind, Mode::Retained, LIMIT, LIMIT, None, false, false);
            assert!(original.entered && retained.entered);
            assert!(original.error.is_none() && retained.error.is_none());
            assert_eq!(
                shape(original.result.unwrap()),
                shape(retained.result.unwrap())
            );
            let (work, storage) = totals(&expected_debits(&fixture(kind)).unwrap());
            assert_eq!(
                (original.work, original.owned),
                (
                    added_frame().unwrap() + work,
                    added_frame().unwrap() + storage
                )
            );
            assert_eq!(
                (retained.work, retained.owned),
                (original.work, original.owned)
            );
            assert!(
                retained.witness.admitted
                    && retained.witness.installed
                    && retained.retained_after_postflight
                    && retained.floor_restored
            );
            assert_eq!(retained.witness.before, retained.witness.after);
            assert_eq!(retained.result.unwrap().side, [false; 6]);
            if kind == 1 {
                assert!(retained.result.unwrap().rows.iter().any(|row| row.1 > 0));
            }
        }
    }
    #[test]
    fn original_retained_constructor_every_positive_original_debit_one_short_matches_refusal_prefix()
     {
        let kind = 1;
        let baseline = run(kind, Mode::Retained, LIMIT, LIMIT, None, false, false);
        assert!(baseline.result.is_some());
        let prefix = added_frame().unwrap();
        let mut work = 0;
        let mut storage = 0;
        let mut denied = 0;
        for event in expected_debits(&fixture(kind)).unwrap() {
            let limits = match event {
                Debit::Work(n) if n > 0 => {
                    Some((baseline.prefix_work + prefix + work + n - 1, LIMIT, true))
                }
                Debit::Storage(n) if n > 0 => Some((
                    LIMIT,
                    baseline.prefix_storage + prefix + storage + n - 1,
                    false,
                )),
                _ => None,
            };
            if let Some((w, s, is_work)) = limits {
                let original = run(kind, Mode::Original, w, s, None, false, false);
                let retained = run(kind, Mode::Retained, w, s, None, false, false);
                assert!(original.entered && retained.entered);
                assert!(original.error.is_some() && retained.error.is_some());
                assert_eq!(original.error, retained.error);
                assert_eq!(
                    (retained.work, retained.owned),
                    (original.work, original.owned)
                );
                assert_eq!(
                    (retained.failed_work, retained.failed_storage),
                    (is_work, !is_work)
                );
                assert!(
                    retained.witness.installed
                        && retained.retained_after_postflight
                        && retained.floor_restored
                );
                assert_eq!(retained.witness.before, retained.witness.after);
                denied += 1;
            }
            match event {
                Debit::Work(n) => work += n,
                Debit::Storage(n) => storage += n,
            }
        }
        assert!(
            denied > 20,
            "all original frame and payload debit cuts, not a small sample"
        );
    }
    #[test]
    fn original_retained_constructor_prefix_denial_never_creates_owner_or_panic_payload() {
        let baseline = run(0, Mode::Retained, LIMIT, LIMIT, None, false, false);
        let prefix = added_frame().unwrap();
        for (w, s) in [
            (baseline.prefix_work + prefix - 1, LIMIT),
            (LIMIT, baseline.prefix_storage + prefix - 1),
        ] {
            let refused = run(0, Mode::Retained, w, s, Some(1), false, false);
            assert!(refused.entered && refused.error.is_some());
            assert!(
                !refused.witness.admitted
                    && !refused.witness.installed
                    && !refused.retained_after_postflight
            );
            assert_eq!(refused.checkpoints, 0);
            assert!(refused.floor_restored);
        }
    }
    #[test]
    fn original_retained_constructor_every_closed_checkpoint_keeps_same_panic_and_partial_owner() {
        let baseline = run(1, Mode::Retained, LIMIT, LIMIT, None, false, false);
        assert!(baseline.checkpoints > 12);
        let mut outer_capacity = false;
        let mut row_capacity = false;
        let mut first_cache_only = false;
        for cut in 1..=baseline.checkpoints {
            let report = run(1, Mode::Retained, LIMIT, LIMIT, Some(cut), false, false);
            assert!(report.same_panic && report.outer_panic && report.witness.installed);
            assert!(report.retained_after_postflight && report.floor_restored);
            assert_eq!(report.witness.before, report.witness.after);
            let snapshot = report.witness.before.unwrap();
            outer_capacity |= snapshot.checked.2 > 0 && snapshot.checked.1 == 0;
            row_capacity |= snapshot.rows.iter().any(|row| row.2 > 0);
            first_cache_only |= snapshot.dominance.is_some() && snapshot.zero.is_none();
        }
        assert!(outer_capacity && row_capacity && first_cache_only);
        // These are closed constructor-boundary panics, not instrumented meter panics.
    }
    #[test]
    fn original_retained_constructor_original_source_and_shape_refusal_priority() {
        for (wrong_graph, foreign_function) in [(true, false), (false, true)] {
            let original = run(
                0,
                Mode::Original,
                LIMIT,
                LIMIT,
                None,
                wrong_graph,
                foreign_function,
            );
            let retained = run(
                0,
                Mode::Retained,
                LIMIT,
                LIMIT,
                None,
                wrong_graph,
                foreign_function,
            );
            assert!(original.error.is_some());
            assert_eq!(original.error, retained.error);
            assert_eq!(
                (original.work, original.owned),
                (retained.work, retained.owned)
            );
            assert!(retained.witness.installed && retained.retained_after_postflight);
            assert_eq!(retained.witness.before, retained.witness.after);
            assert!(!retained.failed_work && !retained.failed_storage);
        }
    }
    #[test]
    fn original_retained_constructor_second_prepare_is_terminal_without_payload_loss() {
        let normal = run(1, Mode::Retained, LIMIT, LIMIT, None, false, false);
        let retry = run(1, Mode::Retry, LIMIT, LIMIT, None, false, false);
        assert!(retry.error.as_deref().unwrap().contains("Accounting"));
        assert_eq!((retry.work, retry.owned), (normal.work, normal.owned));
        assert_eq!(
            shape(retry.witness.after.unwrap()),
            shape(normal.witness.after.unwrap())
        );
        assert!(retry.retained_after_postflight && retry.floor_restored);
    }
    #[test]
    fn original_retained_constructor_occupied_slot_rejects_foreign_retry_without_overwrite() {
        let types = assertion_proof_types();
        let function = fixture(1);
        let graph = projected_loop_cfg_graph_v1(&function).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut retired = None;
        let mut cuts = ConstructorCuts::new(None);
        let mut witness = Witness::default();
        with_rich_tables_for_test_v1(&[], &types, &function, &mut budget, |rich, budget| {
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                retained_constructor(
                    &types,
                    &function,
                    &graph,
                    rich,
                    &mut resources,
                    &mut retired,
                    &mut cuts,
                    &mut witness,
                    false,
                )
                .unwrap();
            }
            let before = retired.as_ref().unwrap().snapshot(2);
            let mut foreign_work = Work::new(LIMIT);
            let mut foreign_budget = Budget::new(&mut foreign_work, LIMIT);
            let mut foreign_owned = 0;
            let mut second = Witness::default();
            let mut foreign_cuts = ConstructorCuts::new(Some(1));
            let error = {
                let mut resources =
                    PreparationResourcesV1::new(&mut foreign_budget, &mut foreign_owned);
                retained_constructor(
                    &types,
                    &function,
                    &graph,
                    rich,
                    &mut resources,
                    &mut retired,
                    &mut foreign_cuts,
                    &mut second,
                    false,
                )
                .unwrap_err()
            };
            assert!(format!("{error:?}").contains("Accounting"));
            assert!(!second.admitted && !second.installed);
            assert_eq!(
                (
                    foreign_budget.work(),
                    foreign_budget.storage(),
                    foreign_owned
                ),
                (0, 0, 0)
            );
            assert_eq!(foreign_cuts.seen, 0);
            assert_eq!(retired.as_ref().unwrap().snapshot(2), before);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), owned);
        drop(retired);
        drop(cuts);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn original_retained_constructor_unmetered_and_preexisting_denial_remain_inert() {
        let types = assertion_proof_types();
        let function = fixture(0);
        let graph = projected_loop_cfg_graph_v1(&function).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        with_rich_tables_for_test_v1(&[], &types, &function, &mut budget, |rich, budget| {
            let mut retired = None;
            let mut cuts = ConstructorCuts::new(Some(1));
            let mut witness = Witness::default();
            {
                let mut unmetered = PreparationResourcesV1::unmetered();
                assert!(
                    retained_constructor(
                        &types,
                        &function,
                        &graph,
                        rich,
                        &mut unmetered,
                        &mut retired,
                        &mut cuts,
                        &mut witness,
                        false
                    )
                    .is_err()
                );
            }
            assert!(!witness.admitted && retired.is_none() && cuts.original_address.is_none());
            assert!(budget.charge_work(LIMIT).is_err());
            let before = (budget.work(), budget.storage(), budget.failed_work());
            let mut owned = 0;
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                assert!(
                    retained_constructor(
                        &types,
                        &function,
                        &graph,
                        rich,
                        &mut resources,
                        &mut retired,
                        &mut cuts,
                        &mut witness,
                        false
                    )
                    .is_err()
                );
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.failed_work()),
                before
            );
            assert_eq!(owned, 0);
            assert!(retired.is_none() && cuts.original_address.is_none());
            Err::<(), _>(QueryError::Unavailable(
                "intentional sticky-denial component",
            ))
        })
        .unwrap_err();
        assert!(budget.failed_work().is_some());
        assert_eq!(budget.storage(), 0);
    }
}
