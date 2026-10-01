use super::*;
use tile_materialization_faults_v29 as hooks;

#[test]
fn observation_callback_resource_error_is_not_reclassified_or_refunded() {
    for storage in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let mut donor = fixture(&mut budget);
        let error = Pending::with_scalar_candidate_observation_v29(
            &mut donor,
            &mut budget,
            |view, budget| {
                assert!(!view.grants_execution_authority());
                if storage {
                    budget.reserve_storage(SCHEDULE_LIMIT)?;
                } else {
                    budget.charge_work(SCHEDULE_LIMIT)?;
                }
                Ok::<(), ArgumentResourceV1>(())
            },
        )
        .unwrap_err();
        match error {
            ObservationError::Callback(ArgumentResourceV1::Storage(limit)) if storage => {
                assert_eq!(budget.failed_storage(), Some(limit.actual()));
            }
            ObservationError::Callback(ArgumentResourceV1::Work(limit)) if !storage => {
                assert_eq!(budget.failed_work(), Some(limit.actual()));
            }
            _ => panic!("callback resource kind was changed: {error:?}"),
        }
        assert!(donor.is_none());
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    }
}

struct Armed;
impl Drop for Armed {
    fn drop(&mut self) {
        hooks::ARMED.with(|slot| {
            slot.replace(None);
        });
        hooks::SEEN.set(None);
    }
}

#[test]
fn observation_materializer_refusal_and_panic_keep_original_source_account() {
    use hooks::Point::*;
    for point in [
        CopyTransfer,
        CopyAdopted,
        Emitted,
        V18Transfer,
        V18Adopted,
        Projected,
    ] {
        for preexisting in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            assert!(work.charge_work(SCHEDULE_LIMIT + 7).is_err());
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
            let (pending, capture) =
                pending_source(SourceCase::Shifted, preexisting, None, &mut budget);
            let mut donor = Some(pending);
            assert!(budget.reserve_storage(SCHEDULE_LIMIT + 11).is_err());
            let denied = (budget.failed_work(), budget.failed_storage());
            let ledger = budget.work_ledger_identity_v1();
            let payload = Box::new([0x5a_u8; 17]);
            let address = &*payload as *const [u8; 17];
            hooks::ARMED.with(|slot| assert!(slot.replace(Some((point, payload))).is_none()));
            hooks::SEEN.set(None);
            let _armed = Armed;
            let mut visits = 0;
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |_, _| {
                    visits += 1;
                    Ok::<(), ()>(())
                })
            }));
            assert!(
                hooks::ARMED.with(|slot| slot.borrow().is_none()),
                "checkpoint not reached"
            );
            let seen = hooks::SEEN.get().unwrap();
            assert_eq!(visits, 0);
            assert!(donor.is_none());
            assert_eq!(budget.storage(), SCHEDULE_FLOOR + capture);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.work(), seen.0);
            assert_eq!(budget.peak_storage(), seen.2);
            assert_eq!(budget.failed_work(), denied.0);
            if matches!(point, CopyTransfer | V18Transfer) {
                let error = outcome.unwrap().unwrap_err();
                let ObservationError::Resource(ArgumentResourceV1::Storage(limit)) = error else {
                    panic!("exact failed receipt admission: {error:?}");
                };
                assert_eq!(
                    budget.failed_storage(),
                    Some(denied.1.unwrap().max(limit.actual()))
                );
            } else {
                let payload = outcome.unwrap_err().downcast::<[u8; 17]>().unwrap();
                assert_eq!(&*payload as *const [u8; 17], address);
                assert_eq!(*payload, [0x5a; 17]);
                assert_eq!(budget.failed_storage(), denied.1);
            }
        }
    }
}

#[test]
fn observation_callback_error_drop_and_payment_remain_callers_responsibility() {
    struct Error<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Error<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = std::cell::Cell::new(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut donor = fixture(&mut budget);
    let outcome =
        Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |_, budget| {
            budget
                .reserve_storage(std::mem::size_of::<Error<'_>>())
                .unwrap();
            Err(Error(&drops))
        });
    assert!(matches!(outcome, Err(ObservationError::Callback(_))));
    assert_eq!(drops.get(), 0);
    assert_eq!(
        budget.storage(),
        SCHEDULE_FLOOR + std::mem::size_of::<Error<'_>>()
    );
    drop(outcome);
    assert_eq!(drops.get(), 1);
    budget
        .release_storage(std::mem::size_of::<Error<'_>>())
        .unwrap();
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
}
