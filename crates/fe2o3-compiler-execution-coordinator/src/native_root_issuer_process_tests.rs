// Run the exact ignored matrix in a disposable root container, never on a host.
// Static fixtures: ISSUER_V3 is fe2o3-compiler-execution-issuer-conditional
// (the binary named -native is V2); ANCHOR_HELPER_V3 and ANCHOR_DAEMON_V3 are
// fe2o3-external-anchor-provisioning-helper-v3 and fe2o3-external-anchor-service-v3.
// All use the FE2O3_NATIVE_ROOT_ prefix. FE2O3_NATIVE_COMPILER_EXEC_FIXTURE is
// the existing static exec-stop fixture. The admitted daemon image is pinned
// as the unused supervisor; the distinct helper is the unused launcher. Neither
// supervisor nor launcher executes in this startup-only test. Issuer runtime
// uses the canonical sealed-static measurement. No compiler instruction resumes.
// Require explicit FE2O3_RUN_NATIVE_ROOT_ISSUER=1, FE2O3_NATIVE_ROOT_SCRATCH=/tmp,
// FE2O3_NATIVE_ROOT_RUNTIME=/run/fe2o3, private writable mounts at both paths,
// procfs, clone3/ptrace and CHOWN/FOWNER/SETUID/SETGID/SETPCAP/SYS_PTRACE/KILL, plus
// DAC_READ_SEARCH (or DAC_OVERRIDE) for real root-bound lifecycle revalidation.
// Use read-only fixture/root mounts, no network/GPU, bounded memory/CPU/PIDs,
// and an outer 600-second timeout. Startup mechanics only, no production credit.
use super::super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{fs as disk, os::unix::fs::MetadataExt, process::Command};

mod fixtures {
    include!("native_root_issuer_process_fixtures_tests.rs");
}
mod preparation {
    include!("native_root_issuer_process_preparation_tests.rs");
}
mod compiler {
    include!("native_root_issuer_process_compiler_tests.rs");
}
mod cleanup {
    include!("native_root_issuer_process_cleanup_tests.rs");
}
mod continuity {
    include!("native_root_issuer_process_continuity_tests.rs");
}

// Logical ceiling only; each launch is narrowed to its checked image-sized quote.
const WORK: usize = 1 << 60;
const STORAGE: usize = 1024 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
const TURNS: usize = 2048;
const CASE_ENV: &str = "FE2O3_NATIVE_ROOT_ISSUER_CASE";
const CASES: [&str; 10] = [
    "ready-cancel",
    "ready-unwind",
    "ready-image-mismatch",
    "ready-issuer-exit",
    "ready-compiler-cancel",
    "same-uid",
    "corrupt-state",
    "zero-timeout",
    "short-work",
    "short-storage",
];

#[test]
#[ignore = "opt-in isolated root; actual static V3 issuer and anchor binaries required"]
fn native_root_issuer_startup_matrix() {
    fixtures::require_environment();
    let helper = test_name("native_root_issuer_startup_case");
    for case in CASES {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &helper,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CASE_ENV, case)
            .output()
            .unwrap();
        require_output(output, &format!("NATIVE_ROOT_ISSUER_V3_OK case={case} "));
    }
}

fn test_name(role: &str) -> String {
    // libtest omits the crate name in its exact test filter.
    format!("{}::{role}", module_path!().split_once("::").unwrap().1)
}

fn require_output(output: std::process::Output, marker: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprint!("{stdout}{stderr}");
    assert!(
        output.status.success(),
        "subprocess {marker}: {}",
        output.status
    );
    assert!(
        stderr.lines().any(|line| line.starts_with(marker)),
        "subprocess did not execute the required case: {marker}"
    );
}

#[test]
#[ignore = "subprocess role; invoke native_root_issuer_startup_matrix instead"]
fn native_root_issuer_startup_case() {
    fixtures::require_environment();
    let case = std::env::var(CASE_ENV).expect("matrix subprocess case");
    assert!(CASES.contains(&case.as_str()));
    run(&case);
}

#[test]
#[ignore = "UID-dropped state observation subprocess; invoked only by the matrix"]
fn native_root_issuer_state_probe() {
    fixtures::probe_state();
}

#[allow(unsafe_code)]
fn run(case: &str) {
    let mut f = fixtures::Fixture::new(case == "corrupt-state");
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let mut foreign = Budget::new(&mut foreign_work, STORAGE);
    let fd_count = || disk::read_dir("/proc/self/fd").unwrap().count();
    let original_fds = fd_count();
    // Drop order keeps the creator, original Work/Budget and paths alive while
    // emergency cleanup drains, including when any assertion unwinds.
    let mut cleanup = cleanup::Drain::new();
    let prepared = preparation::prepare(&mut f, &mut cleanup.pool, &mut b);
    let prepared_storage = prepared.retained_storage();
    let expected_policy = prepared.trust.policy().policy().identity();
    let client_uid = if case == "same-uid" {
        fixtures::ISSUER
    } else {
        fixtures::CLIENT
    };
    let (mut trace, exit, drops) = compiler::confirmed(&f, client_uid, &mut cleanup.pool, &mut b);
    let trace_storage = trace.retained_storage();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    let quota = prepared.issuer_launch_quota(&trace).unwrap();
    let cleanup_quota = prepared
        .issuer_cleanup_quota(&trace, 2 * TURNS + 2)
        .unwrap();
    cleanup.check_capacity(cleanup_quota);
    let continuity = prepared
        .issuer_continuity_quota::<compiler::Backing>()
        .unwrap();
    // Limit this invocation on its ORIGINAL account. Scalars reserve capacity;
    // no large backing allocation or replacement ledger is used for ballast.
    let allowance = if case == "short-work" {
        LOCAL_WORK - 1
    } else {
        quota
            .work()
            .checked_add(4 * continuity.work())
            .unwrap()
            .checked_add(continuity::additional_work(case, continuity))
            .unwrap()
    };
    b.charge_work(
        WORK.checked_sub(b.work() + allowance)
            .expect("request work cap"),
    )
    .unwrap();
    let scratch = if case == "short-storage" {
        FRAME - 1
    } else {
        quota.scratch()
    };
    let padding = STORAGE
        .checked_sub(b.storage() + scratch)
        .expect("request storage cap");
    b.reserve_storage(padding).unwrap();
    let floor = b.storage();
    let before_work = b.work();
    let before_peak = b.peak_storage();
    // SAFETY: genuine Prepared and original confirmed, held trace; no compiler
    // resume, PID reopen or foreign wait consumer. The original funded pool owns
    // the matching lifecycle guard. Drain keeps this creator alive through reap.
    let launched = unsafe {
        prepared.launch_issuer(
            &mut trace,
            if case == "zero-timeout" {
                Duration::ZERO
            } else {
                TIMEOUT
            },
            &mut cleanup.pool,
            &mut b,
        )
    };
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(&b as *const Budget<'_> as usize, address);
    assert!(b.work() - before_work <= quota.work());
    let launch_work = b.work() - before_work;
    assert!(b.peak_storage() <= before_peak.max(floor + quota.scratch()));
    b.release_storage(padding).unwrap();
    if case.starts_with("ready-") {
        let (issuer, growth) =
            launched.expect("actual V3 issuer startup through Ready120 + EOF and root challenge");
        b.reserve_storage(growth.additional_storage()).unwrap();
        assert_eq!(
            issuer.retained_storage(),
            prepared_storage + growth.additional_storage()
        );
        assert_eq!(issuer.readiness().canonical_bytes().len(), 120);
        assert_eq!(
            issuer.readiness().issuer_pid(),
            issuer.pid().as_raw_pid() as u32
        );
        assert_eq!(issuer.readiness().policy_identity(), expected_policy);
        f.assert_state("ready");
        let live = b.storage();
        let used = b.work();
        issuer.validate_ready(&trace, &mut b).unwrap();
        assert_eq!(b.storage(), live);
        assert!(b.work() - used <= issuer.continuity_quota().work());
        if case == "ready-image-mismatch" {
            issuer
                .child
                .with_resources(&mut b, |p, b| -> Result<()> {
                    use fe2o3_broker_authority_service::IssuerAdmissionErrorKindV1 as Kind;
                    let policy = p.prepared.trust.policy().policy();
                    let mut digest = policy.executable().sha256();
                    digest[0] ^= 1;
                    let (wrong, charge) = Policy::new(
                        policy.generation(),
                        Measurement::new(digest, policy.executable().byte_len()).unwrap(),
                        policy.runtime(),
                        *policy.verifying_key(),
                        *policy.external_anchor_verifying_key(),
                        b,
                    )
                    .unwrap();
                    b.reserve_storage(charge.additional_storage())?;
                    let error = fe2o3_broker_authority_service::validate_retained_issuer_image_v3(
                        &issuer.child,
                        &wrong,
                        b,
                    )
                    .unwrap_err();
                    assert_eq!(error.kind(), Some(Kind::ExecutablePolicyMismatch));
                    Ok(())
                })
                .unwrap();
            assert_eq!(b.storage(), live);
        }
        foreign.reserve_storage(live).unwrap();
        assert!(matches!(
            issuer.validate_ready(&trace, &mut foreign),
            Err(Error::Resource(Resource::Accounting))
        ));
        // Same ledger at another address and a foreign ledger at the original
        // address must both refuse while the actual child remains live.
        std::mem::swap(&mut b, &mut foreign);
        assert!(matches!(
            issuer.validate_ready(&trace, &mut b),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert!(matches!(
            issuer.validate_ready(&trace, &mut foreign),
            Err(Error::Resource(Resource::Accounting))
        ));
        std::mem::swap(&mut b, &mut foreign);
        issuer.validate_ready(&trace, &mut b).unwrap();
        assert!(
            trace.poll(&mut b).unwrap().is_exec(),
            "startup must leave compiler held"
        );
        let before_lifetime_work = b.work();
        match case {
            "ready-issuer-exit" => continuity::issuer_exit(&issuer, &mut trace, &mut b),
            "ready-compiler-cancel" => continuity::stop_issuer(&issuer, &trace, &mut b),
            _ => {}
        }
        if matches!(case, "ready-issuer-exit" | "ready-compiler-cancel") {
            assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
        let issuer_storage = issuer.retained_storage();
        assert_ne!(trace.cancel(), CleanupPoll::Quarantined);
        if case == "ready-compiler-cancel" {
            continuity::compiler_cancelled(&issuer, &trace, &mut b);
            assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
        cleanup::wait_exit(&mut cleanup.pool, exit.as_fd());
        if case == "ready-compiler-cancel" {
            continuity::compiler_cancelled(&issuer, &trace, &mut b);
            assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
        if matches!(case, "ready-issuer-exit" | "ready-compiler-cancel") {
            assert_eq!(b.storage(), live);
            assert!(
                b.work() - before_lifetime_work <= continuity::additional_work(case, continuity)
            );
        }
        drop(trace);
        b.release_storage(trace_storage).unwrap();
        assert_eq!(
            drops.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "issuer must retain actual compiler backing after compiler reap"
        );
        if case == "ready-unwind" {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _issuer = issuer;
                panic!("intentional foreground issuer unwind");
            }));
            assert!(result.is_err());
        } else {
            assert_ne!(issuer.cancel(), CleanupPoll::Quarantined);
        }
        b.release_storage(issuer_storage).unwrap();
    } else {
        let error = launched.unwrap_err();
        match case {
            "short-work" => {
                assert!(
                    matches!(error, Error::Resource(Resource::Work(_))),
                    "{error:?}"
                );
                assert_eq!(b.failed_work(), Some(WORK + 1));
            }
            "short-storage" => {
                assert!(
                    matches!(error, Error::Resource(Resource::Storage(_))),
                    "{error:?}"
                );
                assert_eq!(b.failed_storage(), Some(STORAGE + 1));
            }
            "zero-timeout" => assert!(
                matches!(error, Error::Transport(launch_io::Failure::InvalidTimeout)),
                "{error:?}"
            ),
            _ => assert!(
                matches!(
                    error,
                    Error::Transport(
                        launch_io::Failure::MalformedReadyTransfer
                            | launch_io::Failure::ChildExited(_)
                    )
                ),
                "issuer admission must refuse before readiness, got {error:?}"
            ),
        }
        // Verify launch refusal itself cancels the original compiler BEFORE
        // explicit cancellation or dropping its foreground trace.
        cleanup::wait_exit(&mut cleanup.pool, exit.as_fd());
        drop(trace);
        b.release_storage(trace_storage + prepared_storage).unwrap();
        if case == "corrupt-state" {
            f.assert_state("corrupt");
        } else {
            f.assert_state("absent");
        }
    }
    drop(exit);
    b.release_storage(FILE_STORAGE).unwrap();
    cleanup.finish();
    assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(b.storage(), 0, "all foreground ownership charges retired");
    assert_eq!(
        fd_count(),
        original_fds,
        "native startup leaked descriptors"
    );
    f.assert_unlocked();
    eprintln!(
        "NATIVE_ROOT_ISSUER_V3_OK case={case} launch_work={launch_work}/{} peak={}",
        quota.work(),
        b.peak_storage()
    );
}
