//! Invoked only in a fresh, locked, distinct-UID supervisor subprocess.
use super::*;
use crate::authority_v2_test_process::receive_sized_packet;
use crate::native_consuming_test_process::{Case, LIFECYCLE_TIMEOUT, MeasuredImage};
use crate::{
    MAX_PROTECTED_ISSUER_PROCESSES_V1 as CAPACITY, ProtectedIssuerBoundaryV2 as Boundary,
    ProtectedIssuerCleanupErrorV2 as CleanupError, ProtectedIssuerCleanupServiceV2 as Cleanup,
    ProtectedIssuerLaunchErrorV2 as LaunchError, ProtectedIssuerTerminationV1 as Termination,
    ProtectedIssuerWaitV2 as Wait,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES;
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
use fe2o3_protected_service_profile::{
    ProtectedServiceProcessProfileV2 as Profile, require_owned_sigchld_v2,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

const REQUEST_WORK: usize = 1_000_000_000_000;
const REQUEST_STORAGE: usize = 2 * 1024 * 1024 * 1024;
const REQUEST_PREFIX: usize = 37;
const SERVICE_WORK: usize = 10_000_000;
const SERVICE_PREFIX: usize = 53;
const CLEANUP_TURNS: usize = 4096;
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("fe2o3-native-consuming-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        // The inert fixture issuer must never create durable service state.
        if let Err(error) = std::fs::remove_dir(&self.0) {
            if std::thread::panicking() {
                eprintln!("native fixture root cleanup failed: {error}");
            } else {
                panic!("native fixture root is not empty or cannot be removed: {error}");
            }
        }
    }
}

pub(crate) fn exercise(case: Case, peer: &OwnedFd, pidfd: &OwnedFd, submitter: &OwnedFd) {
    let initial_fds = fd_inventory();
    let mut work = Work::new(REQUEST_WORK);
    {
        let mut budget = Budget::new(&mut work, REQUEST_STORAGE);
        budget.charge_work(REQUEST_PREFIX).unwrap();
        budget.reserve_storage(EXTRA).unwrap();
        let ledger = budget.work_ledger_identity_v1();

        let credentials = crate::IssuerServiceCredentialProfileV1::new(65_533, 65_533).unwrap();
        let profile_storage = {
            let (profile, delta) = Profile::capture(credentials, &mut budget).unwrap();
            budget.reserve_storage(delta.additional_storage()).unwrap();
            profile.revalidate_current(&mut budget).unwrap();
            require_owned_sigchld_v2(&mut budget).unwrap();
            delta.additional_storage()
        };
        budget.release_storage(profile_storage).unwrap();
        assert_eq!(budget.storage(), EXTRA);

        let root = Root::new();
        let supervisor = crate::authority_v2::tests::bound_consuming_fixture(
            MeasuredImage::from_env("FE2O3_STATIC_PREEXEC_LAUNCHER"),
            MeasuredImage::from_env(case.image_env()),
            &root.0,
            peer,
            pidfd,
            &mut budget,
        );
        let supervisor_storage = supervisor.retained_storage();
        let request_floor = EXTRA + supervisor_storage;
        assert_eq!(budget.storage(), request_floor);
        let request_fds = fd_inventory();
        let anchor_pid = supervisor.external_anchor_process().pid();
        let anchor_pidfds = pidfd_references(anchor_pid);
        send_packet(submitter, &frame(b"ANC2", anchor_pid), &[]).unwrap();

        let mut service_work = Work::new(SERVICE_WORK);
        service_work.charge_work(SERVICE_PREFIX).unwrap();
        let mut cleanup = Cleanup::admit(Account::new(service_work, Cleanup::STORAGE)).unwrap();
        let service_before = cleanup.report().unwrap();
        assert_eq!(
            service_before.work,
            SERVICE_PREFIX + Cleanup::ADMISSION_WORK
        );
        assert_eq!(service_before.storage, Cleanup::STORAGE);
        assert_eq!(service_before.failed_work, None);

        let (accepted, handoff, client_pid, client_pidfds) =
            accept(&supervisor, submitter, &mut budget);
        let expected_manifest = accepted.manifest().identity();
        let expected_policy = supervisor.policy().identity();
        let consumed = accepted.retained_storage();
        let before_prepare = budget.work();
        let (prepared, delta) = supervisor.prepare_launch(accepted, &mut budget).unwrap();
        budget.reserve_storage(delta.additional_storage()).unwrap();
        assert_eq!(
            prepared.retained_storage(),
            consumed + delta.additional_storage()
        );
        assert_eq!(
            budget.storage(),
            request_floor + prepared.retained_storage()
        );
        assert_eq!(prepared.service_manifest().identity(), expected_manifest);
        assert_eq!(pidfd_references(client_pid), client_pidfds + 2);
        assert_eq!(pidfd_references(anchor_pid), anchor_pidfds + 1);
        assert!(budget.work() > before_prepare);
        prepared.revalidate(&supervisor, &mut budget).unwrap();

        // inspect_prepared writes probes into all pipes; never use it here. A
        // readiness reader pin observes queued byte counts without creating any
        // writer alias or removing the child-produced frame from the native reader.
        let witnesses = prepared_witnesses_with_readiness(&prepared, false);
        let readiness_pipe = Witness::new(&prepared.readiness_reader, 2);
        let before_launch = budget.work();
        assert!(budget.work_ledger_identity_v1() == ledger);
        let limits = Wait::new(Wait::MAX_ATTEMPTS, LIFECYCLE_TIMEOUT).unwrap();
        let launch_started = Instant::now();
        let mut launched = match supervisor.launch(prepared, &mut cleanup, limits, &mut budget) {
            Ok(launched) => launched,
            Err(error) => {
                // Preserve a prerequisite refusal as a failure, after draining the
                // same admitted pool. In particular, no relaxed-profile retry.
                let account = drain_cleanup(&mut cleanup);
                eprintln!(
                    "native launch refusal after {:?}: {error:?}; cleanup work={}",
                    launch_started.elapsed(),
                    account.work()
                );
                panic!("native consuming launch prerequisite failed: {error:?}");
            }
        };
        eprintln!(
            "native gated launch completed in {:?}",
            launch_started.elapsed()
        );
        let issuer_pid = launched.pid();
        assert!(![0, std::process::id(), anchor_pid, client_pid].contains(&issuer_pid));
        assert!(launched.is_live().unwrap());
        let launched_storage = launched.retained_storage();

        match case {
            Case::Ready => {
                queued_record(&readiness_pipe.pin, READY_BYTES);
                let mut ready = launched.await_readiness(limits).unwrap();
                assert_eq!(ready.pid(), issuer_pid);
                assert_eq!(ready.readiness().issuer_pid(), issuer_pid);
                assert_eq!(
                    ready.readiness().launch_manifest_identity(),
                    expected_manifest
                );
                assert_eq!(ready.readiness().policy_identity(), expected_policy);
                assert!(ready.retained_storage() >= launched_storage);
                ready.revalidate().unwrap();
                let expected_bytes = *ready.readiness().canonical_bytes();
                let ready_identity = ready.readiness().identity();
                let ready_storage = ready.retained_storage();
                let mut serving = ready.publish_readiness(limits).unwrap();
                assert_eq!(serving.pid(), issuer_pid);
                assert_eq!(serving.retained_storage(), ready_storage);
                assert_eq!(serving.readiness().canonical_bytes(), &expected_bytes);
                serving.revalidate().unwrap();
                send_packet(submitter, &frame(b"PUB2", issuer_pid), &[]).unwrap();
                let (published, []) =
                    receive_sized_packet::<READY_BYTES, 0>(submitter, Instant::now() + IO_TIMEOUT)
                        .unwrap();
                assert_eq!(published, expected_bytes);

                // The client sends the agreed inert 0x01 byte on its existing
                // service endpoint only after publication has been verified.
                send_packet(submitter, &frame(b"FIN2", issuer_pid), &[]).unwrap();
                let (completed, []) =
                    receive_packet::<0>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
                assert_eq!(completed, frame(b"FIN2", issuer_pid));
                let exited = serving.wait_for_exit(limits).unwrap();
                assert_eq!(exited.pid(), issuer_pid);
                assert_eq!(exited.termination(), Termination::Exited { status: 0 });
                assert_eq!(exited.readiness().canonical_bytes(), &expected_bytes);
                assert_eq!(exited.retained_storage(), ready_storage);
                eprintln!(
                    "native ready pid={issuer_pid} manifest={expected_manifest:?} policy={expected_policy:?} \
                 readiness={ready_identity:?}; publication verified; terminal={:?}",
                    exited.termination(),
                );
                // Exited still borrows the original ledger; only its final Drop
                // permits inspection or retirement of the remaining supervisor.
                drop(exited);
            }
            Case::MissingEof => {
                queued_record(&readiness_pipe.pin, READY_BYTES);
                let bounded = Wait::new(256, Duration::from_millis(200)).unwrap();
                let error = launched
                    .await_readiness(bounded)
                    .err()
                    .expect("missing EOF must not produce a ready owner");
                assert!(
                    matches!(
                        error,
                        LaunchError::Timeout(Boundary::Readiness)
                            | LaunchError::Attempts(Boundary::Readiness)
                    ),
                    "missing EOF must refuse at the readiness bound: {error:?}"
                );
                eprintln!("native missing EOF pid={issuer_pid}: {error:?}; cleanup entered");
            }
            Case::Trailing => {
                queued_record(&readiness_pipe.pin, READY_BYTES + 1);
                let error = launched
                    .await_readiness(limits)
                    .err()
                    .expect("trailing bytes must not produce a ready owner");
                assert!(
                    matches!(
                        error,
                        LaunchError::State("native readiness has trailing bytes")
                    ),
                    "expected trailing-data refusal: {error:?}"
                );
                eprintln!("native trailing data pid={issuer_pid}: {error:?}; cleanup entered");
            }
            Case::DropBeforeReady => {
                assert_eq!(rustix::io::ioctl_fionread(&readiness_pipe.pin).unwrap(), 0);
                drop(launched);
                eprintln!(
                    "native silent pid={issuer_pid}: launched owner dropped before readiness"
                );
            }
        }

        assert_eq!(budget.storage(), request_floor);
        assert!(budget.work() > before_launch);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        let service_account = drain_cleanup(&mut cleanup);
        assert!(service_account.work() >= service_before.work);
        let pid = rustix::process::Pid::from_raw(i32::try_from(issuer_pid).unwrap()).unwrap();
        // Only after native lifecycle/pool retirement may a non-owning probe check
        // ECHILD. No helper competes with the native owner for its terminal wait.
        assert!(matches!(
            rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG),
            Err(rustix::io::Errno::CHILD)
        ));
        assert_eq!(pidfd_references(issuer_pid), 0);
        for witness in witnesses {
            witness.assert_released();
        }
        readiness_pipe.assert_released();
        handoff.assert_released();
        drop((readiness_pipe, handoff));
        assert_eq!(pidfd_references(client_pid), client_pidfds);
        assert_eq!(pidfd_references(anchor_pid), anchor_pidfds);
        assert_eq!(fd_inventory(), request_fds);
        assert_eq!(budget.storage(), request_floor);
        drop(supervisor);
        budget.release_storage(supervisor_storage).unwrap();
        assert_eq!(budget.storage(), EXTRA);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(fd_inventory(), initial_fds);
        eprintln!(
            "native consuming {case:?}: request work={} prefix={REQUEST_PREFIX} storage={} \
         cleanup work={} storage={} fd baseline restored; issuer reaped",
            budget.work(),
            budget.storage(),
            service_account.work(),
            service_account.storage(),
        );
        drop(root);
    }
    assert_eq!(work.failed_work(), None);
}

fn queued_record(reader: &OwnedFd, expected: usize) {
    let deadline = Instant::now() + IO_TIMEOUT;
    for _ in 0..64 {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "child produced no readiness record");
        let timeout = Timespec {
            tv_sec: remaining.as_secs() as i64,
            tv_nsec: i64::from(remaining.subsec_nanos()),
        };
        let mut descriptors = [PollFd::new(reader, PollFlags::IN)];
        match poll(&mut descriptors, Some(&timeout)) {
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => panic!("observe child readiness pipe: {error}"),
            Ok(0) => panic!("child produced no readiness record before the deadline"),
            Ok(_) => {
                assert_eq!(
                    rustix::io::ioctl_fionread(reader).unwrap() as usize,
                    expected,
                    "exact child-produced record must be queued before consuming readiness"
                );
                return;
            }
        }
    }
    panic!("child readiness observation exhausted its finite attempts");
}

fn drain_cleanup(cleanup: &mut Cleanup) -> Account {
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    let mut previous = cleanup.report().unwrap().work;
    for turn in 0..CLEANUP_TURNS {
        assert!(
            Instant::now() < deadline,
            "native cleanup exceeded its deadline"
        );
        let report = cleanup.pump(CAPACITY).unwrap();
        assert!(report.work > previous);
        assert_eq!(report.work_limit, SERVICE_WORK);
        assert_eq!(report.storage, Cleanup::STORAGE);
        assert_eq!(report.failed_work, None);
        previous = report.work;
        match cleanup.shutdown() {
            Ok(account) => {
                assert_eq!(account.storage(), 0);
                assert_eq!(account.peak_storage(), Cleanup::STORAGE);
                assert_eq!(account.work_limit(), SERVICE_WORK);
                assert!(account.work() >= previous);
                assert_eq!(account.failed_work(), None);
                assert_eq!(account.failed_storage(), None);
                eprintln!(
                    "native cleanup shutdown after {} funded turns: work={}",
                    turn + 1,
                    account.work()
                );
                return account;
            }
            Err(CleanupError::Busy) => std::thread::yield_now(),
            Err(error) => panic!("native cleanup shutdown refused: {error:?}"),
        }
    }
    panic!("native cleanup exhausted {CLEANUP_TURNS} funded turns");
}
