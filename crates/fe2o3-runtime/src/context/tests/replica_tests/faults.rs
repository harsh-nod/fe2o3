#![cfg(test)]

use super::super::peer_directed_tests::Observation;
use super::*;

#[test]
fn replica_definite_refusal_preserves_unrelated_settled_copy_and_account() {
    let mut f = Fixture::new(2);
    let copy = f.start(0, 1);
    let healthy = f.settle(copy);
    let usage = f.account.usage();
    f.context.backend.copy_failure = MockMemoryFailure::Rejected;
    assert!(matches!(
        f.context.submit_tracked_replica_copy_v1(
            f.streams[2],
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[2], RuntimeAccessV1::Write)
        ),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert!(!f.context.is_terminal());
    assert_eq!(f.account.usage(), usage);
    assert_eq!(
        f.context.replica_registry_usage_v1().unwrap(),
        RuntimeReplicaUsageV1 {
            capacity: 2,
            pending: 0,
            settled: 1,
        }
    );
    f.context.validate_replica_v1(healthy).unwrap();
    assert_eq!(f.read(1), [0x31; 96]);
    f.finish();
}

#[test]
fn replica_failed_original_copy_retires_without_ever_certifying_bytes() {
    let mut f = Fixture::new(2);
    let mut copy = f.start(0, 1);
    let backend = *f.context.backend.pending_copies.keys().next().unwrap();
    f.context
        .backend
        .producer_launch
        .observations
        .insert(backend, Observation::Failed);
    assert_eq!(
        f.context.poll_tracked_replica_copy_v1(&mut copy).unwrap(),
        RuntimePollV1::Failed { code: 7 }
    );
    assert_eq!(
        f.context.retire_tracked_replica_copy_v1(copy).unwrap(),
        RuntimeReplicaCopySettlementV1::SettledWithoutCurrentReplica(
            RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(7))
        )
    );
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 0);
    assert_eq!(
        f.context.backend.memory[&f.context.allocations[&f.allocations[1]].backend_allocation],
        [0x32; 96]
    );
    f.finish();
}

#[test]
fn replica_pending_writer_invalidates_source_or_destination_before_settlement() {
    for changed in [0, 1] {
        let mut f = Fixture::new(2);
        let copy = f.start(0, 1);
        let reference = f.settle(copy);
        let mut writer = f
            .context
            .peer_copy(
                f.streams[changed],
                region(f.allocations[2], RuntimeAccessV1::Read),
                region(f.allocations[changed], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        assert!(f.context.validate_replica_v1(reference).is_err());
        assert_eq!(f.context.poll(&mut writer).unwrap(), RuntimePollV1::Pending);
        assert!(f.context.validate_replica_v1(reference).is_err());
        assert_eq!(
            f.context.poll(&mut writer).unwrap(),
            RuntimePollV1::Succeeded
        );
        f.context.release_submission(writer).unwrap();
        assert!(f.context.validate_replica_v1(reference).is_err());
        f.context.forget_replica_v1(reference).unwrap();
        f.finish();
    }
}

#[test]
fn replica_observer_cannot_be_polled_or_retired_by_a_foreign_context() {
    let mut f = Fixture::new(1);
    let mut foreign = Fixture::new(1);
    let mut copy = f.start(0, 1);
    let before = foreign.account.usage();
    assert!(
        foreign
            .context
            .poll_tracked_replica_copy_v1(&mut copy)
            .is_err()
    );
    let refusal = foreign
        .context
        .retire_tracked_replica_copy_v1(copy)
        .unwrap_err();
    assert_eq!(foreign.account.usage(), before);
    assert_eq!(foreign.context.backend.poll_call_count, 0);
    assert_eq!(
        foreign.context.replica_registry_usage_v1().unwrap().pending,
        0
    );
    let reference = f.settle(refusal.copy);
    f.context.validate_replica_v1(reference).unwrap();
    foreign.finish();
    f.finish();
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn replica_unknown_and_forgotten_attempts_retain_custody_before_fail_stop() {
    use std::io::Read;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Child, Command, Stdio};
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    const ENV: &str = "FE2O3_REPLICA_FAIL_STOP_TEST";
    const TEST: &str = "context::tests::replica_tests::faults::replica_unknown_and_forgotten_attempts_retain_custody_before_fail_stop";
    const READY: &str = "REPLICA_ORIGINAL_CUSTODY_RETAINED";
    if let Some(mode) = std::env::var_os(ENV) {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mut f = Fixture::new(2);
        let account = f.account.usage();
        if mode == "forgotten" {
            let copy = f.start(0, 1);
            #[allow(
                clippy::forget_non_drop,
                reason = "prove the Context guard does not depend on an observer destructor"
            )]
            std::mem::forget(copy);
        } else if mode == "graph-terminal" || mode == "graph-panic" {
            super::graph::start_unknown(&mut f, mode == "graph-panic");
        } else if mode == "release-terminal" || mode == "release-panic" {
            let mut copy = f.start(0, 1);
            f.observe(&mut copy);
            f.context.backend.release_submission_failure = if mode == "release-terminal" {
                MockMemoryFailure::Terminal
            } else {
                MockMemoryFailure::Panic
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                f.context.retire_tracked_replica_copy_v1(copy).is_err()
            }));
            assert!(matches!(result, Ok(true)) || result.is_err());
            assert!(f.context.is_terminal());
        } else {
            f.context.backend.copy_failure = match mode.to_str().unwrap() {
                "terminal" => MockMemoryFailure::Terminal,
                "quiescent" => MockMemoryFailure::Quiescent,
                "panic" => MockMemoryFailure::Panic,
                _ => panic!("unknown subprocess mode"),
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                f.context.submit_tracked_replica_copy_v1(
                    f.streams[1],
                    region(f.allocations[0], RuntimeAccessV1::Read),
                    region(f.allocations[1], RuntimeAccessV1::Write),
                )
            }));
            assert!(matches!(result, Ok(Err(_))) || result.is_err());
            assert!(f.context.is_terminal());
        }
        assert_eq!(f.account.usage(), account);
        assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 1);
        let cleanup = f.context.backend.cleanup_log.clone();
        let report = f.context.cleanup();
        assert!(!report.is_complete());
        assert_eq!(report.pending_replica_copies_v1(), 1);
        assert_eq!(f.context.backend.cleanup_log, cleanup);
        assert_eq!(f.account.usage(), account);
        eprintln!("{READY}");
        drop(f.context);
        panic!("pending replica Context destruction returned");
    }
    for mode in [
        "forgotten",
        "terminal",
        "quiescent",
        "panic",
        "release-terminal",
        "release-panic",
        "graph-terminal",
        "graph-panic",
    ] {
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(ENV, mode)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut stderr = child.0.stderr.take().unwrap();
        let flags = rustix::fs::fcntl_getfl(&stderr).unwrap();
        rustix::fs::fcntl_setfl(&stderr, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut bytes = Vec::new();
        let (status, refused) = loop {
            let mut chunk = [0; 1024];
            match stderr.read(&mut chunk) {
                Ok(count) => bytes.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("child stderr: {error}"),
            }
            if bytes.len() > 16 * 1024 || Instant::now() >= deadline {
                child.0.kill().unwrap();
                break (child.0.wait().unwrap(), true);
            }
            if let Some(status) = child.0.try_wait().unwrap() {
                // Exited direct child has closed this sole stderr writer.
                let mut tail = [0; 1024];
                while let Ok(count) = stderr.read(&mut tail) {
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&tail[..count]);
                    if bytes.len() > 16 * 1024 {
                        break;
                    }
                }
                break (status, bytes.len() > 16 * 1024);
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let text = String::from_utf8_lossy(&bytes);
        assert!(!refused, "{mode}: watchdog/log refusal: {text}");
        assert_eq!(status.signal(), Some(6), "{mode}: {text}");
        assert!(text.contains(READY), "{mode}: {text}");
        assert!(!text.contains("destruction returned"), "{mode}: {text}");
    }
}
