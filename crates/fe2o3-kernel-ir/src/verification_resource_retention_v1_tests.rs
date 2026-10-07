use super::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

type Owned = CanonicalKernelIrOwnedVerificationResourceBudgetV1;
type Budget<'w> = CanonicalKernelIrVerificationResourceBudgetV1<'w>;
type Work = CanonicalKernelIrWorkBudgetV1;
type Loan<'w> = CanonicalKernelIrOriginalAccountRetentionLoanV1<'w>;

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

#[test]
fn retention_is_original_account_specific_and_drop_never_refunds_storage() {
    let mut account = Owned::new(Work::new(128), 1024);
    account.with_budget(|budget| {
        let ledger = budget.work_ledger_identity_v1();
        let storage = budget.storage_account_identity_v1();
        let first = budget.retain_original_account_v1().unwrap();
        let second = budget.retain_original_account_v1().unwrap();
        assert!(std::ptr::eq(first.state, second.state));
        assert_eq!(first.state.loans.load(Ordering::Relaxed), 2);
        assert_eq!(budget.storage(), 2 * Loan::RETAINED_STORAGE);
        drop(first);
        assert_eq!(second.state.loans.load(Ordering::Relaxed), 1);
        drop(second);
        assert_eq!(budget.storage(), 2 * Loan::RETAINED_STORAGE);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage_account_identity_v1(), storage);
    });
    assert_eq!(
        (account.work(), account.storage()),
        (16, 2 * Loan::RETAINED_STORAGE)
    );
    account.with_budget(|budget| {
        let loan = budget.retain_original_account_v1().unwrap();
        assert_eq!(loan.state.loans.load(Ordering::Relaxed), 1);
        drop(loan);
    });
    assert_eq!(account.storage(), 3 * Loan::RETAINED_STORAGE);
}

#[test]
fn retention_refuses_inline_replacement_and_bounded_scratch_views() {
    let mut work = Work::new(128);
    assert!(matches!(
        Budget::new(&mut work, 1024).retain_original_account_v1(),
        Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
    ));
    let replacement_work = Box::leak(Box::new(Work::new(128)));
    let mut account = Owned::new(Work::new(1024), 8192);
    account.with_budget(|budget| {
        let result = budget
            .with_bounded_scratch_v1::<_, CanonicalKernelIrVerificationResourceErrorV1>(
                128,
                4096,
                |scratch| {
                    assert!(matches!(
                        scratch.retain_original_account_v1(),
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
                    ));
                    Ok(())
                },
            );
        result.unwrap();
        let original = std::mem::replace(budget, Budget::new(replacement_work, 1024));
        assert!(matches!(
            budget.retain_original_account_v1(),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        ));
        *budget = original;
        drop(budget.retain_original_account_v1().unwrap());
    });
}

#[test]
fn retention_count_and_resource_denials_precede_counter_changes() {
    let mut account = Owned::new(Work::new(100_000), 100_000);
    account.with_budget(|budget| {
        let state = budget.window.unwrap().retention.as_ref().unwrap();
        let mut loans = Vec::new();
        for _ in 0..Budget::MAX_ORIGINAL_ACCOUNT_RETENTION_LOANS_V1 {
            loans.push(budget.retain_original_account_v1().unwrap());
        }
        let before = budget.storage();
        assert!(matches!(
            budget.retain_original_account_v1(),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        ));
        assert_eq!(budget.storage(), before);
        assert_eq!(state.loans.load(Ordering::Relaxed), loans.len());
        drop(loans);
        assert_eq!(state.loans.load(Ordering::Relaxed), 0);
        // Private corrupted-counter control: checked overflow refuses before
        // incrementing or reserving storage, then restore this CPU fixture only.
        state.loans.store(usize::MAX, Ordering::Relaxed);
        assert!(matches!(
            budget.retain_original_account_v1(),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        ));
        assert_eq!(state.loans.load(Ordering::Relaxed), usize::MAX);
        assert_eq!(budget.storage(), before);
        state.loans.store(0, Ordering::Relaxed);
    });
    for (work, storage) in [(7, 1024), (128, Loan::RETAINED_STORAGE - 1)] {
        Owned::new(Work::new(work), storage).with_budget(|budget| {
            assert!(budget.retain_original_account_v1().is_err());
            assert_eq!(
                budget
                    .window
                    .unwrap()
                    .retention
                    .as_ref()
                    .unwrap()
                    .loans
                    .load(Ordering::Relaxed),
                0
            );
        });
    }
}

#[test]
fn retention_normal_async_cancellation_releases_loan_before_hidden_exit_guard() {
    let mut future = Box::pin(Owned::new(Work::new(128), 1024).into_budget_scope_async_v1(
        async |budget| {
            let loan = budget.retain_original_account_v1().unwrap();
            std::future::pending::<()>().await;
            drop(loan);
        },
    ));
    assert!(poll(future.as_mut()).is_pending());
    drop(future);
}

#[test]
fn retention_no_loan_paths_preserve_result_denials_and_unwind_behavior() {
    let mut account = Owned::new(Work::new(16), 16);
    let result = account.with_budget(|budget| {
        budget.charge_work(7).unwrap();
        budget.reserve_storage(9).unwrap();
        Err::<(), _>("same refusal")
    });
    assert_eq!(result, Err("same refusal"));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| account.with_budget(
            |budget| {
                assert!(budget.charge_work(10).is_err());
                panic!("original unwind");
            }
        )))
        .is_err()
    );
    assert_eq!(
        (account.work(), account.storage(), account.failed_work()),
        (7, 9, Some(17))
    );
}

struct NativeOwnerFixture<'w> {
    _loan: Loan<'w>,
}

fn forget_nested_owner(budget: &mut Budget<'_>) {
    let mut owner = Box::new(NativeOwnerFixture {
        _loan: budget.retain_original_account_v1().unwrap(),
    });
    let mut nested = Box::pin(async {
        std::hint::black_box(&mut owner);
        std::future::pending::<()>().await;
        std::hint::black_box(&mut owner);
    });
    assert!(poll(nested.as_mut()).is_pending());
    std::mem::forget(nested);
    std::mem::forget(owner);
}

#[cfg(target_os = "linux")]
#[test]
fn retention_forgotten_loans_fail_stop_on_return_error_unwind_and_nested_forgets() {
    use std::{
        os::unix::process::ExitStatusExt,
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    const ENV: &str = "FE2O3_ACCOUNT_RETENTION_REFUSAL";
    const TEST: &str = "verification_resource_v1::retention::tests::retention_forgotten_loans_fail_stop_on_return_error_unwind_and_nested_forgets";
    if let Some(mode) = std::env::var_os(ENV) {
        eprintln!("entered exact original-account retention control");
        let mut account = Owned::new(Work::new(1024), 4096);
        match mode.to_str().unwrap() {
            "return" => {
                account.with_budget(|b| std::mem::forget(b.retain_original_account_v1().unwrap()))
            }
            "error" => {
                let _ = account.with_budget(|b| {
                    std::mem::forget(b.retain_original_account_v1().unwrap());
                    Err::<(), _>("original failure")
                });
            }
            "unwind" => account.with_budget(|b| {
                std::mem::forget(b.retain_original_account_v1().unwrap());
                panic!("forgotten original loan before unwind");
            }),
            "replacement" => {
                let work = Box::leak(Box::new(Work::new(128)));
                account.with_budget(|b| {
                    std::mem::forget(b.retain_original_account_v1().unwrap());
                    *b = Budget::new(work, 1024);
                });
            }
            "sync-double" => account.with_budget(forget_nested_owner),
            "async-double" => {
                let mut future =
                    Box::pin(account.into_budget_scope_async_v1(async |b| forget_nested_owner(b)));
                let _ = poll(future.as_mut());
            }
            "async-triple" => {
                let mut future = Box::pin(account.into_budget_scope_async_v1(async |b| {
                    let mut outer = Box::pin(async {
                        forget_nested_owner(b);
                        std::future::pending::<()>().await;
                    });
                    assert!(poll(outer.as_mut()).is_pending());
                    std::mem::forget(outer);
                }));
                let _ = poll(future.as_mut());
            }
            "async-drop" => {
                let mut future = Box::pin(account.into_budget_scope_async_v1(async |b| {
                    std::mem::forget(b.retain_original_account_v1().unwrap());
                    std::future::pending::<()>().await;
                }));
                assert!(poll(future.as_mut()).is_pending());
                drop(future);
            }
            _ => panic!("unknown original-account retention control"),
        }
        panic!("original account escaped with a forgotten retention loan");
    }
    struct Reap(Option<Child>);
    impl Drop for Reap {
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    for mode in [
        "return",
        "error",
        "unwind",
        "replacement",
        "sync-double",
        "async-double",
        "async-triple",
        "async-drop",
    ] {
        // Core suppression is inherited by exec; no unsafe signal/resource code
        // or new production dependency is needed in this target-neutral crate.
        let mut child = Reap(Some(
            Command::new("/bin/sh")
                .args(["-c", "ulimit -c 0; exec \"$@\"", "retention-control"])
                .arg(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
                .env(ENV, mode)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ));
        let deadline = Instant::now() + Duration::from_secs(20);
        while child.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "retention control timed out: {mode}"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let output = child.0.take().unwrap().wait_with_output().unwrap();
        assert_eq!(
            output.status.signal(),
            Some(6),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("entered exact original-account retention control"),
            "wrong exact test filter: {mode}"
        );
    }
}
