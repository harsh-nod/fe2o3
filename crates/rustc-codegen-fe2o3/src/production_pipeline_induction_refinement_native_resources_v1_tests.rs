use super::*;

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
    mutation: bool,
    profile: Profile,
    replay: bool,
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let mut observation = None;
    with_prefix(erased, profile, mutation, |prefix, parent| {
        let mut prefix = Some(prefix);
        let native = if replay {
            let (value, receipt) =
                prepare(prefix.take().unwrap(), profile, Limits::default(), parent).unwrap();
            parent.reserve_storage(receipt.retained_storage()).unwrap();
            Some((value, receipt))
        } else {
            None
        };
        let sibling = vec![0x37u8; 37];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(work_limit);
        let (error, accepted, peak, failed_storage, llvm, retained) = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = match &native {
                Some((value, _)) => value.verify_equivalence(&mut budget).map(|()| None),
                None => prepare(
                    prefix.take().unwrap(),
                    profile,
                    Limits::default(),
                    &mut budget,
                )
                .map(Some),
            };
            assert_eq!(budget.storage(), floor);
            let (error, llvm, retained) = match result {
                Ok(Some((value, receipt))) => {
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(value.retained_storage_floor_v1(), budget.storage());
                    let llvm = (value.llvm.len(), value.llvm.capacity());
                    drop(value);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    (None, Some(llvm), Some(receipt.retained_storage()))
                }
                Ok(None) => {
                    let (value, receipt) = native.as_ref().unwrap();
                    (
                        None,
                        Some((value.llvm.len(), value.llvm.capacity())),
                        Some(receipt.retained_storage()),
                    )
                }
                Err(error) => (Some(error), None, None),
            };
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), floor);
            assert_eq!(sibling, [0x37; 37]);
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
        if let Some((value, receipt)) = native {
            drop(value);
            parent.release_storage(receipt.retained_storage()).unwrap();
        }
    });
    observation.unwrap()
}
fn exact_work_cases(replay: bool) {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let measured = measure(
                    erased,
                    mutation,
                    profile,
                    replay,
                    1_000_000_000,
                    1024 * 1024 * 1024,
                );
                assert!(measured.error.is_none());
                assert_eq!(
                    (measured.failed_work, measured.failed_storage),
                    (None, None)
                );
                let exact = measure(
                    erased,
                    mutation,
                    profile,
                    replay,
                    measured.work,
                    measured.peak,
                );
                assert!(exact.error.is_none());
                assert_eq!(
                    (exact.work, exact.peak, exact.llvm, exact.retained),
                    (
                        measured.work,
                        measured.peak,
                        measured.llvm,
                        measured.retained
                    )
                );
                assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
                let short = measure(
                    erased,
                    mutation,
                    profile,
                    replay,
                    measured.work - 1,
                    measured.peak,
                );
                match short.error {
                    Some(ProductionPipelineError::InductionRefinementNativeStage(
                        InductionRefinementNativeStageErrorV1::Resource(Resource::Work(error)),
                    )) => {
                        assert_eq!(
                            (error.actual(), error.limit()),
                            (measured.work, measured.work - 1)
                        );
                    }
                    other => panic!("exact final refined LLVM comparison Work refusal: {other:?}"),
                }
                let comparison = measured
                    .llvm
                    .unwrap()
                    .0
                    .checked_mul(2)
                    .unwrap()
                    .checked_add(1)
                    .unwrap();
                assert_eq!(short.work, measured.work - comparison);
                assert_eq!(short.peak, measured.peak);
                assert_eq!(
                    (short.failed_work, short.failed_storage),
                    (Some(measured.work), None)
                );
            }
        }
    }
}
#[test]
fn induction_refinement_native_factory_exact_and_short_work_keep_final_comparison_phase() {
    exact_work_cases(false);
}
#[test]
fn induction_refinement_native_replay_exact_and_short_work_keep_final_comparison_phase() {
    exact_work_cases(true);
}

// Stop before the native emitter: pay the real owner, then independently replay
// its public source continuation and the retained original P7 history.
fn replay_prefix_reference(
    erased: bool,
    mutation: bool,
    profile: Profile,
) -> (usize, usize, usize) {
    let mut observation = None;
    with_prefix(erased, profile, mutation, |prefix, parent| {
        let source_floor = parent.storage();
        let source_ledger = parent.work_ledger_identity_v1();
        let (value, receipt) = prepare(prefix, profile, Limits::default(), parent).unwrap();
        parent.reserve_storage(receipt.retained_storage()).unwrap();
        let sibling = vec![0x37u8; 37];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(1_000_000_000);
        {
            let mut budget = Budget::new(&mut work, 1024 * 1024 * 1024);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            match &value.owner {
                Refined::Direct(owner) => owner.verify_equivalence(&mut budget).unwrap(),
                Refined::Erased(owner) => owner.verify_equivalence(&mut budget).unwrap(),
            }
            assert_eq!(budget.storage(), floor);
            let history_header = fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1;
            budget.reserve_storage(history_header).unwrap();
            value
                .prefix_execution
                .check_history_v1(value.owner.history(), &mut budget)
                .unwrap();
            budget.release_storage(history_header).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x37; 37]);
            observation = Some((budget.work(), budget.peak_storage(), floor));
        }
        assert_eq!(work.failed_work(), None);
        drop(value);
        parent.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(parent.storage(), source_floor);
        assert!(parent.work_ledger_identity_v1() == source_ledger);
    });
    observation.unwrap()
}

fn storage_cases(replay: bool) {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let measured = measure(
                    erased,
                    mutation,
                    profile,
                    replay,
                    1_000_000_000,
                    1024 * 1024 * 1024,
                );
                assert!(measured.error.is_none());
                let short = measure(
                    erased,
                    mutation,
                    profile,
                    replay,
                    measured.work,
                    measured.peak - 1,
                );
                match short.error {
                    Some(ProductionPipelineError::PrivateCellNativeStage(
                        PrivateCellNativeStageErrorV1::Resource(Resource::Storage(error)),
                    )) => assert_eq!(
                        (error.actual(), error.limit()),
                        (measured.peak, measured.peak - 1)
                    ),
                    other => panic!("exact final native scratch Storage refusal: {other:?}"),
                }
                let (llvm_len, llvm_capacity) = measured.llvm.unwrap();
                let comparison = llvm_len.checked_mul(2).unwrap().checked_add(1).unwrap();
                assert_eq!(short.work, measured.work.checked_sub(comparison).unwrap());
                if replay {
                    let (prefix_work, prefix_peak, floor) =
                        replay_prefix_reference(erased, mutation, profile);
                    let native_entry_work = prefix_work.checked_add(3).unwrap();
                    assert_eq!(short.work, native_entry_work);
                    assert_eq!(
                        measured.work,
                        native_entry_work.checked_add(comparison).unwrap()
                    );
                    assert_eq!(short.peak, prefix_peak);
                    let scratch = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
                        .checked_mul(3)
                        .and_then(|bytes| bytes.checked_add(size_of::<String>() * 2))
                        .unwrap();
                    assert_eq!(measured.peak, floor.checked_add(scratch).unwrap());
                } else {
                    let owner_header = if erased {
                        size_of::<ErasedRefined>()
                    } else {
                        size_of::<DirectRefined>()
                    };
                    // The earlier identical native scratch request has no retained
                    // LLVM backing or completed candidate headers alongside it.
                    let candidate_header = size_of::<PreparedInductionRefinementNativeOutputV1>()
                        .checked_sub(owner_header)
                        .and_then(|bytes| bytes.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
                        .unwrap();
                    let between_native_requests =
                        candidate_header.checked_add(llvm_capacity).unwrap();
                    assert_eq!(
                        short.peak,
                        measured.peak.checked_sub(between_native_requests).unwrap()
                    );
                }
                assert_eq!(
                    (short.failed_work, short.failed_storage),
                    (None, Some(measured.peak))
                );
                assert!(short.work < measured.work && short.peak < measured.peak);
            }
        }
    }
}
#[test]
fn induction_refinement_native_factory_storage_short_observes_first_denial() {
    storage_cases(false);
}
#[test]
fn induction_refinement_native_replay_storage_short_observes_first_denial() {
    storage_cases(true);
}

#[test]
fn induction_refinement_native_success_keeps_prior_denials_live_receipts_and_ledger() {
    for erased in [false, true] {
        for mutation in [false, true] {
            with_prefix(erased, Profile::Gfx942, mutation, |prefix, parent| {
                const W: usize = 1_000_000_000;
                const S: usize = 1024 * 1024 * 1024;
                let sibling = vec![0x47u8; 47];
                let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
                let mut work = Work::new(W);
                {
                    let mut budget = Budget::new(&mut work, S);
                    budget.reserve_storage(floor).unwrap();
                    budget.charge_work(17).unwrap();
                    match budget.charge_work(W) {
                        Err(Resource::Work(e)) => assert_eq!((e.actual(), e.limit()), (W + 17, W)),
                        other => panic!("seeded Work refusal: {other:?}"),
                    }
                    match budget.reserve_storage(S) {
                        Err(Resource::Storage(e)) => {
                            assert_eq!((e.actual(), e.limit()), (S + floor, S))
                        }
                        other => panic!("seeded Storage refusal: {other:?}"),
                    }
                    let ledger = budget.work_ledger_identity_v1();
                    let (value, receipt) =
                        prepare(prefix, Profile::Gfx942, Limits::default(), &mut budget).unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.failed_storage(), Some(S + floor));
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(value.retained_storage_floor_v1(), budget.storage());
                    value.verify_equivalence(&mut budget).unwrap();
                    assert!(value.llvm.capacity() >= value.llvm.len());
                    assert!(
                        receipt.retained_storage() >= value.llvm.capacity() + size_of::<String>()
                    );
                    drop(value);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.failed_storage(), Some(S + floor));
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert_eq!(sibling, [0x47; 47]);
                }
                assert_eq!(work.failed_work(), Some(W + 17));
            });
        }
    }
}
#[test]
fn induction_refinement_native_scope_refuses_foreign_ledger_after_real_preparation() {
    with_prefix(false, Profile::Gfx942, true, |prefix, parent| {
        let sibling = vec![0x62u8; 41];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(1_000_000_000);
        let mut other_work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1024 * 1024 * 1024);
        let mut replacement = Budget::new(&mut other_work, 1024 * 1024 * 1024);
        budget.reserve_storage(floor).unwrap();
        replacement.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result<()> = scoped(floor, &mut budget, |budget| {
            let (value, receipt) = prepare(prefix, Profile::Gfx942, Limits::default(), budget)?;
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            std::mem::swap(budget, &mut replacement);
            drop(value);
            Ok(())
        });
        assert!(matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                P7Error::Resource(Resource::Accounting)
            ))
        ));
        assert!(budget.work_ledger_identity_v1() != ledger);
        assert_eq!((budget.storage(), budget.work()), (floor, 0));
        std::mem::swap(&mut budget, &mut replacement);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() > 17 && budget.storage() > floor);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(sibling, [0x62; 41]);
        assert_eq!(budget.storage(), floor);
    });
}
