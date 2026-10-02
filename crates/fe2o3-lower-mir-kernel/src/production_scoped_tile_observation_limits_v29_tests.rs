use super::*;

#[derive(Debug)]
struct Probe {
    result: Result<(), ObservationError<()>>,
    work: usize,
    extra: usize,
    visits: usize,
    work_denied: Option<usize>,
    storage_denied: Option<usize>,
}

fn probe(allowance: Option<(usize, usize)>) -> Probe {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let mut donor = fixture(&mut budget);
    let original = donor.as_ref().unwrap().adopted_storage();
    budget
        .reserve_storage(budget.peak_storage() + 1 - budget.storage())
        .unwrap();
    if let Some((work, storage)) = allowance {
        budget
            .charge_work(SCHEDULE_LIMIT - budget.work() - work)
            .unwrap();
        budget
            .reserve_storage(SCHEDULE_LIMIT - budget.storage() - storage)
            .unwrap();
    }
    let before = budget.work();
    let entry = budget.storage();
    let mut visits = 0;
    let result = Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |_, _| {
        visits += 1;
        Ok(())
    });
    if let Some(owner) = donor.take() {
        assert_eq!(budget.storage(), entry);
        drop(owner);
        budget.release_storage(original).unwrap();
    }
    assert_eq!(budget.storage(), entry - original);
    Probe {
        result,
        work: budget.work() - before,
        extra: budget.peak_storage() - entry,
        visits,
        work_denied: budget.failed_work(),
        storage_denied: budget.failed_storage(),
    }
}

#[test]
fn observation_exact_and_one_short_original_account_limits() {
    let baseline = probe(None);
    assert!(baseline.result.is_ok(), "{baseline:?}");
    assert_eq!(baseline.visits, 1);
    assert!(baseline.work > 0 && baseline.extra > 0);
    let exact = probe(Some((baseline.work, baseline.extra)));
    assert!(exact.result.is_ok(), "{exact:?}");
    assert_eq!(exact.visits, 1);
    assert_eq!((exact.work, exact.extra), (baseline.work, baseline.extra));
    let short = probe(Some((baseline.work - 1, baseline.extra)));
    let Err(ObservationError::Resource(ArgumentResourceV1::Work(limit))) = short.result else {
        panic!("expected exact work refusal: {short:?}");
    };
    assert_eq!(short.work_denied, Some(limit.actual()));
    let short = probe(Some((baseline.work, baseline.extra - 1)));
    let Err(ObservationError::Resource(ArgumentResourceV1::Storage(limit))) = short.result else {
        panic!("expected exact storage refusal: {short:?}");
    };
    assert_eq!(short.storage_denied, Some(limit.actual()));
}

fn huge_callback(
    payload: [u8; 8192],
) -> impl for<'view, 'work> FnOnce(
    ProductionScopedTileCandidateViewV29<'view>,
    &mut ArgumentBudgetV1<'work>,
) -> Result<(), [u8; 4096]> {
    move |_, budget| {
        assert_eq!(payload[0], 17);
        budget.reserve_storage(4096).unwrap();
        Err([29; 4096])
    }
}

fn callback_frame<E, F>(_: &F) -> usize {
    scoped_tile_observation_frame_v29::<E, F>().unwrap()
}

#[test]
fn observation_prepays_large_callback_and_error_headers_before_take() {
    for missing in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let mut donor = fixture(&mut budget);
        let callback = huge_callback([17; 8192]);
        let frame = callback_frame::<[u8; 4096], _>(&callback);
        assert!(frame >= 8192 + 2 * 4096);
        let left = frame - missing;
        budget
            .reserve_storage(SCHEDULE_LIMIT - budget.storage() - left)
            .unwrap();
        let before = budget.storage();
        let identity = *donor.as_ref().unwrap().pending_identity();
        let result =
            Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, callback);
        assert!(matches!(
            result,
            Err(ObservationError::Resource(ArgumentResourceV1::Storage(_)))
        ));
        if missing == 1 {
            assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
            assert_eq!(budget.storage(), before);
        } else {
            assert!(
                donor.is_none(),
                "exact wrapper frame passes take; preparation then refuses"
            );
            assert!(budget.storage() < before);
        }
    }
}

#[test]
fn observation_large_error_remains_caller_owned_after_candidate_drop() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut donor = fixture(&mut budget);
    let error = Pending::with_scalar_candidate_observation_v29(
        &mut donor,
        &mut budget,
        huge_callback([17; 8192]),
    )
    .unwrap_err();
    assert!(matches!(error, ObservationError::Callback([29, ..])));
    assert_eq!(budget.storage(), SCHEDULE_FLOOR + 4096);
    drop(error);
    budget.release_storage(4096).unwrap();
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
}

#[test]
fn observation_never_refunds_a_substituted_callback_ledger() {
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let mut donor = fixture(&mut budget);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 101);
        foreign.reserve_storage(101).unwrap();
        foreign.charge_work(5).unwrap();
        let mut foreign = Some(foreign);
        let result = catch_unwind(AssertUnwindSafe(|| {
            Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |_, budget| {
                let _original = std::mem::replace(budget, foreign.take().unwrap());
                if panic {
                    std::panic::panic_any(47_u32);
                }
                Ok::<(), ()>(())
            })
        }));
        if panic {
            assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 47);
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(ObservationError::Resource(ArgumentResourceV1::Accounting))
            ));
        }
        assert!(donor.is_none());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (5, 101, 101)
        );
    }
}
