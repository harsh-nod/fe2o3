//! Private counter/teardown controls; no admitted proof or native execution fixture.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn private_host_epoch_rejects_reentry_and_retains_original_account_until_close() {
    let anchor = EpochAnchor::new();
    let failed = Cell::new(false);
    let mut account = Owned::new(Work::new(1024), 4096);
    account.with_budget(|budget| {
        let original = budget.work_ledger_identity_v1();
        let mut epoch = anchor.begin(&failed, budget).unwrap();
        assert!(anchor.require_idle().is_err());
        assert!(anchor.begin(&failed, budget).is_err());
        let carrier = epoch.retain_carrier().unwrap();
        assert_eq!(anchor.carriers.get(), 1);
        // A close refusal alone must not release the original retention loan.
        // The original carrier can still be disposed before dropping this guard.
        assert!(anchor.active.get());
        drop(carrier);
        assert_eq!(anchor.carriers.get(), 0);
        epoch.close().unwrap();
        drop(epoch);
        anchor.require_idle().unwrap();
        assert!(!failed.get());
        assert!(budget.work_ledger_identity_v1() == original);
        assert_eq!(
            budget.storage(),
            Epoch::LOAN_STORAGE,
            "Drop must not refund"
        );
        budget.release_storage(Epoch::LOAN_STORAGE).unwrap();
    });
    assert_eq!(account.storage(), 0);
}

#[test]
fn private_host_epoch_cancel_without_carriers_poisoned_but_does_not_invent_settlement() {
    let anchor = EpochAnchor::new();
    let failed = Cell::new(false);
    let mut account = Owned::new(Work::new(1024), 4096);
    account.with_budget(|budget| {
        let epoch = anchor.begin(&failed, budget).unwrap();
        drop(epoch);
        assert!(failed.get());
        anchor.require_idle().unwrap();
        assert!(anchor.begin(&failed, budget).is_err());
        assert_eq!(budget.storage(), Epoch::LOAN_STORAGE);
    });
}

#[test]
fn private_host_carrier_bound_refuses_before_count_change() {
    let anchor = EpochAnchor::new();
    let failed = Cell::new(false);
    Owned::new(Work::new(1024), 4096).with_budget(|budget| {
        let mut epoch = anchor.begin(&failed, budget).unwrap();
        let carriers: Vec<_> = (0..fe2o3_runtime::MAX_RUNTIME_GFX942_SCOPED_SUBMISSIONS_V1)
            .map(|_| epoch.retain_carrier().unwrap())
            .collect();
        assert!(epoch.retain_carrier().is_err());
        assert_eq!(anchor.carriers.get(), carriers.len());
        drop(carriers);
        epoch.close().unwrap();
    });
}

#[test]
fn private_host_epoch_spans_await_and_closes_only_after_effect_free_carrier_disposal() {
    use std::{future::Future, task::Context};
    let anchor = EpochAnchor::new();
    let failed = Cell::new(false);
    let mut account = Owned::new(Work::new(1024), 4096);
    account.with_budget(|budget| {
        let identity = budget.work_ledger_identity_v1();
        let release = Cell::new(false);
        let mut operation = Box::pin(async {
            let mut epoch = anchor.begin(&failed, budget).unwrap();
            let carrier = epoch.retain_carrier().unwrap();
            std::future::poll_fn(|_| {
                if release.get() {
                    std::task::Poll::Ready(())
                } else {
                    std::task::Poll::Pending
                }
            })
            .await;
            // This private counter fixture has no runtime/native effects. Real
            // submitted carriers remain behind the runtime settlement guard.
            drop(carrier);
            epoch.close().unwrap();
        });
        let mut cx = Context::from_waker(std::task::Waker::noop());
        assert!(operation.as_mut().poll(&mut cx).is_pending());
        assert!(anchor.require_idle().is_err());
        assert_eq!(anchor.carriers.get(), 1);
        release.set(true);
        assert!(operation.as_mut().poll(&mut cx).is_ready());
        drop(operation);
        anchor.require_idle().unwrap();
        assert!(!failed.get());
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), Epoch::LOAN_STORAGE);
    });
}

#[test]
fn private_host_epoch_async_cancellation_releases_no_storage_and_poisoned_owner_cannot_reopen() {
    use std::{future::Future, task::Context};
    let anchor = EpochAnchor::new();
    let failed = Cell::new(false);
    Owned::new(Work::new(1024), 4096).with_budget(|budget| {
        let mut operation = Box::pin(async {
            let epoch = anchor.begin(&failed, budget).unwrap();
            let _effect_free_carrier = epoch.retain_carrier().unwrap();
            std::future::pending::<()>().await;
        });
        let mut cx = Context::from_waker(std::task::Waker::noop());
        assert!(operation.as_mut().poll(&mut cx).is_pending());
        assert_eq!(anchor.carriers.get(), 1);
        drop(operation);
        assert_eq!(anchor.carriers.get(), 0);
        assert!(failed.get());
        anchor.require_idle().unwrap();
        assert!(anchor.begin(&failed, budget).is_err());
        assert_eq!(budget.storage(), Epoch::LOAN_STORAGE);
    });
}

#[cfg(target_os = "linux")]
#[test]
fn private_host_epoch_forget_and_early_exit_abort_before_proof_or_account_teardown() {
    use std::{
        os::unix::process::ExitStatusExt,
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    const ENV: &str = "FE2O3_NATIVE_HOST_EPOCH_REFUSAL";
    const TEST: &str = "native_conditional_fill_artifact_v1::intake::lifecycle::scoped::epoch::tests::private_host_epoch_forget_and_early_exit_abort_before_proof_or_account_teardown";
    struct ProofTeardown;
    impl Drop for ProofTeardown {
        fn drop(&mut self) {
            eprintln!("FORBIDDEN proof dependency teardown");
        }
    }
    struct Owner {
        anchor: EpochAnchor,
        _proof: ProofTeardown,
    }
    if let Some(mode) = std::env::var_os(ENV) {
        eprintln!("entered exact private host epoch refusal");
        Owned::new(Work::new(1024), 4096).with_budget(|budget| {
            let owner = Box::new(Owner {
                anchor: EpochAnchor::new(),
                _proof: ProofTeardown,
            });
            let failed = Cell::new(false);
            if matches!(
                mode.to_str().unwrap(),
                "async-forget" | "async-owner-forget"
            ) {
                use std::{future::Future, task::Context};
                let mut operation = Box::pin(async {
                    let epoch = owner.anchor.begin(&failed, budget).unwrap();
                    let _carrier = epoch.retain_carrier().unwrap();
                    std::future::pending::<()>().await;
                });
                let mut cx = Context::from_waker(std::task::Waker::noop());
                assert!(operation.as_mut().poll(&mut cx).is_pending());
                std::mem::forget(operation);
                if mode == "async-owner-forget" {
                    std::mem::forget(owner);
                } else {
                    drop(owner);
                }
                return;
            }
            let mut epoch = owner.anchor.begin(&failed, budget).unwrap();
            match mode.to_str().unwrap() {
                "carrier-forget" => {
                    std::mem::forget(epoch.retain_carrier().unwrap());
                    drop(epoch);
                }
                "early-close" => {
                    std::mem::forget(epoch.retain_carrier().unwrap());
                    assert!(epoch.close().is_err());
                    drop(epoch);
                }
                "epoch-forget" => {
                    std::mem::forget(epoch);
                    drop(owner);
                }
                "double-forget" => {
                    std::mem::forget(epoch);
                    std::mem::forget(owner);
                }
                "triple-forget" => {
                    std::mem::forget(epoch.retain_carrier().unwrap());
                    std::mem::forget(epoch);
                    std::mem::forget(owner);
                }
                _ => panic!("unknown private host epoch refusal"),
            }
        });
        panic!("forgotten host epoch escaped original owner/account guards");
    }
    struct Reap(Option<Child>);
    impl Drop for Reap {
        fn drop(&mut self) {
            if let Some(mut c) = self.0.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
    for mode in [
        "carrier-forget",
        "early-close",
        "epoch-forget",
        "double-forget",
        "triple-forget",
        "async-forget",
        "async-owner-forget",
    ] {
        let mut child = Reap(Some(
            Command::new("/bin/sh")
                .args(["-c", "ulimit -c 0; exec \"$@\"", "host-epoch-control"])
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
                "host epoch child timed out: {mode}"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let output = child.0.take().unwrap().wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
        assert!(
            stderr.contains("entered exact private host epoch refusal"),
            "wrong exact filter: {mode}"
        );
        assert!(
            !stderr.contains("FORBIDDEN proof dependency teardown"),
            "{mode}: {stderr}"
        );
    }
}
