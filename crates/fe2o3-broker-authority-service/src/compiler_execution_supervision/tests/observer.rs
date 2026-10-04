use super::*;
use crate::RetainedCompilerExecutionOccurrenceV1;
use std::io::{BufRead, Write};
use std::process::Stdio;

#[test]
fn standalone_observer_retains_original_client_without_service_directory() {
    let mut fixture = spawn_remote_rustc(RemoteFixtureMutation::Exact);
    let observation =
        ValidatedRemoteRustcProcessObservationV1::observe_client(fixture.client.take().unwrap())
            .unwrap();
    drop(fixture.admission.take());
    observation.revalidate().unwrap();
    assert_eq!(observation.descriptor(), &fixture.descriptor);
    assert!(fixture.shutdown().success());
    assert!(observation.revalidate().is_err());
}

#[test]
fn standalone_occurrence_rejects_changed_invocation_and_backend() {
    for mutation in [
        RemoteFixtureMutation::PublishedInvocationMismatch,
        RemoteFixtureMutation::BackendBytesMismatch,
        RemoteFixtureMutation::MissingInvocationCapability,
    ] {
        let mut fixture = spawn_remote_rustc(mutation);
        assert!(
            RetainedCompilerExecutionOccurrenceV1::observe(fixture.client.take().unwrap()).is_err()
        );
        assert!(fixture.shutdown().success());
    }
}

fn check_owned_occurrence(client_ids: Option<(u32, u32)>) {
    let mut fixture = spawn_remote_rustc_as(RemoteFixtureMutation::PublishedExact, client_ids);
    let client = fixture.client.take().unwrap();
    if let Some((uid, gid)) = client_ids {
        assert_eq!(client.client().uid(), uid);
        assert_eq!(client.client().gid(), gid);
        let metadata = fs::metadata(fixture._artifact_directory.path()).unwrap();
        assert_eq!(metadata.uid(), uid);
        assert_eq!(metadata.mode() & 0o777, 0o700);
    }
    let occurrence = RetainedCompilerExecutionOccurrenceV1::observe(client).unwrap();
    drop(fixture.admission.take());
    assert_eq!(occurrence.subject().attempt(), fixture.attempt.unwrap());
    assert_ne!(occurrence.identity(), &[0; 32]);
    occurrence.revalidate().unwrap();

    let output = fixture._artifact_directory.path().to_owned();
    let producer = fixture.producer.as_ref().unwrap().clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = begin_build_attempt(
            &output,
            &producer,
            BuildInvocation::from_bytes([0xd1; 32]),
            BuildSession::from_bytes([0xd2; 16]),
        );
        finished_tx.send(result).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        finished_rx
            .recv_timeout(Duration::from_millis(250))
            .is_err()
    );
    occurrence.revalidate().unwrap();
    assert!(fixture.shutdown().success());
    assert!(occurrence.revalidate().is_err());
    assert!(
        finished_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "failed observation must not silently release the publication lock"
    );
    drop(occurrence);
    assert_ne!(
        finished_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap(),
        fixture.attempt.unwrap()
    );
    worker.join().unwrap();
}

#[test]
fn owned_occurrence_retains_lock_after_service_drop_and_failed_revalidation() {
    check_owned_occurrence(None);
}

#[test]
#[ignore = "requires private real-root namespace, CAP_SYS_PTRACE/DAC and an owned UID1000 fixture"]
fn cross_uid_owned_occurrence() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    check_owned_occurrence(Some((1000, 1000)));
}

#[test]
#[ignore = "owned subprocess helper for the cross-UID publication-lock test"]
fn publication_lock_holder() {
    assert_eq!(rustix::process::geteuid().as_raw(), 1000);
    let root = std::env::var_os("FE2O3_OBSERVER_LOCK_ROOT").expect("private helper root");
    let attempt =
        BuildAttempt::from_env_value(&std::env::var("FE2O3_OBSERVER_LOCK_ATTEMPT").unwrap())
            .unwrap();
    let producer = ProducerIdentity::from_codegen(
        "compiler_occurrence",
        Some(std::path::Path::new("/src/compiler_occurrence.rs")),
    )
    .unwrap();
    let (_lease, token) =
        fe2o3_artifact_transaction::try_observe_compiler_module_handoff_currentness_in_slot_v3(
            std::path::Path::new(&root),
            &producer,
            attempt,
            fe2o3_artifact_transaction::CompilerModuleHandoffSlotV3::Production,
        )
        .unwrap();
    println!("OBSERVER_LOCK_READY");
    std::io::stdout().flush().unwrap();
    let mut byte = [0];
    assert_eq!(std::io::stdin().read(&mut byte).unwrap(), 1);
    token.revalidate_locked_currentness().unwrap();
}

#[test]
#[ignore = "requires private real-root namespace and two owned UID1000 processes"]
fn cross_uid_publication_lock_contention() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let mut fixture =
        spawn_remote_rustc_as(RemoteFixtureMutation::PublishedExact, Some((1000, 1000)));
    let artifact_root = File::open(fixture._artifact_directory.path()).unwrap();
    let artifact_fd = artifact_root.as_raw_fd();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "compiler_execution_supervision::tests::observer::publication_lock_holder",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_OBSERVER_LOCK_ROOT", "/proc/self/fd/198")
        .env(
            "FE2O3_OBSERVER_LOCK_ATTEMPT",
            fixture.attempt.unwrap().to_env_value(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    // SAFETY: only async-signal-safe credential syscalls execute between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(artifact_fd, 198) != 198 || libc::fcntl(198, libc::F_SETFD, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::setgroups(0, std::ptr::null()) != 0
                || libc::setresgid(1000, 1000, 1000) != 0
                || libc::setresuid(1000, 1000, 1000) != 0
            {
                return Err(io::Error::last_os_error());
            }
            let header = [0x20080522_u32, 0];
            let data = [0_u32; 6];
            if libc::syscall(libc::SYS_capset, header.as_ptr(), data.as_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = crate::test_process_execution::spawn(&mut command).unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let ready = loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).unwrap() == 0 {
            break false;
        }
        if line.contains("OBSERVER_LOCK_READY") {
            break true;
        }
    };
    if !ready {
        let status = child.wait().unwrap();
        panic!("lock helper exited before readiness: {status}");
    }
    let started = std::time::Instant::now();
    let result = RetainedCompilerExecutionOccurrenceV1::observe(fixture.client.take().unwrap());
    child.stdin.take().unwrap().write_all(&[1]).unwrap();
    assert!(child.wait().unwrap().success());
    assert!(
        matches!(
            result,
            Err(
                crate::ProtectedCompilerExecutionOccurrenceErrorV1::RecoverPublication(
                    fe2o3_artifact_transaction::CompilerModuleHandoffErrorV3::Busy
                )
            )
        ),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(fixture.shutdown().success());
}
