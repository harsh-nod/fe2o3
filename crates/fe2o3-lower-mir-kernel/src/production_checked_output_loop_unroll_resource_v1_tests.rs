use super::*;
const PREFIX: usize = 17;
struct Observation {
    result: Result<(), Error>,
    floor: usize,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(
    shared: bool,
    profile: Profile,
    bound: Option<u32>,
    replay: bool,
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let (input, inherited) = forwarded(shared, profile, bound);
    let (mut input, mut ready, mut retained) = (Some(input), None, inherited);
    if replay {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, receipt) = input
            .take()
            .unwrap()
            .unroll(Limits::default(), &mut budget)
            .unwrap();
        retained += receipt.retained_storage();
        ready = Some(owner);
    }
    let sibling = vec![0x79u8; 37];
    let floor = retained + std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    work.charge_work(PREFIX).unwrap();
    let (result, accepted, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = if replay {
            ready.as_ref().unwrap().replay(&mut budget)
        } else {
            input
                .take()
                .unwrap()
                .unroll(Limits::default(), &mut budget)
                .map(|(owner, receipt)| {
                    // Getter-only inspection is not further controlled replay.
                    assert!(receipt.retained_storage() > 0);
                    assert_eq!(owner.limits(), Limits::default());
                    drop(owner);
                })
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x79; 37]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    drop(ready);
    Observation {
        result,
        floor,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
#[test]
fn source_unroll_factory_and_replay_exact_work_peak_and_terminal_work_short() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(3), None] {
                for replay in [false, true] {
                    let full = measure(shared, profile, bound, replay, WORK, STORAGE);
                    assert!(full.result.is_ok());
                    assert!(full.work > PREFIX && full.peak > full.floor);
                    let exact = measure(shared, profile, bound, replay, full.work, full.peak);
                    assert!(exact.result.is_ok());
                    assert_eq!(
                        (exact.work, exact.peak, exact.floor),
                        (full.work, full.peak, full.floor)
                    );
                    assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
                    let short = measure(shared, profile, bound, replay, full.work - 1, full.peak);
                    match short.result {
                        Err(Error::Resource(AssertOriginResourceV1::Work(error))) => {
                            assert_eq!((error.actual(), error.limit()), (full.work, full.work - 1));
                        }
                        other => panic!("exact outer final charge: {other:?}"),
                    }
                    assert_eq!(
                        (
                            short.work,
                            short.peak,
                            short.failed_work,
                            short.failed_storage
                        ),
                        (full.work - 1, full.peak, Some(full.work), None)
                    );
                }
            }
        }
    }
}
#[test]
fn source_unroll_storage_observations_require_strict_phase_successor() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(3), None] {
                for replay in [false, true] {
                    let full = measure(shared, profile, bound, replay, WORK, STORAGE);
                    assert!(full.result.is_ok());
                    let short = measure(shared, profile, bound, replay, full.work, full.peak - 1);
                    assert!(short.result.is_err());
                    assert_eq!(short.floor, full.floor);
                    assert_eq!(short.failed_work, None);
                    assert!(short.failed_storage.is_some());
                    eprintln!(
                        "SOURCE_UNROLL_STORAGE shared={shared} profile={profile:?} bound={bound:?} replay={replay} floor={} W={} P={} accepted={} prior_peak={} failed_storage={:?} error={:?}",
                        full.floor,
                        full.work,
                        full.peak,
                        short.work,
                        short.peak,
                        short.failed_storage,
                        short.result
                    );
                }
            }
        }
    }
}
#[test]
fn source_unroll_source_cap_and_iteration_ceiling_are_typed_refusals() {
    for shared in [false, true] {
        for oversized_source in [false, true] {
            let (input, floor) = forwarded(shared, Profile::Gfx942, Some(3));
            let mut limits = Limits::default();
            if oversized_source {
                limits.loops.operations = usize::MAX;
            } else {
                limits.max_iterations = 9;
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let result = input.unroll(limits, &mut budget);
            match result {
                Err(Error::SourceOperationsLimit {
                    requested,
                    source_limit,
                }) if oversized_source => {
                    assert_eq!(requested, usize::MAX);
                    assert!(source_limit < requested);
                }
                Err(Error::Continuation(
                    fe2o3_kernel_opt::OwnedLoopUnrollErrorV1::InvalidIterationLimit(9),
                )) if !oversized_source => {}
                Err(error) => panic!("exact configuration refusal: {error:?}"),
                Ok(_) => panic!("configuration is not a no-op"),
            }
            assert_eq!(budget.storage(), floor);
        }
    }
}
