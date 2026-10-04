use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionCheckedOutputAdmissionErrorPolicy6V1 as Policy6Error,
    ProductionCommutativeContinuationErrorV1 as CommutativeError,
    ProductionLicmErrorV1 as SourceLicmError,
    ProductionLoopPreheadersErrorV1 as SourcePreheadersError,
    ProductionPrivateCellPromotionContinuationErrorV1 as PromotionError,
    ProductionRedundantStoreAdmissionErrorV1 as RedundantStoreError,
};
const W: usize = 1_000_000_000;
const S: usize = 1024 * 1024 * 1024;
#[derive(Debug)]
struct Observation {
    result: std::result::Result<(), QueryError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    retained: Option<usize>,
    rows: Option<usize>,
}
fn measure(
    owner: &Licm,
    floor: usize,
    replay: bool,
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let mut work = Work::new(work_limit);
    let mut retained = None;
    let mut rows = None;
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = match query(owner, Limits::default(), &mut budget) {
            Ok((report, storage)) => {
                assert_eq!(report.retained_storage(), storage.retained_storage());
                retained = Some(storage.retained_storage());
                rows = Some(report.rows().len());
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let result = if replay {
                    report.replay(Limits::default(), &mut budget)
                } else {
                    Ok(())
                };
                drop(report);
                budget.release_storage(storage.retained_storage()).unwrap();
                result
            }
            Err(error) => Err(error),
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
        retained,
        rows,
    }
}
#[derive(Debug)]
struct PrefixObservation {
    result: std::result::Result<(), SourceLicmError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure_source_prefix(owner: &Licm, floor: usize, storage_limit: usize) -> PrefixObservation {
    let mut work = Work::new(W);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = match owner {
            Licm::Direct(v) => v.verify_equivalence(&mut budget),
            Licm::Erased(v) => v.verify_equivalence(&mut budget),
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    PrefixObservation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
fn assert_source_prefix_storage(error: &SourceLicmError, mutation: bool, peak: usize) {
    let promotion = if mutation {
        let SourceLicmError::Prefix(preheaders) = error else {
            panic!("exact LICM prefix replay refusal: {error:?}")
        };
        let SourcePreheadersError::Admission(promotion) = preheaders.as_ref() else {
            panic!("exact preheader final-source admission refusal: {preheaders:?}")
        };
        promotion.as_ref()
    } else {
        let SourceLicmError::Admission(promotion) = error else {
            panic!("exact LICM final-source admission refusal: {error:?}")
        };
        promotion.as_ref()
    };
    let PromotionError::Prefix(commutative) = promotion else {
        panic!("exact promoted source-prefix refusal: {promotion:?}")
    };
    let CommutativeError::Prefix(redundant_store) = commutative.as_ref() else {
        panic!("exact commutative source-prefix refusal: {commutative:?}")
    };
    let RedundantStoreError::Prefix(policy6) = redundant_store.as_ref() else {
        panic!("exact redundant-store source-prefix refusal: {redundant_store:?}")
    };
    let Policy6Error::Optimization(fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
        fe2o3_pliron::KirOptimizationMapErrorV12::Resources(Resource::Storage(limit)),
    )) = policy6.as_ref()
    else {
        panic!("exact Policy6 optimization-map Storage refusal: {policy6:?}")
    };
    assert_eq!((limit.actual(), limit.limit()), (peak, peak - 1));
}
#[test]
fn source_loop_induction_exact_and_work_short_preserve_complete_final_comparison_phase() {
    for erased in [false, true] {
        for mutation in [false, true] {
            with_owner(erased, Profile::Gfx942, mutation, |owner, parent| {
                let sibling = [0x93u8; 41];
                let floor = parent.storage() + sibling.len();
                for replay in [false, true] {
                    let measured = measure(owner, floor, replay, W, S);
                    assert!(measured.result.is_ok());
                    let exact = measure(owner, floor, replay, measured.work, measured.peak);
                    assert!(exact.result.is_ok());
                    assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
                    assert_eq!(
                        (exact.retained, exact.rows),
                        (measured.retained, measured.rows)
                    );
                    assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
                    let short = measure(owner, floor, replay, measured.work - 1, measured.peak);
                    let Err(QueryError::Resource(Resource::Work(error))) = short.result else {
                        panic!("exact final source-facts Work refusal: {short:?}")
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
                    assert_eq!(short.peak, measured.peak);
                    assert_eq!(sibling, [0x93; 41]);
                }
            });
        }
    }
}
#[test]
fn source_loop_induction_storage_short_preserves_exact_source_prefix_phase() {
    for erased in [false, true] {
        for mutation in [false, true] {
            with_owner(erased, Profile::Gfx942, mutation, |owner, parent| {
                let floor = parent.storage();
                let derived = measure(owner, floor, false, W, S);
                assert!(derived.result.is_ok());
                assert_eq!((derived.failed_work, derived.failed_storage), (None, None));
                let retained = derived.retained.unwrap();
                let rows = derived.rows.unwrap();
                assert_eq!(rows, if mutation { 2 } else { 0 });
                for replay in [false, true] {
                    let measured = measure(owner, floor, replay, W, S);
                    assert!(measured.result.is_ok());
                    assert_eq!(
                        (measured.failed_work, measured.failed_storage),
                        (None, None)
                    );
                    assert_eq!(
                        (measured.retained, measured.rows),
                        (Some(retained), Some(rows))
                    );
                    // Replay checks owner/output (4), all limits (7), and custody (3)
                    // before the same source replay. Only derivation copies rows.
                    let replay_checks = 4 + 7 + 3;
                    let (prefix_floor, earlier_work, earlier_peak) = if replay {
                        assert_eq!(measured.work, 2 * derived.work + replay_checks - rows);
                        assert_eq!(measured.peak, derived.peak + retained);
                        (floor + retained, derived.work + replay_checks, derived.peak)
                    } else {
                        assert_eq!((measured.work, measured.peak), (derived.work, derived.peak));
                        (floor, 0, floor)
                    };
                    // Measure the public genuine-owner replay separately; do not
                    // infer its accepted prefix from the query under test.
                    let prefix = measure_source_prefix(owner, prefix_floor, S);
                    assert!(prefix.result.is_ok());
                    assert_eq!((prefix.failed_work, prefix.failed_storage), (None, None));
                    assert_eq!(prefix.peak, measured.peak);
                    let prefix_short =
                        measure_source_prefix(owner, prefix_floor, measured.peak - 1);
                    let Err(prefix_error) = &prefix_short.result else {
                        panic!("source-prefix Storage refusal: {prefix_short:?}")
                    };
                    assert_source_prefix_storage(prefix_error, mutation, measured.peak);
                    assert_eq!(
                        (prefix_short.failed_work, prefix_short.failed_storage),
                        (None, Some(measured.peak))
                    );
                    assert!(prefix_short.work < prefix.work);
                    assert!(prefix_short.peak < prefix.peak);
                    let short = measure(owner, floor, replay, measured.work, measured.peak - 1);
                    let Err(QueryError::Prefix(error)) = &short.result else {
                        panic!("exact source-query prefix Storage refusal: {short:?}")
                    };
                    assert_source_prefix_storage(error, mutation, measured.peak);
                    assert_eq!(
                        (short.failed_work, short.failed_storage),
                        (None, Some(measured.peak))
                    );
                    assert_eq!(short.work, earlier_work + prefix_short.work);
                    assert_eq!(short.peak, earlier_peak.max(prefix_short.peak));
                    assert_eq!(
                        (short.retained, short.rows),
                        if replay {
                            (Some(retained), Some(rows))
                        } else {
                            (None, None)
                        }
                    );
                }
            });
        }
    }
}
#[test]
fn source_loop_induction_missing_owner_floor_report_receipt_and_foreign_ledger_refuse() {
    for erased in [false, true] {
        with_owner(erased, Profile::Gfx942, true, |owner, budget| {
            let intrinsic = match owner {
                Licm::Direct(v) => v.retained_input_storage_floor_v1().unwrap(),
                Licm::Erased(v) => v.retained_input_storage_floor_v1().unwrap(),
            };
            let mut work = Work::new(W);
            let mut short = Budget::new(&mut work, S);
            short.reserve_storage(intrinsic - 1).unwrap();
            assert!(matches!(
                query(owner, Limits::default(), &mut short),
                Err(QueryError::Resource(Resource::Accounting))
            ));
            assert_eq!(short.storage(), intrinsic - 1);
            assert_eq!(short.work(), 0);
            let floor = budget.storage();
            let (report, storage) = query(owner, Limits::default(), budget).unwrap();
            assert!(matches!(
                report.replay(Limits::default(), budget),
                Err(QueryError::Resource(Resource::Accounting))
            ));
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let mut other_work = Work::new(W);
            let mut other = Budget::new(&mut other_work, S);
            other.reserve_storage(budget.storage()).unwrap();
            assert!(matches!(
                report.replay(Limits::default(), &mut other),
                Err(QueryError::Resource(Resource::Accounting))
            ));
            assert_eq!(other.storage(), budget.storage());
            budget.release_storage(1).unwrap();
            assert!(matches!(
                report.replay(Limits::default(), budget),
                Err(QueryError::Resource(Resource::Accounting))
            ));
            budget.reserve_storage(1).unwrap();
            report.replay(Limits::default(), budget).unwrap();
            drop(report);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}
#[test]
fn source_loop_induction_success_retains_seeded_denials_work_and_live_sibling() {
    for erased in [false, true] {
        with_owner(erased, Profile::Gfx942, true, |owner, parent| {
            let sibling = [0xa7u8; 43];
            let floor = parent.storage() + sibling.len();
            let baseline = measure(owner, floor, false, W, S);
            assert!(baseline.result.is_ok());
            let mut work = Work::new(W);
            {
                let mut budget = Budget::new(&mut work, S);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(17).unwrap();
                assert!(matches!(budget.charge_work(W), Err(Resource::Work(_))));
                assert!(matches!(
                    budget.reserve_storage(S),
                    Err(Resource::Storage(_))
                ));
                let ledger = budget.work_ledger_identity_v1();
                let (report, storage) = query(owner, Limits::default(), &mut budget).unwrap();
                assert_eq!(budget.work(), baseline.work + 17);
                assert_eq!(budget.peak_storage(), baseline.peak);
                assert_eq!(budget.failed_storage(), Some(floor + S));
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_eq!(report.rows().len(), 2);
                assert!(budget.work_ledger_identity_v1() == ledger);
                drop(report);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(sibling, [0xa7; 43]);
            }
            assert_eq!(work.failed_work(), Some(W + 17));
        });
    }
}
