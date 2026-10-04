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
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let mut observation = None;
    with_prefix(erased, profile, mutation, |prefix, parent| {
        let sibling = vec![0x37_u8; 37];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(work_limit);
        let (error, accepted, peak, failed_storage, llvm, retained) = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = prepare(prefix, profile, &mut budget);
            assert_eq!(budget.storage(), floor);
            let (error, llvm, retained) = match result {
                Ok((value, receipt)) => {
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(value.retained_storage_floor_v1(), budget.storage());
                    let llvm = (value.llvm.len(), value.llvm.capacity());
                    drop(value);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    (None, Some(llvm), Some(receipt.retained_storage()))
                }
                Err(error) => (Some(error), None, None),
            };
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), floor);
            assert!(sibling.iter().all(|b| *b == 0x37));
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
    });
    observation.unwrap()
}

#[test]
fn licm_native_exact_and_work_short_have_the_exact_final_comparison_denial() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let measured =
                    measure(erased, mutation, profile, 1_000_000_000, 1024 * 1024 * 1024);
                assert!(measured.error.is_none());
                assert_eq!(
                    (measured.failed_work, measured.failed_storage),
                    (None, None)
                );
                let exact = measure(erased, mutation, profile, measured.work, measured.peak);
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
                let short = measure(erased, mutation, profile, measured.work - 1, measured.peak);
                match short.error {
                    Some(ProductionPipelineError::LicmNativeStage(
                        LicmNativeStageErrorV1::Resource(Resource::Work(error)),
                    )) => {
                        assert_eq!(
                            (error.actual(), error.limit()),
                            (measured.work, measured.work - 1)
                        );
                    }
                    other => {
                        panic!("expected exact final LICM LLVM comparison Work refusal: {other:?}")
                    }
                }
                assert_eq!(
                    short.work,
                    measured.work - (2 * measured.llvm.unwrap().0 + 1)
                );
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
fn licm_native_success_keeps_seeded_denials_actual_receipts_and_the_same_ledger() {
    for erased in [false, true] {
        for mutation in [false, true] {
            with_prefix(erased, Profile::Gfx942, mutation, |prefix, parent| {
                const W: usize = 1_000_000_000;
                const S: usize = 1024 * 1024 * 1024;
                let sibling = vec![0x47_u8; 47];
                let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
                let mut work = Work::new(W);
                {
                    let mut budget = Budget::new(&mut work, S);
                    budget.reserve_storage(floor).unwrap();
                    budget.charge_work(7).unwrap();
                    match budget.charge_work(W) {
                        Err(Resource::Work(e)) => assert_eq!((e.actual(), e.limit()), (W + 7, W)),
                        other => panic!("seeded Work denial: {other:?}"),
                    }
                    match budget.reserve_storage(S) {
                        Err(Resource::Storage(e)) => {
                            assert_eq!((e.actual(), e.limit()), (S + floor, S))
                        }
                        other => panic!("seeded Storage denial: {other:?}"),
                    }
                    let ledger = budget.work_ledger_identity_v1();
                    let (value, receipt) = prepare(prefix, Profile::Gfx942, &mut budget).unwrap();
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
                    assert!(sibling.iter().all(|b| *b == 0x47));
                }
                assert_eq!(work.failed_work(), Some(W + 7));
            });
        }
    }
}

#[test]
fn licm_native_storage_short_observation_keeps_first_denial_and_live_floor() {
    // The final replay's native scratch reservation precedes its text comparison.
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let measured =
                    measure(erased, mutation, profile, 1_000_000_000, 1024 * 1024 * 1024);
                assert!(measured.error.is_none());
                let short = measure(erased, mutation, profile, measured.work, measured.peak - 1);
                match short.error {
                    Some(ProductionPipelineError::PrivateCellNativeStage(
                        PrivateCellNativeStageErrorV1::Resource(Resource::Storage(error)),
                    )) => assert_eq!(
                        (error.actual(), error.limit()),
                        (measured.peak, measured.peak - 1)
                    ),
                    other => {
                        panic!("expected exact final native scratch Storage refusal: {other:?}")
                    }
                }
                let owner_header = if erased {
                    size_of::<ErasedLicm>()
                } else {
                    size_of::<DirectLicm>()
                };
                let candidate_header = size_of::<PreparedLicmNativeOutputV1>()
                    .checked_sub(owner_header)
                    .and_then(|bytes| bytes.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
                    .unwrap();
                let (llvm_len, llvm_capacity) = measured.llvm.unwrap();
                // Between the two identical scratch requests, retain only the
                // actual LLVM backing and the completed candidate's new headers.
                assert_eq!(
                    short.peak,
                    measured
                        .peak
                        .checked_sub(candidate_header.checked_add(llvm_capacity).unwrap())
                        .unwrap()
                );
                assert_eq!(
                    short.work,
                    measured
                        .work
                        .checked_sub(llvm_len.checked_mul(2).unwrap().checked_add(1).unwrap())
                        .unwrap()
                );
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
fn licm_native_scope_refuses_foreign_ledger_after_genuine_preparation() {
    with_prefix(false, Profile::Gfx942, true, |prefix, parent| {
        let sibling = vec![0x62_u8; 41];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut original_work = Work::new(1_000_000_000);
        let mut other_work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut original_work, 1024 * 1024 * 1024);
        let mut replacement = Budget::new(&mut other_work, 1024 * 1024 * 1024);
        budget.reserve_storage(floor).unwrap();
        replacement.reserve_storage(floor).unwrap();
        budget.charge_work(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result<()> = scoped(floor, &mut budget, |budget| {
            let (value, receipt) = prepare(prefix, Profile::Gfx942, budget)?;
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
        // Undo the hostile caller's swap; scopes cannot repair a foreign ledger.
        std::mem::swap(&mut budget, &mut replacement);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() > 7);
        assert!(budget.storage() > floor);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert!(sibling.iter().all(|b| *b == 0x62));
        assert_eq!(budget.storage(), floor);
    });
}
