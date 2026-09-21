use super::*;
use crate::production_pipeline::private_cell_native_v1::PrivateCellNativeStageErrorV1;
use std::mem::size_of_val;

struct Observation {
    error: Option<ProductionPipelineError>,
    floor: usize,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    llvm: Option<(usize, usize)>,
    retained: Option<usize>,
}
fn assert_resource_selection(owner: &Unrolled, bound: Option<u64>) {
    assert_eq!(
        tail(owner)
            .origins()
            .selection
            .map(|selection| selection.iterations),
        bound.map(|value| u8::try_from(value).unwrap())
    );
    if bound.is_some() {
        assert_ne!(
            owner.source().output().canonical().identity(),
            owner.output().canonical().identity()
        );
    }
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
                    assert_resource_selection(&v.owner, bound);
                    let llvm = (v.llvm.len(), v.llvm.capacity());
                    drop(v);
                    budget.release_storage(r.retained_storage()).unwrap();
                    (None, Some(llvm), Some(r.retained_storage()))
                }
                Ok(None) => {
                    let (v, r) = native.as_ref().unwrap();
                    assert_resource_selection(&v.owner, bound);
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
            floor,
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

struct FactoryPrefix {
    floor: usize,
    complete_floor: usize,
    work: usize,
    peak: usize,
    llvm: (usize, usize),
    retained: usize,
}
struct ReplayPrefix {
    floor: usize,
    work: usize,
    peak: usize,
    llvm: (usize, usize),
    retained: usize,
}
fn native_scratch() -> usize {
    dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(size_of::<String>() * 2))
        .unwrap()
}

// Use the real F prefix and public source-U continuation, without the native-U
// factory or its completed-owner replay. The first emitter is measured separately.
fn factory_prefix_reference(erased: bool, bound: Option<u64>, profile: Profile) -> FactoryPrefix {
    let mut observation = None;
    with_prefix(erased, profile, bound, |prefix, parent| {
        let parent_floor = parent.storage();
        let parent_ledger = parent.work_ledger_identity_v1();
        let sibling = vec![0x57u8; 37];
        let floor = parent_floor
            .checked_add(size_of_val(&sibling))
            .and_then(|bytes| bytes.checked_add(sibling.capacity()))
            .unwrap();
        let mut work = Work::new(WORK);
        {
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let active_u = if erased {
                size_of::<ErasedU>()
            } else {
                size_of::<DirectU>()
            };
            let header = size_of::<PreparedLoopUnrollNativeOutputV1>()
                .checked_sub(active_u)
                .and_then(|bytes| bytes.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
                .and_then(|bytes| bytes.checked_sub(size_of::<String>()))
                .unwrap();
            let active_f = if erased {
                size_of::<ErasedComposed>()
            } else {
                size_of::<DirectComposed>()
            };
            let transient = size_of::<Composed>().checked_sub(active_f).unwrap();
            budget.charge_work(3).unwrap();
            budget.reserve_storage(header).unwrap();
            budget.reserve_storage(transient).unwrap();
            let (source, execution, history, promoted, preheaders, licm, refined, forwarded) =
                prepare_refined_forwarding_source_prefix_v1(
                    prefix,
                    Limits::default(),
                    ForwardingLimits::default(),
                    &mut budget,
                )
                .unwrap();
            let (owner, receipt) = match source {
                Composed::Direct(source) => {
                    let (owner, receipt) = source
                        .continue_bounded_loop_unroll_v1(UnrollLimits::default(), &mut budget)
                        .unwrap();
                    (Unrolled::Direct(owner), receipt)
                }
                Composed::Erased(source) => {
                    let (owner, receipt) = source
                        .continue_bounded_loop_unroll_v1(UnrollLimits::default(), &mut budget)
                        .unwrap();
                    (Unrolled::Erased(owner), receipt)
                }
            };
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            budget.release_storage(transient).unwrap();
            assert_resource_selection(&owner, bound);
            let source_added = [
                header,
                history,
                execution.retained_storage(),
                promoted,
                preheaders,
                licm,
                refined,
                forwarded,
                receipt.retained_storage(),
            ]
            .into_iter()
            .try_fold(0usize, usize::checked_add)
            .unwrap();
            let source_floor = floor.checked_add(source_added).unwrap();
            assert_eq!(budget.storage(), source_floor);
            let source_work = budget.work();
            let source_peak = budget.peak_storage();
            let (llvm, native_retained) =
                lower_native(owner.output(), profile, &mut budget).unwrap();
            assert_eq!(budget.storage(), source_floor);
            assert_eq!(budget.work(), source_work.checked_add(3).unwrap());
            assert_eq!(
                native_retained,
                size_of::<String>().checked_add(llvm.capacity()).unwrap()
            );
            let first_native_peak = source_floor.checked_add(native_scratch()).unwrap();
            assert!(source_peak <= first_native_peak);
            assert_eq!(budget.peak_storage(), first_native_peak);
            budget.reserve_storage(native_retained).unwrap();
            let retained = source_added.checked_add(native_retained).unwrap();
            let complete_floor = floor.checked_add(retained).unwrap();
            assert_eq!(budget.storage(), complete_floor);
            assert!(complete_floor <= first_native_peak);
            observation = Some(FactoryPrefix {
                floor,
                complete_floor,
                work: budget.work(),
                peak: budget.peak_storage(),
                llvm: (llvm.len(), llvm.capacity()),
                retained,
            });
            drop(llvm);
            drop(owner);
            drop(execution);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x57; 37]);
        }
        assert_eq!(work.failed_work(), None);
        assert_eq!(parent.storage(), parent_floor);
        assert!(parent.work_ledger_identity_v1() == parent_ledger);
    });
    observation.unwrap()
}

// Prepare the genuine complete native owner outside the measured ledger, then
// stop its independent public source/P7 replay before the next LLVM emission.
fn replay_prefix_reference(erased: bool, bound: Option<u64>, profile: Profile) -> ReplayPrefix {
    let mut observation = None;
    with_prefix(erased, profile, bound, |prefix, parent| {
        let parent_floor = parent.storage();
        let parent_ledger = parent.work_ledger_identity_v1();
        let (native, receipt) = prepare(
            prefix,
            profile,
            Limits::default(),
            ForwardingLimits::default(),
            UnrollLimits::default(),
            parent,
        )
        .unwrap();
        parent.reserve_storage(receipt.retained_storage()).unwrap();
        assert_resource_selection(&native.owner, bound);
        let sibling = vec![0x57u8; 37];
        let floor = parent
            .storage()
            .checked_add(size_of_val(&sibling))
            .and_then(|bytes| bytes.checked_add(sibling.capacity()))
            .unwrap();
        let mut work = Work::new(WORK);
        {
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            match &native.owner {
                Unrolled::Direct(owner) => owner.verify_equivalence(&mut budget).unwrap(),
                Unrolled::Erased(owner) => owner.verify_equivalence(&mut budget).unwrap(),
            }
            assert_eq!(budget.storage(), floor);
            let history_header = fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1
                .checked_add(size_of::<FinalSource<'_>>())
                .unwrap();
            budget.reserve_storage(history_header).unwrap();
            native
                .prefix_execution
                .check_history_v1(native.owner.source().history(), &mut budget)
                .unwrap();
            budget.release_storage(history_header).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x57; 37]);
            observation = Some(ReplayPrefix {
                floor,
                work: budget.work(),
                peak: budget.peak_storage(),
                llvm: (native.llvm.len(), native.llvm.capacity()),
                retained: receipt.retained_storage(),
            });
        }
        assert_eq!(work.failed_work(), None);
        drop(native);
        parent.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(parent.storage(), parent_floor);
        assert!(parent.work_ledger_identity_v1() == parent_ledger);
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
                let factory = factory_prefix_reference(erased, bound, profile);
                let retained = replay_prefix_reference(erased, bound, profile);
                assert_eq!(factory.complete_floor, retained.floor);
                assert_eq!(
                    (factory.llvm, factory.retained),
                    (retained.llvm, retained.retained)
                );
                let attempted = retained.floor.checked_add(native_scratch()).unwrap();
                let comparison = retained
                    .llvm
                    .0
                    .checked_mul(2)
                    .and_then(|bytes| bytes.checked_add(1))
                    .unwrap();
                for replay in [false, true] {
                    let full = measure(erased, bound, profile, replay, WORK, STORAGE);
                    assert!(full.error.is_none(), "{:?}", full.error);
                    assert_eq!((full.failed_work, full.failed_storage), (None, None));
                    assert_eq!(
                        (full.llvm, full.retained),
                        (Some(retained.llvm), Some(retained.retained))
                    );
                    let (floor, accepted, prior_peak, terminal) = if replay {
                        (
                            retained.floor,
                            retained.work.checked_add(3).unwrap(),
                            retained.peak,
                            1,
                        )
                    } else {
                        let accepted = factory
                            .work
                            .checked_add(retained.work.checked_sub(17).unwrap())
                            .and_then(|work| work.checked_add(3))
                            .unwrap();
                        (factory.floor, accepted, factory.peak.max(retained.peak), 2)
                    };
                    assert_eq!(full.floor, floor);
                    assert_eq!(full.peak, attempted);
                    assert_eq!(
                        full.work,
                        accepted
                            .checked_add(comparison)
                            .and_then(|work| work.checked_add(terminal))
                            .unwrap()
                    );
                    assert!(prior_peak < attempted);
                    let short = measure(erased, bound, profile, replay, full.work, full.peak - 1);
                    match short.error {
                        Some(ProductionPipelineError::PrivateCellNativeStage(
                            PrivateCellNativeStageErrorV1::Resource(Resource::Storage(error)),
                        )) => {
                            assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1))
                        }
                        other => panic!("exact second U emitter scratch refusal: {other:?}"),
                    }
                    assert_eq!(
                        (short.floor, short.work, short.peak),
                        (floor, accepted, prior_peak)
                    );
                    assert_eq!(
                        (short.failed_work, short.failed_storage),
                        (None, Some(attempted))
                    );
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
