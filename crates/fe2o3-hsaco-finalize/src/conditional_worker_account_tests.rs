use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
const OUTSIDE: usize = fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1 + 1;

#[test]
fn retained_worker_account_exact_and_one_short_work_and_storage() {
    let work = 16 + Budget::STORAGE_WINDOW_WORK_V1 + 5;
    let peak = OUTSIDE + 7 + Budget::STORAGE_WINDOW_SCRATCH_V1 + 4 * size_of::<AccountMode>() + 11;
    for (work_limit, storage_limit, success) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let mut owned = Owned::new(Work::new(work_limit), storage_limit);
        owned.with_budget(|b| {
            b.reserve_storage(OUTSIDE).unwrap();
            let identity = b.storage_account_identity_v1();
            let ledger = b.work_ledger_identity_v1();
            let address = b as *const Budget<'_> as usize;
            let mode = AccountMode::original(b).unwrap();
            let result = mode.run(b, 7, |b| -> Result<_, Resource> {
                assert_eq!(b.storage_account_identity_v1(), identity);
                assert_eq!(b.work_ledger_identity_v1(), ledger);
                assert_eq!(b as *const Budget<'_> as usize, address);
                b.charge_work(5)?;
                b.reserve_storage(11)?;
                Ok(23)
            });
            assert_eq!(result.is_ok(), success);
            if success {
                assert_eq!(result.unwrap(), 23);
                assert_eq!((b.work(), b.peak_storage()), (work, peak));
            }
            assert_eq!(b.storage(), OUTSIDE);
            assert_eq!(b.storage_limit(), storage_limit);
        });
    }
}

#[test]
fn retained_worker_account_refuses_foreign_inline_unpaid_and_widening() {
    let mut owner = Owned::new(Work::new(usize::MAX), OUTSIDE * 3);
    let mut alien = Owned::new(Work::new(usize::MAX), OUTSIDE * 3);
    owner.with_budget(|b| {
        b.reserve_storage(OUTSIDE).unwrap();
        let mode = AccountMode::original(b).unwrap();
        alien.with_budget(|other| {
            other.reserve_storage(OUTSIDE).unwrap();
            assert!(
                mode.run::<(), Resource>(other, 0, |_| panic!("foreign account"))
                    .is_err()
            );
            assert_eq!(other.storage(), OUTSIDE);
        });
        assert!(
            mode.run::<(), Resource>(b, OUTSIDE + 1, |_| panic!("unpaid input"))
                .is_err()
        );
        assert!(
            mode.run::<(), Resource>(b, usize::MAX, |_| panic!("overflow"))
                .is_err()
        );
        assert!(
            b.with_additional_storage_window_v1(4096, |b| {
                mode.run::<(), Resource>(b, 0, |_| panic!("widened parent window"))
            })
            .is_err()
        );
        assert_eq!(b.storage(), OUTSIDE);
    });
    let mut work = Work::new(32);
    let mut inline = Budget::new(&mut work, OUTSIDE * 2);
    assert!(AccountMode::original(&mut inline).is_err());
    assert_eq!(
        AccountMode::LEGACY
            .run(&mut inline, usize::MAX, |_| Ok::<_, Resource>(19))
            .unwrap(),
        19
    );
}

#[test]
fn retained_worker_account_refusal_and_unwind_restore_only_inert_scope() {
    let mut owned = Owned::new(Work::new(usize::MAX), OUTSIDE * 2);
    owned.with_budget(|b| {
        b.reserve_storage(OUTSIDE).unwrap();
        let mode = AccountMode::original(b).unwrap();
        for panic in [false, true] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                mode.run(b, 3, |b| -> Result<(), Resource> {
                    b.reserve_storage(7)?;
                    if panic {
                        panic!("inert Worker reconstruction");
                    }
                    Err(Resource::Accounting)
                })
            }));
            assert!(matches!(result, Ok(Err(_)) | Err(_)));
            assert_eq!(b.storage(), OUTSIDE);
        }
    });
}

#[test]
fn replay_quote_includes_spare_capacities_and_rejects_unbounded_roster() {
    let mut providers = Vec::with_capacity(9);
    providers.push(Vec::with_capacity(128));
    providers.push(Vec::with_capacity(256));
    let expected = size_of::<Vec<Vec<u8>>>()
        + providers.capacity() * size_of::<Vec<u8>>()
        + providers.iter().map(Vec::capacity).sum::<usize>();
    let mut work = Work::new(10);
    let mut b = Budget::new(&mut work, 0);
    assert_eq!(replay_input_storage(&providers, &mut b).unwrap(), expected);
    assert_eq!(b.work(), 10);
    let mut work = Work::new(9);
    let mut b = Budget::new(&mut work, 0);
    assert!(replay_input_storage(&providers, &mut b).is_err());
    let mut work = Work::new(8);
    let mut b = Budget::new(&mut work, 0);
    assert!(replay_input_storage(&vec![vec![]; MAX_LINK_INPUTS + 1], &mut b).is_err());
    assert_eq!(b.work(), 8);
}
