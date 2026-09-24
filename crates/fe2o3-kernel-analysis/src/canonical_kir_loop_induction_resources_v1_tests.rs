use super::*;

#[derive(Debug)]
struct Measurement {
    result: Result<()>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(
    loops: &Loops<'_, '_>,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Measurement {
    let mut work = Work::new(work_limit);
    let (result, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = CanonicalKirInductionFactsV1::derive(loops, Limits::default(), &mut budget)
            .map(|(facts, _)| {
                drop(facts);
            });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (result, budget.peak_storage(), budget.failed_storage())
    };
    Measurement {
        result,
        work: work.work(),
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn induction_facts_exact_and_short_work_preserve_typed_denial_and_final_charge_prefix() {
    for (module, pending_suffix) in [
        (
            fixture(ScalarType::U32, None, 1, false),
            Some(366 + 2 * (4 + 3)),
        ),
        (fixture(ScalarType::U8, Some((2, 12)), 3, true), None),
    ] {
        with_loops(module, |loops, budget| {
            let floor = budget.storage();
            let measured = measure(loops, floor, LIMIT, LIMIT);
            assert!(measured.result.is_ok());
            let exact = measure(loops, floor, measured.work, measured.peak);
            assert!(exact.result.is_ok());
            assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
            assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
            let short = measure(loops, floor, measured.work - 1, measured.peak);
            let Err(Error::Resource(Resource::Work(error))) = short.result else {
                panic!("final independent row-replay work refusal: {short:?}")
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (measured.work, measured.work - 1)
            );
            assert_eq!(short.work, measured.work - 3);
            assert_eq!(
                (short.failed_work, short.failed_storage),
                (Some(measured.work), None)
            );
            // The fixed fixtures have different live backing. The dynamic
            // fixture retains the checker pending-table peak; its unchanged
            // 366-unit suffix gains two exact definition(4)/value(3) queries.
            // The literal fixture instead peaks in the first Sparse engine.
            let reference = sparse_prefix_reference(loops, floor, measured.peak - 1);
            let storage = measure(loops, floor, LIMIT, measured.peak - 1);
            let Err(Error::Resource(Resource::Storage(error))) = storage.result else {
                panic!("exact fixture-specific Storage refusal: {storage:?}")
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (measured.peak, measured.peak - 1)
            );
            assert_eq!(loops.inventory().blocks().len(), 4);
            if let Some(remaining_work) = pending_suffix {
                assert!(reference.result.is_ok());
                assert!(reference.peak < measured.peak);
                assert_eq!(storage.work, measured.work - remaining_work);
                assert_eq!(
                    storage.peak,
                    reference
                        .peak
                        .max(measured.peak - 4 * size_of::<(usize, usize)>())
                );
                assert_eq!(
                    (reference.failed_work, reference.failed_storage),
                    (None, None)
                );
            } else {
                // Public component reference, never a private layout mirror or
                // a whole-factory self-oracle. Any other phase fails this test.
                let Err(Error::Resource(Resource::Storage(reference_error))) = reference.result
                else {
                    panic!(
                        "first Sparse construction must reach exact Storage refusal: {reference:?}"
                    )
                };
                assert_eq!(error, reference_error);
                assert_eq!(storage.work, reference.work);
                assert_eq!(storage.peak, reference.peak);
                assert_eq!(
                    (reference.failed_work, reference.failed_storage),
                    (None, Some(measured.peak))
                );
            }
            assert_eq!(
                (storage.failed_work, storage.failed_storage),
                (None, Some(measured.peak))
            );
        });
    }
}

fn sparse_prefix_reference(loops: &Loops<'_, '_>, floor: usize, limit: usize) -> Measurement {
    let mut work = Work::new(LIMIT);
    let (result, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = resources::scoped(&mut budget, |meter: &mut Meter<'_, '_>| {
            let limits = Limits::default();
            meter.derive(|b| Ok(loops.replay(loops.inventory(), limits, b)?))?;
            meter.reserve(size_of::<CanonicalKirInductionFactsV1<'_, '_, '_>>())?;
            let (rows, _) = meter.table::<Row>(loops.recurrences.len())?;
            meter.work(3)?;
            let i = loops.inventory();
            let limits = SparseLimits {
                functions: limits.functions,
                definitions: limits.definitions,
                uses: i.uses().len(),
                blocks: limits.blocks,
                operations: limits.operations,
                edges: limits.edges,
                worklist: i
                    .blocks()
                    .len()
                    .checked_add(i.operations().len())
                    .ok_or(Resource::Arithmetic)?,
            };
            let result = meter.derive(|budget| Ok(Sparse::derive(i, limits, budget)?));
            // The reference deliberately stops at this public component, before
            // producer row construction or independent induction checking.
            drop(rows);
            result.map(|(sparse, _)| drop(sparse))
        });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (result, budget.peak_storage(), budget.failed_storage())
    };
    Measurement {
        result,
        work: work.work(),
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
#[test]
fn induction_report_replay_rejects_missing_receipt_foreign_ledger_and_changed_limits() {
    with_loops(fixture(ScalarType::U32, None, 1, false), |loops, budget| {
        let floor = budget.storage();
        let (facts, receipt) =
            CanonicalKirInductionFactsV1::derive(loops, Limits::default(), budget).unwrap();
        assert!(matches!(
            facts.replay(loops, Limits::default(), budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let mut work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut work, LIMIT);
        foreign.reserve_storage(budget.storage()).unwrap();
        assert!(matches!(
            facts.replay(loops, Limits::default(), &mut foreign),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(foreign.storage(), budget.storage());
        let limits = Limits {
            blocks: 3,
            ..Limits::default()
        };
        assert_eq!(
            facts.replay(loops, limits, budget),
            Err(Error::LimitsMismatch)
        );
        assert!(matches!(
            CanonicalKirInductionFactsV1::derive(loops, limits, budget),
            Err(Error::Loops(LoopError::InputLimit {
                kind: "blocks",
                actual: 4,
                limit: 3
            }))
        ));
        facts.replay(loops, Limits::default(), budget).unwrap();
        drop(facts);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
#[test]
fn induction_report_replay_binds_each_exact_limit_even_when_changed_limits_admit_input() {
    with_loops(fixture(ScalarType::U32, None, 1, false), |loops, budget| {
        let floor = budget.storage();
        let (facts, receipt) =
            CanonicalKirInductionFactsV1::derive(loops, Limits::default(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let retained = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
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
                loops.replay(loops.inventory(), limits, budget).unwrap();
                let accepted = budget.work();
                let peak = budget.peak_storage();
                assert_eq!(
                    facts.replay(loops, limits, budget),
                    Err(Error::LimitsMismatch)
                );
                assert_eq!(budget.work(), accepted + 9);
                assert_eq!((budget.storage(), budget.peak_storage()), (retained, peak));
                assert!(budget.work_ledger_identity_v1() == ledger);
            }
        }
        facts.replay(loops, Limits::default(), budget).unwrap();
        drop(facts);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
#[test]
fn induction_scope_drops_real_partial_report_on_error_and_panic_with_live_sibling() {
    use std::{cell::Cell, rc::Rc};
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for panic in [false, true] {
        with_loops(fixture(ScalarType::U32, None, 1, false), |loops, budget| {
            let sibling = [0x24u8; 17];
            budget.reserve_storage(sibling.len()).unwrap();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let accepted = budget.work();
            let dropped = Rc::new(Cell::new(false));
            let result: Result<()> = resources::scoped(budget, |meter| {
                let (facts, receipt) = meter.derive(|budget| {
                    CanonicalKirInductionFactsV1::derive(loops, Limits::default(), budget)
                })?;
                meter.reserve(receipt.retained_storage())?;
                let _live = (facts, Dropped(dropped.clone()));
                if panic {
                    std::panic::panic_any(17u8);
                }
                Err(Error::ReplayMismatch)
            });
            assert!(matches!(
                (panic, result),
                (true, Err(Error::Panicked)) | (false, Err(Error::ReplayMismatch))
            ));
            assert!(dropped.get());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > accepted);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x24; 17]);
            budget.release_storage(sibling.len()).unwrap();
        });
    }
}

#[test]
fn induction_success_preserves_seeded_first_denial_history_and_accepted_work() {
    with_loops(fixture(ScalarType::U64, None, 1, true), |loops, parent| {
        let floor = parent.storage();
        let baseline = measure(loops, floor, LIMIT, LIMIT);
        assert!(baseline.result.is_ok());
        let mut work = Work::new(LIMIT);
        {
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(floor).unwrap();
            assert!(matches!(
                budget.charge_work(LIMIT + 1),
                Err(Resource::Work(_))
            ));
            assert!(matches!(
                budget.reserve_storage(LIMIT + 1),
                Err(Resource::Storage(_))
            ));
            budget.charge_work(17).unwrap();
            let (facts, receipt) =
                CanonicalKirInductionFactsV1::derive(loops, Limits::default(), &mut budget)
                    .unwrap();
            assert_eq!(budget.work(), baseline.work + 17);
            assert_eq!(budget.peak_storage(), baseline.peak);
            assert_eq!(budget.failed_storage(), Some(floor + LIMIT + 1));
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            drop(facts);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
        assert_eq!(work.failed_work(), Some(LIMIT + 1));
    });
}
