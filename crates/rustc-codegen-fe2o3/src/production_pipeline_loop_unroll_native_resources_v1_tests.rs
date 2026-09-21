use super::*;
use std::mem::size_of_val;

struct Observation {
    error: Option<ProductionPipelineError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    llvm: Option<(usize, usize)>,
    retained: Option<usize>,
}
fn measure(
    erased: bool,
    bound: Option<u64>,
    profile: Profile,
    replay: bool,
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let mut observation = None;
    with_prefix(erased, profile, bound, |prefix, parent| {
        let mut prefix = Some(prefix);
        let native = if replay {
            let (v, r) = prepare(
                prefix.take().unwrap(),
                profile,
                Limits::default(),
                ForwardingLimits::default(),
                UnrollLimits::default(),
                parent,
            )
            .unwrap();
            parent.reserve_storage(r.retained_storage()).unwrap();
            Some((v, r))
        } else {
            None
        };
        let sibling = vec![0x57u8; 37];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(work_limit);
        let (error, accepted, peak, failed_storage, llvm, retained) = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = match &native {
                Some((v, _)) => v.verify_equivalence(&mut budget).map(|()| None),
                None => prepare(
                    prefix.take().unwrap(),
                    profile,
                    Limits::default(),
                    ForwardingLimits::default(),
                    UnrollLimits::default(),
                    &mut budget,
                )
                .map(Some),
            };
            assert_eq!(budget.storage(), floor);
            let (error, llvm, retained) = match result {
                Ok(Some((v, r))) => {
                    budget.reserve_storage(r.retained_storage()).unwrap();
                    assert_eq!(v.retained_storage_floor_v1(), budget.storage());
                    let llvm = (v.llvm.len(), v.llvm.capacity());
                    drop(v);
                    budget.release_storage(r.retained_storage()).unwrap();
                    (None, Some(llvm), Some(r.retained_storage()))
                }
                Ok(None) => {
                    let (v, r) = native.as_ref().unwrap();
                    (
                        None,
                        Some((v.llvm.len(), v.llvm.capacity())),
                        Some(r.retained_storage()),
                    )
                }
                Err(e) => (Some(e), None, None),
            };
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), floor);
            assert_eq!(sibling, [0x57; 37]);
            (
                error,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
                llvm,
                retained,
            )
        };
        observation = Some(Observation {
            error,
            work: accepted,
            peak,
            failed_work: work.failed_work(),
            failed_storage,
            llvm,
            retained,
        });
        if let Some((v, r)) = native {
            drop(v);
            parent.release_storage(r.retained_storage()).unwrap();
        }
    });
    observation.unwrap()
}
fn exact_work(replay: bool) {
    for erased in [false, true] {
        for profile in PROFILES {
            for bound in [None, Some(3)] {
                let full = measure(erased, bound, profile, replay, WORK, STORAGE);
                assert!(full.error.is_none(), "{:?}", full.error);
                assert_eq!((full.failed_work, full.failed_storage), (None, None));
                let exact = measure(erased, bound, profile, replay, full.work, full.peak);
                assert!(exact.error.is_none(), "{:?}", exact.error);
                assert_eq!(
                    (exact.work, exact.peak, exact.llvm, exact.retained),
                    (full.work, full.peak, full.llvm, full.retained)
                );
                assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
                let short = measure(erased, bound, profile, replay, full.work - 1, full.peak);
                match short.error {
                    Some(ProductionPipelineError::InductionRefinementNativeStage(
                        InductionRefinementNativeStageErrorV1::ForwardingComposition(
                            RefinedForwardingNativeStageErrorV1::BoundedUnroll(
                                LoopUnrollNativeStageErrorV1::Resource(Resource::Work(e)),
                            ),
                        ),
                    )) => assert_eq!((e.actual(), e.limit()), (full.work, full.work - 1)),
                    other => panic!("exact terminal U work refusal: {other:?}"),
                }
                assert_eq!((short.work, short.peak), (full.work - 1, full.peak));
                assert_eq!(
                    (short.failed_work, short.failed_storage),
                    (Some(full.work), None)
                );
            }
        }
    }
}
#[test]
fn loop_unroll_native_factory_exact_budget_and_final_work_denial() {
    exact_work(false);
}
#[test]
fn loop_unroll_native_replay_exact_budget_and_final_work_denial() {
    exact_work(true);
}

#[test]
fn loop_unroll_native_factory_first_header_denial_is_typed_and_preserves_sibling() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prefix(erased, profile, Some(3), |prefix, parent| {
                let sibling = vec![0x33u8; 19];
                let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
                let header = prepared_header(erased).unwrap();
                assert!(header > 0);
                let mut work = Work::new(WORK);
                {
                    let mut budget = Budget::new(&mut work, floor + header - 1);
                    budget.reserve_storage(floor).unwrap();
                    budget.charge_work(17).unwrap();
                    let ledger = budget.work_ledger_identity_v1();
                    let failure = prepare(
                        prefix,
                        profile,
                        Limits::default(),
                        ForwardingLimits::default(),
                        UnrollLimits::default(),
                        &mut budget,
                    )
                    .err()
                    .unwrap();
                    match failure {
                        ProductionPipelineError::InductionRefinementNativeStage(
                            InductionRefinementNativeStageErrorV1::ForwardingComposition(
                                RefinedForwardingNativeStageErrorV1::BoundedUnroll(
                                    LoopUnrollNativeStageErrorV1::Resource(Resource::Storage(e)),
                                ),
                            ),
                        ) => assert_eq!(
                            (e.actual(), e.limit()),
                            (floor + header, floor + header - 1)
                        ),
                        other => panic!("first U header refusal: {other:?}"),
                    }
                    assert_eq!(
                        (budget.storage(), budget.peak_storage(), budget.work()),
                        (floor, floor, 20)
                    );
                    assert_eq!(budget.failed_storage(), Some(floor + header));
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert_eq!(sibling, [0x33; 19]);
                }
                assert_eq!(work.failed_work(), None);
            });
        }
    }
}
#[test]
fn loop_unroll_native_full_storage_short_observations_require_strict_successor() {
    let mut observations = 0;
    for erased in [false, true] {
        for profile in PROFILES {
            for bound in [None, Some(3)] {
                for replay in [false, true] {
                    let full = measure(erased, bound, profile, replay, WORK, STORAGE);
                    assert!(full.error.is_none(), "{:?}", full.error);
                    let short = measure(erased, bound, profile, replay, full.work, full.peak - 1);
                    eprintln!(
                        "U_NATIVE_STORAGE_DIAGNOSTIC erased={erased} profile={profile:?} bound={bound:?} replay={replay} W={} P={} accepted={} prior_peak={} failed_storage={:?} error={:?}",
                        full.work,
                        full.peak,
                        short.work,
                        short.peak,
                        short.failed_storage,
                        short.error
                    );
                    assert!(short.error.is_some());
                    assert!(short.failed_storage.is_some());
                    assert_eq!(short.failed_work, None);
                    observations += 1;
                }
            }
        }
    }
    assert_eq!(observations, 16);
}
#[test]
fn loop_unroll_native_replay_requires_complete_live_floor() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |owner, parent| {
                let floor = parent.storage();
                let mut work = Work::new(WORK);
                {
                    let mut budget = Budget::new(&mut work, STORAGE);
                    budget
                        .reserve_storage(owner.retained_storage_floor_v1() - 1)
                        .unwrap();
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    assert!(matches!(owner.verify_equivalence(&mut budget), Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)))));
                    assert_eq!(
                        (budget.work(), budget.storage(), budget.peak_storage()),
                        before
                    );
                    assert_eq!(budget.failed_storage(), None);
                }
                assert_eq!(work.failed_work(), None);
                owner.verify_equivalence(parent).unwrap();
                assert_eq!(parent.storage(), floor);
            });
        }
    }
}

#[test]
fn loop_unroll_native_scoped_unwind_drops_new_storage_and_preserves_denial_history() {
    use std::{cell::Cell, rc::Rc};
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let sibling = vec![0x29u8; 29];
    let floor = size_of_val(&sibling) + sibling.capacity();
    let cap = floor + prepared_header(false).unwrap() + 4096;
    let dropped = Rc::new(Cell::new(false));
    let mut work = Work::new(64);
    {
        let mut budget = Budget::new(&mut work, cap);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.reserve_storage(cap + 1).is_err());
        assert!(budget.charge_work(65).is_err());
        let failed = budget.failed_storage();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result<()> = scoped(floor, &mut budget, |budget| {
            budget
                .reserve_storage(prepared_header(false).unwrap())
                .map_err(resource)?;
            let _drop = Dropped(dropped.clone());
            budget.charge_work(2).map_err(resource)?;
            panic!("new U scope cleanup control");
        });
        assert!(matches!(result, Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
            crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1::Panicked))));
        assert!(dropped.get());
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), failed);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x29; 29]);
    }
    assert_eq!(work.failed_work(), Some(65));
}
