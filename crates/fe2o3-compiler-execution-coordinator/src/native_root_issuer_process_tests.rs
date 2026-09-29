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
// and an outer 600-second timeout. The separate publication matrix observes an
// inert V5 fixture through that same held trace. Neither matrix grants production
// rustc, protected proof, root RPC, durable retirement or GPU qualification credit.
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
mod publication_fixture {
    include!("native_root_issuer_process_publication_fixture_tests.rs");
}
mod publication {
    include!("native_root_issuer_process_publication_tests.rs");
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

#[test]
#[ignore = "opt-in isolated root; real native trace and inert V5 publication custody, not production rustc"]
fn native_root_publication_matrix() {
    publication::matrix();
}

#[test]
#[ignore = "subprocess role; invoke native_root_publication_matrix instead"]
fn native_root_publication_case() {
    publication::case();
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
    let (trace, exit, drops) = compiler::confirmed(&f, client_uid, &mut cleanup.pool, &mut b);
    let trace_storage = trace.retained_storage();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    let quota = prepared.issuer_launch_quota(&trace).unwrap();
    let transfer = trace.issuer_inputs_quota().unwrap();
    let cleanup_quota = prepared
        .issuer_cleanup_quota(&trace, 3 * TURNS + 3)
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
            .checked_add(continuity::additional_work(case, continuity, transfer))
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
        prepared.launch_root_attempt(
            trace,
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
        let (mut attempt, growth) =
            launched.expect("actual V3 issuer startup through Ready120 + EOF and root challenge");
        b.reserve_storage(growth.additional_storage()).unwrap();
        assert_eq!(
            attempt.retained_storage(),
            prepared_storage + trace_storage + growth.additional_storage()
        );
        assert_eq!(
            attempt.retained_storage(),
            attempt_storage::<compiler::Backing>(
                attempt.trace.retained_storage(),
                attempt.root.retained_storage(),
                attempt.issuer.as_ref().unwrap().retained,
            )
            .unwrap()
        );
        assert_eq!(attempt.readiness().unwrap().canonical_bytes().len(), 120);
        assert_eq!(
            attempt.readiness().unwrap().issuer_pid(),
            attempt.issuer_pid().unwrap().as_raw_pid() as u32
        );
        assert_eq!(
            attempt.readiness().unwrap().policy_identity(),
            expected_policy
        );
        f.assert_state("ready");
        let live = b.storage();
        let used = b.work();
        attempt.validate_ready(&mut b).unwrap();
        assert_eq!(b.storage(), live);
        assert!(b.work() - used <= attempt.continuity_quota().work());
        if case == "ready-image-mismatch" {
            let issuer = attempt.issuer.as_ref().unwrap();
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
            attempt.validate_ready(&mut foreign),
            Err(Error::Resource(Resource::Accounting))
        ));
        // Same ledger at another address and a foreign ledger at the original
        // address must both refuse while the actual child remains live.
        std::mem::swap(&mut b, &mut foreign);
        assert!(matches!(
            attempt.validate_ready(&mut b),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert!(matches!(
            attempt.validate_ready(&mut foreign),
            Err(Error::Resource(Resource::Accounting))
        ));
        std::mem::swap(&mut b, &mut foreign);
        attempt.validate_ready(&mut b).unwrap();
        assert!(
            attempt.poll_compiler(&mut b).unwrap().is_exec(),
            "startup must leave compiler held"
        );
        let before_lifetime_work = b.work();
        match case {
            "ready-issuer-exit" => continuity::issuer_exit(&mut attempt, &mut b),
            "ready-compiler-cancel" => continuity::stop_issuer(&attempt, &mut b),
            _ => {}
        }
        if matches!(case, "ready-cancel" | "ready-unwind" | "ready-issuer-exit") {
            continuity::remove_issuer(
                &mut attempt,
                case == "ready-unwind",
                &mut cleanup.pool,
                &mut b,
                &mut foreign,
            );
        }
        let retained = attempt.retained_storage();
        assert_eq!(
            drops.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "original compiler backing must remain owned"
        );
        if case == "ready-unwind" {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _attempt = attempt;
                panic!("intentional foreground attempt unwind");
            }));
            assert!(result.is_err());
            cleanup::wait_exit(&mut cleanup.pool, exit.as_fd());
        } else {
            if case == "ready-compiler-cancel" {
                continuity::refuse_outer_scope(&mut attempt, &mut b);
                continuity::compiler_cancelled(&attempt, &mut b);
            } else {
                assert_ne!(attempt.cancel_compiler(), CleanupPoll::Quarantined);
            }
            cleanup::wait_exit(&mut cleanup.pool, exit.as_fd());
            if case == "ready-compiler-cancel" {
                continuity::compiler_cancelled(&attempt, &mut b);
            }
            // Private-field inspection preserves the original dependency regression;
            // no production API can extract either the compiler or the root session.
            let NativeAttempt {
                trace,
                root,
                mut issuer,
                ..
            } = attempt;
            drop(trace);
            drop(root);
            if issuer.is_some() {
                assert_eq!(
                    drops.load(std::sync::atomic::Ordering::SeqCst),
                    0,
                    "issuer must retain actual compiler backing after compiler reap"
                );
                assert_ne!(
                    cancel_issuer_slot(&mut issuer),
                    Some(CleanupPoll::Quarantined)
                );
            }
        }
        assert_eq!(b.storage(), live);
        assert!(
            b.work() - before_lifetime_work
                <= continuity::additional_work(case, continuity, transfer)
        );
        b.release_storage(retained).unwrap();
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
        // The consuming constructor must cancel/drop its compiler on refusal;
        // no loose foreground trace remains for the caller to rescue it.
        cleanup::wait_exit(&mut cleanup.pool, exit.as_fd());
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
