//! Accounting and actual inert publication locks, not compiler authority.
use super::*;
use crate::compiler_module_handoff::conditional_v5::tests::fixture::Fixture;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::{Duration, Instant},
};

const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5;
// Shape of the historical LLVM + rustc-driver backing, not a runtime admission.
const EXTERNAL: usize = 352_457_184;

fn isolated(name: &str) -> bool {
    const ENV: &str = "FE2O3_TEST_COMPOSED_V5_CUSTODY";
    if std::env::var_os(ENV).is_some() {
        return false;
    }
    let name = format!("{}::{name}", module_path!());
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(name.strip_prefix("fe2o3_artifact_transaction::").unwrap())
        .arg("--nocapture")
        .env(ENV, "1");
    let mut child = crate::with_artifact_process_spawn_v1(|| command.spawn()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "composed custody test: {status}");
            return true;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("composed custody test timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn published(f: &Fixture) -> CompilerModuleHandoffReceiptV5 {
    // Fixture production precedes the original request, not a budget reset.
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, LIMIT);
    f.reserve(&mut b);
    f.publish(&mut b).unwrap()
}

fn locked(f: &Fixture, held: bool) {
    let fd = rustix::fs::open(
        f.path.join(crate::LOCK_FILE),
        OFlags::RDWR | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    assert_eq!(
        crate::acquire_linux_ofd_exclusive_lock(&fd, true).unwrap(),
        !held
    );
}

#[test]
fn composed_recovery_exact_one_short_and_legacy_cap() {
    if isolated("composed_recovery_exact_one_short_and_legacy_cap") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let q = compiler_module_handoff_try_recovery_quota_v5(&f.path, &f.producer, receipt.length())
        .unwrap();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    let work = WORK + q.work();
    let scratch = FRAME + q.scratch();
    for short in [None, Some(false), Some(true)] {
        let total = EXTERNAL + scratch - usize::from(short == Some(true));
        let mut account = Owned::new(Work::new(work - usize::from(short == Some(false))), total);
        account.with_budget(|b| {
            b.reserve_storage(EXTERNAL).unwrap();
            let identity = Account::capture(b).unwrap();
            let result = try_recover_compiler_module_handoff_receipt_in_root_budget_v5(
                &f.path,
                &f.producer,
                f.attempt,
                receipt.length(),
                &barrier,
                b,
            );
            match short {
                None => {
                    assert_eq!(result.unwrap(), receipt);
                    assert_eq!(b.work(), work);
                    assert_eq!(b.peak_storage(), EXTERNAL + scratch);
                }
                Some(false) => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                Some(true) => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
            identity.require(b).unwrap();
            assert_eq!(b.storage_limit(), total);
            assert_eq!(b.storage(), EXTERNAL);
            locked(&f, false);
            f.ready();
        });
    }
    let mut account = Owned::new(Work::new(usize::MAX), EXTERNAL + scratch);
    account.with_budget(|b| {
        b.reserve_storage(EXTERNAL).unwrap();
        b.with_additional_storage_window_v1(scratch, |b| {
            assert!(matches!(
                try_recover_compiler_module_handoff_receipt_with_limit_v5(
                    &f.path,
                    &f.producer,
                    f.attempt,
                    receipt.length(),
                    &barrier,
                    b
                ),
                Err(Error::Resource(Resource::Accounting))
            ));
            Ok::<_, Error>(())
        })
        .unwrap();
        let output = PinnedOutput::open(&f.path).unwrap();
        let writer = output.try_lock().unwrap().unwrap();
        assert!(matches!(
            try_recover_compiler_module_handoff_receipt_in_root_budget_v5(
                &f.path,
                &f.producer,
                f.attempt,
                receipt.length(),
                &barrier,
                b
            ),
            Err(Error::Busy)
        ));
        locked(&f, true);
        f.ready();
        drop(writer);
        assert_eq!(
            try_recover_compiler_module_handoff_receipt_in_root_budget_v5(
                &f.path,
                &f.producer,
                f.attempt,
                receipt.length(),
                &barrier,
                b
            )
            .unwrap(),
            receipt
        );
        assert_eq!(b.storage(), EXTERNAL);
    });
}

#[test]
fn one_window_pair_and_full_quote_revalidation_on_small_and_large_roots() {
    if isolated("one_window_pair_and_full_quote_revalidation_on_small_and_large_roots") {
        return;
    }
    fn send<T: Send + 'static>() {}
    send::<CompilerModuleHandoffCustodyResourcesV5>();
    send::<Lease>();
    send::<Token>();
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    let allowance = acquisition_allowance(quote).unwrap();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    // Same receipt, different actual Current; equal inert bytes never bind custody.
    let mut foreign_work = Work::new(usize::MAX);
    let mut foreign_budget = Budget::new(&mut foreign_work, LIMIT);
    let (foreign, _) = acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
        &f.path,
        &f.producer,
        &quote,
        &barrier,
        &mut foreign_budget,
    )
    .unwrap();
    foreign_budget
        .reserve_storage(quote.retained_storage())
        .unwrap();
    let (expected_subject, expected_storage) =
        Subject::from_publication(receipt, &f.handoff, &mut foreign_budget).unwrap();
    foreign_budget
        .reserve_storage(expected_storage.retained_storage())
        .unwrap();
    for external in [0, EXTERNAL] {
        let floor = external
            + quote.retained_storage()
            + size_of::<CompilerModuleHandoffCustodyResourcesV5>();
        let total = if external == 0 {
            LIMIT
        } else {
            floor + allowance
        };
        let mut account = Owned::new(Work::new(usize::MAX), total);
        account.with_budget(|b| {
            b.reserve_storage(floor).unwrap();
            let identity = Account::capture(b).unwrap();
            let mut resources = CompilerModuleHandoffCustodyResourcesV5::prepare(quote, b).unwrap();
            let mut lease = None;
            let mut token = None;
            resources
                .with_acquisition(&barrier, b, |scope| {
                    let (owner, storage) = scope.acquire_lease(&f.path, &f.producer)?;
                    lease = Some(owner);
                    scope.reserve_retained(storage)?;
                    let (owner, storage) = scope.acquire_token(lease.as_ref().unwrap())?;
                    token = Some(owner);
                    scope.reserve_retained(storage)?;
                    scope.revalidate(lease.as_ref().unwrap(), token.as_ref().unwrap())
                })
                .unwrap();
            let lease = lease.as_ref().unwrap();
            let current = token.as_ref().unwrap();
            assert_eq!(b.storage(), floor);
            locked(&f, true);
            assert!(
                resources
                    .with_acquisition::<()>(&barrier, b, |_| panic!("renewed pair allowance"))
                    .is_err()
            );
            assert!(matches!(
                resources.revalidate(&foreign, current, b),
                Err(Error::MismatchedCurrentnessToken)
            ));
            let mut other = Owned::new(Work::new(usize::MAX), total);
            other.with_budget(|other| {
                other.reserve_storage(floor).unwrap();
                assert!(matches!(
                    resources.revalidate(lease, current, other),
                    Err(Error::Resource(Resource::Accounting))
                ));
            });
            assert!(matches!(
                resources.subject(&foreign, current, b),
                Err(SubjectError::Resource(Resource::Accounting))
            ));
            let subject_needed = quote.retained_storage() + FRAME + SUBJECT_SCRATCH;
            let subject_filler = total - floor - subject_needed + 1;
            b.reserve_storage(subject_filler).unwrap();
            assert!(matches!(
                resources.subject(lease, current, b),
                Err(SubjectError::Resource(Resource::Storage(_)))
            ));
            locked(&f, true);
            assert_eq!(b.storage(), floor + subject_filler);
            b.release_storage(1).unwrap();
            let before = b.work();
            let (subject, subject_storage) = resources.subject(lease, current, b).unwrap();
            b.reserve_storage(subject_storage.retained_storage())
                .unwrap();
            assert_eq!(subject, expected_subject);
            assert_eq!(
                b.work() - before,
                WORK + crate::INERT_COMPILER_EXECUTION_SUBJECT_WORK_V3
            );
            assert_eq!(b.peak_storage(), total);
            b.release_storage(subject_filler - 1).unwrap();
            drop(subject);
            b.release_storage(subject_storage.retained_storage())
                .unwrap();
            let q = quote.currentness_revalidation_quota().unwrap();
            let needed = quote.retained_storage() + FRAME + q.scratch();
            // One short includes the complete stored Q, not just decoder scratch.
            let filler = total - floor - needed + 1;
            b.reserve_storage(filler).unwrap();
            assert!(matches!(
                resources.revalidate(lease, current, b),
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(b.storage(), floor + filler);
            locked(&f, true);
            let denial = b.failed_storage();
            b.release_storage(1).unwrap();
            let before = b.work();
            resources.revalidate(lease, current, b).unwrap();
            assert_eq!(b.work() - before, WORK + q.work());
            assert_eq!(b.peak_storage(), total);
            assert_eq!(b.failed_storage(), denial);
            b.release_storage(filler - 1).unwrap();
            identity.require(b).unwrap();
            assert_eq!(b.storage_limit(), total);
            if external != 0 {
                assert!(matches!(
                    Subject::from_publication(receipt, current.handoff(), b),
                    Err(SubjectError::Resource(Resource::Accounting))
                ));
                assert!(matches!(
                    current.revalidate_locked_currentness(b),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert!(matches!(
                    acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
                        &f.path,
                        &f.producer,
                        &quote,
                        &barrier,
                        b
                    ),
                    Err(Error::Resource(Resource::Accounting))
                ));
            }
            drop(token.take());
            locked(&f, false);
            assert_eq!(b.storage(), floor);
        });
    }
    drop(foreign);
}

#[test]
fn pair_exact_and_one_short_funding_preserves_installed_lease() {
    if isolated("pair_exact_and_one_short_funding_preserves_installed_lease") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    let lease_q = quote.lease_acquisition_quota().unwrap();
    let token_q = quote.token_acquisition_quota().unwrap();
    let work = CompilerModuleHandoffCustodyResourcesV5::PREPARE_WORK
        + WORK
        + lease_q.work()
        + token_q.work();
    // Tight phase overlap for this two-constructor callback, NOT the larger
    // conservative allowance (which also permits a scoped revalidation).
    let scratch = quote.retained_storage()
        + FRAME
        + lease_q
            .scratch()
            .max(quote.lease_storage().retained_storage() + token_q.scratch())
            .max(quote.retained_storage());
    let floor = EXTERNAL + quote.retained_storage();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    for short in [None, Some(false), Some(true)] {
        let mut account = Owned::new(
            Work::new(work - usize::from(short == Some(false))),
            floor + scratch - usize::from(short == Some(true)),
        );
        account.with_budget(|b| {
            b.reserve_storage(floor).unwrap();
            let mut resources = CompilerModuleHandoffCustodyResourcesV5::prepare(quote, b).unwrap();
            let mut lease = None;
            let mut token = None;
            let result = resources.with_acquisition(&barrier, b, |scope| {
                let (owner, charge) = scope.acquire_lease(&f.path, &f.producer)?;
                lease = Some(owner);
                scope.reserve_retained(charge)?;
                let (owner, charge) = scope.acquire_token(lease.as_ref().unwrap())?;
                token = Some(owner);
                scope.reserve_retained(charge)
            });
            match short {
                None => {
                    result.unwrap();
                    assert_eq!(b.work(), work);
                    assert_eq!(b.peak_storage(), floor + scratch);
                }
                Some(false) => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                Some(true) => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
            assert!(lease.is_some());
            assert_eq!(token.is_some(), short.is_none());
            assert_eq!(b.storage(), floor);
            locked(&f, token.is_some());
            assert!(
                resources
                    .with_acquisition::<()>(&barrier, b, |_| panic!("retry after short funding"))
                    .is_err()
            );
            drop((lease, token));
            locked(&f, false);
            assert_eq!(b.storage(), floor);
        });
    }
}

#[test]
fn installed_pair_survives_refusal_unwind_and_exhausted_work() {
    if isolated("installed_pair_survives_refusal_unwind_and_exhausted_work") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    for exit in 0..3 {
        let mut account = Owned::new(Work::new(usize::MAX), EXTERNAL + LIMIT);
        account.with_budget(|b| {
            let floor = EXTERNAL + quote.retained_storage();
            b.reserve_storage(floor).unwrap();
            let mut resources = CompilerModuleHandoffCustodyResourcesV5::prepare(quote, b).unwrap();
            let mut lease = None;
            let mut token = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                resources.with_acquisition(&barrier, b, |scope| {
                    let (owner, charge) = scope.acquire_lease(&f.path, &f.producer)?;
                    lease = Some(owner);
                    scope.reserve_retained(charge)?;
                    if exit == 2 {
                        scope.budget.charge_work(usize::MAX - scope.budget.work())?;
                        assert!(matches!(
                            scope.acquire_token(lease.as_ref().unwrap()),
                            Err(Error::Resource(Resource::Work(_)))
                        ));
                        assert!(matches!(
                            scope.acquire_token(lease.as_ref().unwrap()),
                            Err(Error::Resource(Resource::Accounting))
                        ));
                        return Err(Error::Busy);
                    }
                    let (owner, charge) = scope.acquire_token(lease.as_ref().unwrap())?;
                    token = Some(owner);
                    scope.reserve_retained(charge)?;
                    if exit == 1 {
                        panic!("installed pair unwind");
                    }
                    Err::<(), _>(Error::Busy)
                })
            }));
            if exit == 1 {
                assert!(result.is_err());
            } else {
                assert!(matches!(result, Ok(Err(Error::Busy))));
            }
            assert_eq!(b.storage(), floor);
            assert!(lease.is_some());
            locked(&f, exit != 2);
            assert!(
                resources
                    .with_acquisition::<()>(&barrier, b, |_| panic!("retry"))
                    .is_err()
            );
            // Independent custody is never refunded or removed by the failed operation.
            drop((lease, token));
            locked(&f, false);
            assert_eq!(b.storage(), floor);
            b.release_storage(floor).unwrap();
            b.reserve_storage(EXTERNAL + LIMIT).unwrap();
        });
    }
}

#[test]
fn constructor_refusal_cannot_retry_or_rebase_allowance() {
    if isolated("constructor_refusal_cannot_retry_or_rebase_allowance") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    let mut account = Owned::new(Work::new(usize::MAX), LIMIT);
    account.with_budget(|b| {
        b.reserve_storage(quote.retained_storage()).unwrap();
        let mut resources = CompilerModuleHandoffCustodyResourcesV5::prepare(quote, b).unwrap();
        resources
            .with_acquisition(&barrier, b, |scope| {
                let mut producer = f.producer.clone();
                producer.crate_name.reserve(4096);
                assert!(scope.acquire_lease(&f.path, &producer).is_err());
                assert!(matches!(
                    scope.acquire_lease(&f.path, &f.producer),
                    Err(Error::Resource(Resource::Accounting))
                ));
                Ok(())
            })
            .unwrap();
        locked(&f, false);
        assert!(
            resources
                .with_acquisition::<()>(&barrier, b, |_| panic!("retry"))
                .is_err()
        );
    });
    assert!(matches!(
        checked_allowance(&[usize::MAX, 1]),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        checked_allowance(&[LIMIT, 1]),
        Err(Error::Resource(Resource::Accounting))
    ));
    // An external backing larger than a 256 MiB ORIGINAL account still refuses.
    let mut small = Owned::new(Work::new(usize::MAX), LIMIT);
    small.with_budget(|b| assert!(b.reserve_storage(EXTERNAL).is_err()));
}
