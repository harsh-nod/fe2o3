use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MODE: &str = "FE2O3_PRIVATE_CREATOR_SCOPE_TEST";
const WITNESS: &str = "FE2O3_PRIVATE_CREATOR_SCOPE_WITNESS";

#[test]
fn original_pool_shutdown_is_the_only_nonfatal_retirement() {
    for (mode, code, witness) in [
        ("empty", 0, "complete"),
        ("busy_then_empty", 0, "complete"),
        ("drop", 125, "armed"),
        ("unwind", 125, "armed"),
        ("busy", 125, "armed"),
        ("exhausted", 125, "armed"),
        ("other_pool", 125, "armed"),
    ] {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "creator_scope::tests::creator_scope_subprocess",
                "--nocapture",
            ])
            .env_clear()
            .env(MODE, mode)
            .env(WITNESS, file.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("creator scope subprocess timed out: {mode}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(file.path()).unwrap(),
            witness,
            "{mode}"
        );
    }
}

fn witness(value: &str) {
    std::fs::write(std::env::var_os(WITNESS).unwrap(), value).unwrap();
}

#[test]
#[allow(unsafe_code)]
fn creator_scope_subprocess() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    // Only private controller mechanics run here. No root identity, executable,
    // child, namespace, service-manager unit, or deployment authority is admitted.
    let prefix = 17;
    let limit = prefix
        + if mode == "exhausted" {
            Cleanup::ADMISSION_WORK + Cleanup::pump_work(1).unwrap()
        } else {
            100_000_000
        };
    let peak = Cleanup::STORAGE + 13;
    let mut cleanup_work = Work::new(limit);
    cleanup_work.charge_work(prefix).unwrap();
    assert_eq!(
        cleanup_work.charge_work(limit).unwrap_err().actual(),
        prefix + limit
    );
    let mut account = Account::new(cleanup_work, peak);
    // Borrowed ledger identity cannot survive an Account move. Seed distinctive
    // accepted/failed history instead, while leaving admission's live floor zero.
    account.with_budget(|b| {
        b.reserve_storage(peak).unwrap();
        b.release_storage(peak).unwrap();
        assert!(b.reserve_storage(peak + 1).is_err());
    });
    let cleanup = crate::process_reaper::isolated_cleanup(account);
    let mut work = Work::new(100_000_000);
    let mut request = Budget::new(&mut work, 0);
    request
        .charge_work(DedicatedCreatorScopeV1::CONTROL_WORK)
        .unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: this explicitly selected disposable subprocess owns only these
        // mechanical pools. The guard stays on this test thread through shutdown.
        let mut scope = unsafe { DedicatedCreatorScopeV1::enter(cleanup) };
        if matches!(mode.as_str(), "busy" | "busy_then_empty" | "exhausted") {
            // SAFETY: borrow only to reserve unused capacity; never replace or
            // independently shut down the controller. There is no native child.
            let reservation = unsafe { scope.cleanup_for_launch() }
                .reserve_launch(&mut request)
                .unwrap();
            assert!(matches!(scope.shutdown(), Err(Failure::Busy)));
            if mode == "busy_then_empty" {
                drop(reservation);
            } else {
                if mode == "exhausted" {
                    scope.pump(1).unwrap();
                    assert!(matches!(scope.shutdown(), Err(Failure::Resource(_))));
                }
                witness("armed");
                drop(scope);
                witness("escaped");
                drop(reservation);
                return;
            }
        }
        if mode == "other_pool" {
            let mut other = crate::process_reaper::isolated_cleanup(Account::new(
                Work::new(Cleanup::ADMISSION_WORK),
                Cleanup::STORAGE,
            ));
            other.shutdown().unwrap();
        }
        match mode.as_str() {
            "empty" | "busy_then_empty" => {
                let returned = scope.shutdown().unwrap();
                let repeated_shutdown = if mode == "busy_then_empty" {
                    Cleanup::shutdown_work()
                } else {
                    0
                };
                assert_eq!(returned.work_limit(), limit);
                assert_eq!(
                    returned.work(),
                    prefix + Cleanup::ADMISSION_WORK + repeated_shutdown
                );
                assert_eq!(returned.failed_work(), Some(prefix + limit));
                assert_eq!(returned.storage_limit(), peak);
                assert_eq!((returned.storage(), returned.peak_storage()), (0, peak));
                assert_eq!(returned.failed_storage(), Some(peak + 1));
                drop(scope);
                witness("complete");
            }
            "unwind" => {
                witness("armed");
                panic!("injected creator-scope unwind");
            }
            "drop" | "other_pool" => {
                witness("armed");
                drop(scope);
                witness("escaped");
            }
            _ => panic!("unknown creator-scope mode"),
        }
    }));
    assert!(result.is_ok(), "armed scope escaped through catch_unwind");
}
