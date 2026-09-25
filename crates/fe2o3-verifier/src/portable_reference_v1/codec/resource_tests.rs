use super::*;

fn run<R>(
    decoding: bool,
    f: &Fixture,
    bytes: &[u8],
    budget: &mut Budget<'_>,
    consume: impl FnOnce(&mut Budget<'_>) -> R,
) -> Result<R, Error> {
    if decoding {
        with_decoded_native_cpu_input_v1(bytes, budget, |_, b| consume(b))
    } else {
        with_encoded_native_cpu_input_v1(f.input(), budget, |_, _, b| consume(b))
    }
}

#[test]
fn encode_and_decode_exact_and_one_short_work_storage() {
    let f = fixture();
    let (bytes, _) = encode(f.input()).unwrap();
    for decoding in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(31).unwrap();
        run(decoding, &f, &bytes, &mut budget, |_| ()).unwrap();
        let (cost, peak) = (budget.work(), budget.peak_storage());
        assert_eq!(budget.storage(), 31);
        for (work_limit, storage_limit, denial) in
            [(cost, peak, 0), (cost - 1, peak, 1), (cost, peak - 1, 2)]
        {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(31).unwrap();
            let mut called = false;
            let result = run(decoding, &f, &bytes, &mut budget, |_| {
                called = true;
            });
            match denial {
                0 => {
                    result.unwrap();
                    assert!(called);
                    assert_eq!(budget.work(), cost);
                    assert_eq!(budget.peak_storage(), peak);
                }
                1 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert!(!called);
                    assert!(budget.failed_work().is_some());
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert!(!called);
                    assert!(budget.failed_storage().is_some());
                }
            }
            assert_eq!(budget.storage(), 31);
        }
    }
}

#[test]
fn success_callback_error_and_unwind_preserve_additions_and_first_denials() {
    let f = fixture();
    let (bytes, _) = encode(f.input()).unwrap();
    for decoding in [false, true] {
        for exit in 0..3 {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(31).unwrap();
            assert!(budget.charge_work(usize::MAX).is_err());
            assert!(budget.reserve_storage(usize::MAX).is_err());
            let denials = (budget.failed_work(), budget.failed_storage());
            let result = catch_unwind(AssertUnwindSafe(|| {
                run(decoding, &f, &bytes, &mut budget, |b| {
                    b.reserve_storage(7).unwrap();
                    b.charge_work(13).unwrap();
                    match exit {
                        0 => Ok(()),
                        1 => Err("consumer rejection"),
                        _ => panic!("codec callback"),
                    }
                })
            }));
            match exit {
                0 => result.unwrap().unwrap().unwrap(),
                1 => assert_eq!(result.unwrap().unwrap(), Err("consumer rejection")),
                _ => assert!(result.is_err()),
            }
            assert_eq!(budget.storage(), 38);
            assert!(budget.work() > 30);
            assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
        }
    }
}

#[test]
fn invalid_input_refunds_only_scratch_preserving_denials() {
    let mut f = fixture();
    let (mut bytes, _) = encode(f.input()).unwrap();
    f.digest[0] ^= 1;
    *bytes.last_mut().unwrap() ^= 1;
    for decoding in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(31).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let denials = (budget.failed_work(), budget.failed_storage());
        assert!(
            run(decoding, &f, &bytes, &mut budget, |_| panic!(
                "invalid input exposed"
            ))
            .is_err()
        );
        assert_eq!(budget.storage(), 31);
        assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
    }
}

#[test]
fn funded_foreign_account_and_one_short_floor_are_not_refunded_even_on_unwind() {
    let f = fixture();
    let (bytes, _) = encode(f.input()).unwrap();
    for decoding in [false, true] {
        for substitute in [false, true] {
            for unwind in [false, true] {
                let mut work = Work::new(usize::MAX);
                let mut budget = Budget::new(&mut work, usize::MAX);
                budget.reserve_storage(31).unwrap();
                let account = budget.work_ledger_identity_v1();
                let mut original_work = 0;
                let mut callback_state = None;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run(decoding, &f, &bytes, &mut budget, |b| {
                        original_work = b.work();
                        if substitute {
                            let protected = b.storage();
                            *b =
                                Budget::new(Box::leak(Box::new(Work::new(usize::MAX))), usize::MAX);
                            b.reserve_storage(protected).unwrap();
                            b.charge_work(13).unwrap();
                            assert!(b.work_ledger_identity_v1() != account);
                        } else {
                            b.release_storage(1).unwrap();
                            assert!(b.work_ledger_identity_v1() == account);
                        }
                        callback_state = Some((b.work(), b.storage()));
                        if unwind {
                            panic!("damaged account");
                        }
                    })
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), Err(Error::Resource(Resource::Accounting)));
                }
                assert_eq!(Some((budget.work(), budget.storage())), callback_state);
                drop(budget);
                assert_eq!(work.work(), original_work);
            }
        }
    }
}

#[test]
fn allocation_arithmetic_checked_before_reservation() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    assert_eq!(
        scoped(&mut budget, |s| s.vector::<u128>(usize::MAX).map(drop)),
        Err(Error::Resource(Resource::Arithmetic))
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 0, 0)
    );
    scoped(&mut budget, |s| {
        let owned = s.boxed(19u128)?;
        assert_eq!(*owned, 19);
        drop(owned);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 0);
}
