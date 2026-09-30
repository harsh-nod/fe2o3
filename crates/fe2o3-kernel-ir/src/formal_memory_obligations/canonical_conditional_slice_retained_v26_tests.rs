use super::*;

struct RejectedValue<'a> {
    dropped: &'a Cell<usize>,
    panics: bool,
}
impl Drop for RejectedValue<'_> {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
        assert!(!self.panics, "rejected retained callback value");
    }
}

#[test]
fn conditional_slice_retained_loss_survives_every_ancestor_and_callback_disposition() {
    for disposition in 0..8 {
        let (owner, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, true));
        let floor = credit + 17;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = Budget::new(&mut work, 100_000_000);
        budget.reserve_storage(floor).unwrap();
        let retained = Cell::new(None);
        let crossed = Cell::new(0);
        let dropped = Cell::new(0);
        let expected = RefCell::new(Failure::Resource(ResourceError::Accounting));
        let result = with_canonical_guarded_global_reads_v18(
            &owner,
            Default::default(),
            &mut budget,
            |reads, budget| {
                let result = with_canonical_guarded_global_stores_v24(
                    &owner,
                    Default::default(),
                    budget,
                    |stores, budget| {
                        let parent_floor = budget.storage();
                        let result = with_canonical_conditional_slice_domains_v26(
                            reads,
                            stores,
                            &[batch_launch_v26()],
                            FormalIndexWidth::Bits64,
                            budget,
                            |batch, budget| {
                                if disposition == 3 {
                                    budget.charge_work(100_000_000 - budget.work())?;
                                    let error = batch.function_count(budget).unwrap_err();
                                    assert!(matches!(
                                        error,
                                        Failure::Resource(ResourceError::Work(_))
                                    ));
                                    *expected.borrow_mut() = error;
                                } else if disposition == 7 {
                                    let error =
                                        reads.read_at(coordinate(99), budget).err().unwrap();
                                    assert!(matches!(error, Failure::Coordinate(_)));
                                    *expected.borrow_mut() = error;
                                }
                                if disposition == 5 {
                                    assert_eq!(
                                        batch.refuse_retained_custody(),
                                        Failure::Resource(ResourceError::Accounting)
                                    );
                                } else {
                                    budget.release_storage(1)?;
                                    assert!(budget.storage() > parent_floor);
                                }
                                if disposition == 4 {
                                    assert!(matches!(
                                        batch.function_count(budget),
                                        Err(Failure::Resource(ResourceError::Accounting))
                                    ));
                                    budget.reserve_storage(1)?;
                                    assert!(batch.owner(budget).is_err());
                                }
                                retained.set(Some(budget.storage()));
                                match disposition {
                                    1 => Err(Failure::Coordinate(coordinate(98))),
                                    2 => panic!("lost batch floor callback panic"),
                                    _ => Ok(RejectedValue {
                                        dropped: &dropped,
                                        panics: disposition == 6,
                                    }),
                                }
                            },
                        );
                        assert_eq!(Some(budget.storage()), retained.get());
                        crossed.set(crossed.get() + 1);
                        result
                    },
                );
                assert_eq!(Some(budget.storage()), retained.get());
                crossed.set(crossed.get() + 1);
                result
            },
        );
        let error = result.err().expect("retained custody must refuse");
        assert_eq!(error, *expected.borrow(), "disposition={disposition}");
        assert_eq!(crossed.get(), 2);
        assert_eq!(Some(budget.storage()), retained.get());
        assert!(budget.storage() > floor);
        assert_eq!(dropped.get(), usize::from(!matches!(disposition, 1 | 2)));
        // Every rejected value and analysis backing is now dead. Only this
        // fixture, never a production ancestor, drains the refused credit.
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn guarded_read_retained_loss_stays_refused_after_observed_credit_is_restored() {
    let (owner, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, true));
    let floor = credit + 17;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    budget.reserve_storage(floor).unwrap();
    let retained = Cell::new(None);
    let result = with_canonical_guarded_global_reads_v18(
        &owner,
        Default::default(),
        &mut budget,
        |reads, budget| {
            budget.release_storage(1)?;
            assert!(matches!(
                reads.function_count(budget),
                Err(Failure::Resource(ResourceError::Accounting))
            ));
            budget.reserve_storage(1)?;
            retained.set(Some(budget.storage()));
            assert!(reads.owner(budget).is_err());
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
    assert_eq!(Some(budget.storage()), retained.get());
    assert!(budget.storage() > floor);
    budget.release_storage(budget.storage() - floor).unwrap();
}

#[test]
fn guarded_store_fact_retains_its_higher_invocation_backing_floor() {
    let (owner, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, true));
    let floor = credit + 17;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    budget.reserve_storage(floor).unwrap();
    let retained = Cell::new(None);
    let result = with_canonical_guarded_global_stores_v24(
        &owner,
        Default::default(),
        &mut budget,
        |stores, budget| {
            let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(fact) =
                stores.store_at(coordinate(2), budget)?
            else {
                panic!("fixture store must have local conditions");
            };
            budget.release_storage(1)?;
            assert!(matches!(
                fact.invocation_projection(budget),
                Err(Failure::Resource(ResourceError::Accounting))
            ));
            budget.reserve_storage(1)?;
            retained.set(Some(budget.storage()));
            assert!(fact.invocation_projection(budget).is_err());
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
    assert_eq!(Some(budget.storage()), retained.get());
    assert!(budget.storage() > floor);
    budget.release_storage(budget.storage() - floor).unwrap();
}

#[test]
fn conditional_slice_intact_scratch_is_refunded_after_callback_error_and_panic() {
    for panics in [false, true] {
        let dropped = Cell::new(0);
        let (result, _, _) = run_batch_v26(
            &fixture(Axis::X, AccessMode::ReadWrite, true),
            batch_launch_v26(),
            FormalIndexWidth::Bits64,
            100_000_000,
            100_000_000,
            |_, budget| -> Result<RejectedValue<'_>> {
                budget.reserve_storage(19)?;
                if panics {
                    panic!("intact scratch callback panic");
                }
                Ok(RejectedValue {
                    dropped: &dropped,
                    panics: true,
                })
            },
        );
        assert!(matches!(
            result,
            Err(Failure::Resource(ResourceError::Accounting)) | Err(Failure::Panicked)
        ));
        assert_eq!(dropped.get(), usize::from(!panics));
    }
}
