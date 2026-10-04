use super::*;

struct Observation {
    error: Option<ProductionPipelineError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    llvm_bytes: Option<usize>,
    llvm_capacity: Option<usize>,
}
fn measure(erased: bool, mutation: bool, work_limit: usize, storage_limit: usize) -> Observation {
    let mut observation = None;
    with_prefix(erased, Profile::Gfx942, mutation, |prefix, parent| {
        let sibling = vec![0xd7_u8; 37];
        let floor = parent.storage() + sibling.capacity();
        let mut work = Work::new(work_limit);
        let (error, used, peak, failed_storage, llvm_bytes, llvm_capacity) = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = prepare(prefix, Profile::Gfx942, &mut budget);
            let llvm_bytes = result.as_ref().ok().map(|(value, _)| value.llvm_ir().len());
            let llvm_capacity = result.as_ref().ok().map(|(value, _)| value.llvm.capacity());
            let error = result.err();
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(sibling.iter().all(|byte| *byte == 0xd7));
            (
                error,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
                llvm_bytes,
                llvm_capacity,
            )
        };
        observation = Some(Observation {
            error,
            work: used,
            peak,
            failed_work: work.failed_work(),
            failed_storage,
            llvm_bytes,
            llvm_capacity,
        });
    });
    observation.unwrap()
}

#[test]
fn loop_preheaders_native_exact_and_work_short_report_exact_final_comparison_denial() {
    for (erased, mutation) in [(false, true), (false, false), (true, false)] {
        let measured = measure(erased, mutation, 1_000_000_000, 1024 * 1024 * 1024);
        assert!(measured.error.is_none());
        assert_eq!(
            (measured.failed_work, measured.failed_storage),
            (None, None)
        );
        let exact = measure(erased, mutation, measured.work, measured.peak);
        assert!(exact.error.is_none());
        assert_eq!(
            (exact.work, exact.peak, exact.llvm_bytes),
            (measured.work, measured.peak, measured.llvm_bytes)
        );
        assert_eq!(exact.llvm_capacity, measured.llvm_capacity);
        assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
        let short = measure(erased, mutation, measured.work - 1, measured.peak);
        match short.error {
            Some(ProductionPipelineError::LoopPreheadersNativeStage(
                LoopPreheadersNativeStageErrorV1::Resource(Resource::Work(error)),
            )) => {
                assert_eq!(
                    (error.actual(), error.limit()),
                    (measured.work, measured.work - 1)
                );
            }
            other => panic!("expected exact final LLVM-comparison work denial: {other:?}"),
        }
        assert_eq!(
            short.work,
            measured.work - (2 * measured.llvm_bytes.unwrap() + 1)
        );
        assert_eq!(short.peak, measured.peak);
        assert_eq!(
            (short.failed_work, short.failed_storage),
            (Some(measured.work), None)
        );
    }
}

#[test]
fn loop_preheaders_native_success_preserves_seeded_denials_and_same_ledger() {
    for (erased, mutation) in [(false, true), (false, false), (true, false)] {
        with_prefix(erased, Profile::Gfx942, mutation, |prefix, parent| {
            const WORK: usize = 1_000_000_000;
            const STORAGE: usize = 1024 * 1024 * 1024;
            let sibling = vec![0x47_u8; 47];
            let floor = parent.storage() + sibling.capacity();
            let mut work = Work::new(WORK);
            {
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.charge_work(7).unwrap();
                budget.reserve_storage(floor).unwrap();
                match budget.charge_work(WORK) {
                    Err(Resource::Work(error)) => {
                        assert_eq!((error.actual(), error.limit()), (WORK + 7, WORK))
                    }
                    other => panic!("expected seeded work denial: {other:?}"),
                }
                match budget.reserve_storage(STORAGE) {
                    Err(Resource::Storage(error)) => {
                        assert_eq!((error.actual(), error.limit()), (STORAGE + floor, STORAGE))
                    }
                    other => panic!("expected seeded storage denial: {other:?}"),
                }
                let ledger = budget.work_ledger_identity_v1();
                let (value, receipt) = prepare(prefix, Profile::Gfx942, &mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), Some(STORAGE + floor));
                assert!(budget.work_ledger_identity_v1() == ledger);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                value.verify_equivalence(&mut budget).unwrap();
                drop(value);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), Some(STORAGE + floor));
                assert!(sibling.iter().all(|byte| *byte == 0x47));
            }
            assert_eq!(work.failed_work(), Some(WORK + 7));
        });
    }
}

#[test]
fn loop_preheaders_native_storage_short_observation_preserves_floor_and_first_denial() {
    // Refusal occurs in the final replay's actual native scratch reservation.
    for (erased, mutation) in [(false, true), (false, false), (true, false)] {
        let measured = measure(erased, mutation, 1_000_000_000, 1024 * 1024 * 1024);
        assert!(measured.error.is_none());
        let short = measure(erased, mutation, measured.work, measured.peak - 1);
        match short.error {
            Some(ProductionPipelineError::PrivateCellNativeStage(
                PrivateCellNativeStageErrorV1::Resource(Resource::Storage(error)),
            )) => assert_eq!(
                (error.actual(), error.limit()),
                (measured.peak, measured.peak - 1)
            ),
            other => panic!("expected exact final native scratch Storage refusal: {other:?}"),
        }
        let owner_header = if erased {
            size_of::<ErasedPreheaders>()
        } else {
            size_of::<DirectPreheaders>()
        };
        let candidate_header = size_of::<PreparedLoopPreheadersNativeOutputV1>()
            .checked_sub(owner_header)
            .and_then(|bytes| bytes.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
            .unwrap();
        assert_eq!(
            short.peak,
            measured
                .peak
                .checked_sub(
                    candidate_header
                        .checked_add(measured.llvm_capacity.unwrap())
                        .unwrap()
                )
                .unwrap()
        );
        assert_eq!(
            short.work,
            measured
                .work
                .checked_sub(
                    measured
                        .llvm_bytes
                        .unwrap()
                        .checked_mul(2)
                        .unwrap()
                        .checked_add(1)
                        .unwrap()
                )
                .unwrap()
        );
        assert_eq!(
            (short.failed_work, short.failed_storage),
            (None, Some(measured.peak))
        );
        assert!(short.peak < measured.peak);
        assert!(short.work < measured.work);
    }
}
