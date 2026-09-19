use super::*;

fn resource(error: &Error) -> Option<Resource> {
    use fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1 as InventoryError;
    use fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12 as Admission;
    use fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1 as Capture;
    match error {
        Error::Resource(error)
        | Error::Inventory(InventoryError::Resource(error))
        | Error::Loops(CanonicalKirLoopErrorV1::Resource(error))
        | Error::Occurrences(Capture::Resource(error))
        | Error::Materialization(ProductionPreRankedKirErrorV1::Occurrences(Capture::Resource(
            error,
        )))
        | Error::Materialization(ProductionPreRankedKirErrorV1::Canonical(Admission::Resource(
            error,
        )))
        | Error::Materialization(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
        )) => Some(*error),
        _ => None,
    }
}

#[test]
fn constructor_has_exact_and_one_short_cumulative_work_and_peak_storage() {
    fn execute(work_limit: usize, storage_limit: usize) -> (Result<usize>, usize, usize) {
        let (ssa, launch, _) = fixture::source(30);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
            ssa,
            launch,
            Default::default(),
            &mut budget,
        )
        .map(|owner| owner.retained_analysis_storage_v1());
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    }
    let (result, used, peak) = execute(WORK, STORAGE);
    let receipt = result.unwrap();
    assert_eq!(execute(used, peak).0.unwrap(), receipt);
    assert!(matches!(
        resource(&execute(used - 1, peak).0.unwrap_err()),
        Some(Resource::Work(_))
    ));
    assert!(matches!(
        resource(&execute(used, peak - 1).0.unwrap_err()),
        Some(Resource::Storage(_))
    ));
}

#[test]
fn scoped_replay_and_query_have_independent_exact_and_one_short_limits() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let (owner, certificate) = materialize(&mut setup);
    let execute = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let floor = FLOOR + owner.retained_analysis_storage_v1();
        budget.reserve_storage(floor).unwrap();
        let result = query(&owner, &certificate, &mut budget).map(|fact| joined(fact).recurrence());
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, used, peak) = execute(WORK, STORAGE);
    let recurrence = result.unwrap();
    assert_eq!(execute(used, peak).0.unwrap(), recurrence);
    assert!(matches!(
        resource(&execute(used - 1, peak).0.unwrap_err()),
        Some(Resource::Work(_))
    ));
    assert!(matches!(
        resource(&execute(used, peak - 1).0.unwrap_err()),
        Some(Resource::Storage(_))
    ));
}

#[test]
fn a_query_error_cannot_be_ignored_to_return_success() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, certificate) = materialize(&mut budget);
    budget.reserve_storage(FLOOR + owner.retained).unwrap();
    let floor = budget.storage();
    let result = owner.with_u32_recurrences_v1(Default::default(), &mut budget, |query, budget| {
        let first = query.check_u32_certificate_v1(
            SemanticFunctionIdV1::from_index(9),
            &certificate,
            budget,
        );
        assert!(matches!(
            first,
            Err(Error::Mismatch("certificate actual root/body alias"))
        ));
        let before = budget.work();
        let second = query.check_u32_certificate_v1(ROOT, &certificate, budget);
        assert!(matches!(
            second,
            Err(Error::Mismatch("certificate actual root/body alias"))
        ));
        assert_eq!(budget.work(), before);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Error::Mismatch("certificate actual root/body alias"))
    ));
    assert_eq!(budget.storage(), floor);
}

struct Dropped(Rc<Cell<usize>>, bool);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        if self.1 {
            panic!("rejected query result destructor")
        }
    }
}

#[test]
fn actual_setup_replay_error_and_panic_release_all_local_scratch_before_callback() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, certificate) = materialize(&mut budget);
    budget.reserve_storage(FLOOR + owner.retained).unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    for panic in [false, true] {
        SETUP_REPLAY_OBSERVED.set(None);
        assert!(
            SETUP_REPLAY_FAULT
                .with(|slot| slot.replace(Some(if panic {
                    SetupReplayFault::Panic
                } else {
                    SetupReplayFault::Error
                })))
                .is_none()
        );
        let called = Cell::new(false);
        let result = owner.with_u32_recurrences_v1(Default::default(), &mut budget, |_, _| {
            called.set(true);
            Ok(())
        });
        if panic {
            assert!(matches!(result, Err(Error::Panicked)));
        } else {
            assert!(matches!(
                result,
                Err(Error::Mismatch("injected setup replay failure"))
            ));
        }
        assert!(!called.get());
        assert!(SETUP_REPLAY_FAULT.with(|slot| slot.borrow().is_none()));
        let (reserved, capacity) = SETUP_REPLAY_OBSERVED.get().unwrap();
        assert!(reserved > floor);
        assert!(capacity > 0);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        joined(query(&owner, &certificate, &mut budget).unwrap());
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn setup_panic_payload_destructor_runs_after_temporary_and_retained_cleanup() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, certificate) = materialize(&mut budget);
    budget.reserve_storage(FLOOR + owner.retained).unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let count = Arc::new(AtomicUsize::new(0));
    SETUP_REPLAY_OBSERVED.set(None);
    assert!(
        SETUP_REPLAY_FAULT
            .with(|slot| { slot.replace(Some(SetupReplayFault::PanicPayload(count.clone()))) })
            .is_none()
    );
    let called = Cell::new(false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        owner.with_u32_recurrences_v1(Default::default(), &mut budget, |_, _| {
            called.set(true);
            Ok(())
        })
    }));
    assert!(result.is_err());
    assert!(!called.get());
    assert!(SETUP_REPLAY_FAULT.with(|slot| slot.borrow().is_none()));
    let (reserved, capacity) = SETUP_REPLAY_OBSERVED.get().unwrap();
    assert!(reserved > floor);
    assert!(capacity > 0);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    joined(query(&owner, &certificate, &mut budget).unwrap());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn callback_error_panic_extra_reservation_and_rejected_destructor_preserve_exact_floor() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let (owner, _) = materialize(&mut setup);
    for mode in 0..4 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + owner.retained;
        budget.reserve_storage(floor).unwrap();
        let dropped = Rc::new(Cell::new(0));
        let result =
            owner.with_u32_recurrences_v1(
                Default::default(),
                &mut budget,
                |_, budget| match mode {
                    0 => Err(Error::Mismatch("callback")),
                    1 => panic!("callback panic"),
                    _ => {
                        budget.reserve_storage(23).unwrap();
                        Ok(Dropped(dropped.clone(), mode == 3))
                    }
                },
            );
        match mode {
            0 => assert!(matches!(result, Err(Error::Mismatch("callback")))),
            1 => assert!(matches!(result, Err(Error::Panicked))),
            _ => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
        }
        assert_eq!(dropped.get(), usize::from(mode >= 2));
        assert_eq!(budget.storage(), floor + if mode >= 2 { 23 } else { 0 });
    }
}

#[test]
fn panic_payload_drop_is_delayed_until_after_owned_reports_and_exact_cleanup() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("panic payload destructor");
        }
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, _) = materialize(&mut budget);
    budget.reserve_storage(FLOOR + owner.retained).unwrap();
    let floor = budget.storage();
    let count = Arc::new(AtomicUsize::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        owner.with_u32_recurrences_v1(Default::default(), &mut budget, |_, _| -> Result<()> {
            std::panic::panic_any(Payload(count.clone()))
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), floor);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn foreign_ledger_or_wrong_budget_slot_stays_untouched_even_after_restoration() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let (owner, certificate) = materialize(&mut setup);
    for wrong_slot in [false, true] {
        let mut original_work = Work::new(WORK);
        let mut foreign_work = Work::new(WORK);
        let mut budget = Budget::new(&mut original_work, STORAGE);
        let mut spare = Budget::new(&mut foreign_work, STORAGE);
        let floor = FLOOR + owner.retained;
        budget.reserve_storage(floor).unwrap();
        spare.reserve_storage(floor).unwrap();
        let mut scoped = 0;
        let result =
            owner.with_u32_recurrences_v1(Default::default(), &mut budget, |query, budget| {
                scoped = budget.storage() - floor;
                std::mem::swap(budget, &mut spare);
                let target = if wrong_slot { &mut spare } else { &mut *budget };
                let before = (target.work(), target.storage());
                assert!(matches!(
                    query.check_u32_certificate_v1(ROOT, &certificate, target),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!((target.work(), target.storage()), before);
                std::mem::swap(budget, &mut spare);
                Ok(())
            });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(budget.storage(), floor + scoped);
        assert_eq!(spare.storage(), floor);
        budget.release_storage(scoped).unwrap();
    }
}

#[test]
fn undercut_then_restored_floor_is_latched_and_never_released_again() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, certificate) = materialize(&mut budget);
    budget.reserve_storage(FLOOR + owner.retained).unwrap();
    let floor = budget.storage();
    let mut scoped = 0;
    let result = owner.with_u32_recurrences_v1(Default::default(), &mut budget, |query, budget| {
        scoped = budget.storage() - floor;
        budget.release_storage(1).unwrap();
        assert!(matches!(
            query.check_u32_certificate_v1(ROOT, &certificate, budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        budget.reserve_storage(1).unwrap();
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), floor + scoped);
    budget.release_storage(scoped).unwrap();
}

#[test]
fn scoped_constructor_prepaid_growth_counts_old_and_new_actual_capacities() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let values = resources::scoped(&mut budget, |budget| {
        let mut values = Vec::<u64>::new();
        reserve(&mut values, 1, budget)?;
        let first = table_bytes::<u64>(values.capacity())?;
        values.push(3);
        reserve(&mut values, 17, budget)?;
        let second = table_bytes::<u64>(values.capacity())?;
        assert_eq!(budget.storage(), FLOOR + second);
        assert!(budget.peak_storage() >= FLOOR + first + second);
        Ok(values)
    })
    .unwrap();
    assert_eq!(values, [3]);
    assert_eq!(budget.storage(), FLOOR);
    assert!(matches!(
        table_bytes::<u64>(usize::MAX),
        Err(Error::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn failed_prepaid_copy_of_existing_table_releases_the_candidate_only() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, STORAGE);
    let mut values = vec![9u64];
    let bytes = table_bytes::<u64>(values.capacity()).unwrap();
    budget.reserve_storage(FLOOR + bytes).unwrap();
    assert!(matches!(
        reserve(&mut values, 32, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(values, [9]);
    assert_eq!(budget.storage(), FLOOR + bytes);
}
