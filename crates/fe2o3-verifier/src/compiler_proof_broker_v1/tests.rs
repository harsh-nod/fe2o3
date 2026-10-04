use super::*;

#[test]
fn broker_owners_support_scoped_worker_threads() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<CompilerProofBrokerV1>();
    send_sync::<PendingCompilerProofServerV1>();
    send_sync::<AuthenticatedCompilerProofSessionV1>();
}

#[test]
fn original_spawned_child_handle_rejects_reaped_occurrence() {
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let mut child = crate::executor::spawn_artifact_coordinated_child(&mut command).unwrap();
    let original = capture_original_child(&child).unwrap();
    assert_eq!(inspect_pidfd_target(&original).unwrap().pid, child.id());
    require_pidfd_not_pollable(&original).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(require_pidfd_not_pollable(&original).is_err());
    assert!(
        fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1::admit_received(original, child.id())
            .is_err()
    );
}

#[test]
fn process_output_round_trip_and_strict_bounds() {
    let request = vec![7; EXECUTE_BODY];
    let output = FunctionalRefinementRuntimeProcessOutputV1 {
        exit_code: Some(0),
        signal: None,
        stdout: b"verified".to_vec(),
        stderr: vec![],
    };
    let bytes = encode_output(&request, &output).unwrap();
    let decoded = decode_output(&bytes, MAX_OUTPUT).unwrap();
    assert_eq!(decoded.exit_code, output.exit_code);
    assert_eq!(decoded.signal, output.signal);
    assert_eq!(decoded.stdout, output.stdout);
    assert_eq!(decoded.stderr, output.stderr);
    for end in 0..bytes.len() {
        assert!(decode_output(&bytes[..end], MAX_OUTPUT).is_err());
    }
    assert!(decode_output(&bytes, 7).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_output(&trailing, MAX_OUTPUT).is_err());
    let mut noncanonical = bytes;
    noncanonical[132] = 1;
    assert!(decode_output(&noncanonical, MAX_OUTPUT).is_err());
}

#[test]
fn failed_or_unwinding_operation_permanently_poison_owner() {
    let poisoned = AtomicBool::new(false);
    assert!(fail_closed(&poisoned, None, || Ok(())).is_ok());
    assert!(!poisoned.load(Ordering::Acquire));
    assert!(fail_closed(&poisoned, None, || Err::<(), _>(invalid("test rejection"))).is_err());
    assert!(fail_closed(&poisoned, None, || Ok(())).is_err());
    let poisoned = AtomicBool::new(false);
    let panic = std::panic::catch_unwind(|| {
        fail_closed(&poisoned, None, || -> Result<()> { panic!("test panic") })
    });
    assert!(panic.is_err());
    assert!(fail_closed(&poisoned, None, || Ok(())).is_err());
}

#[test]
fn lock_timeout_revokes_in_flight_operation() {
    let poisoned = AtomicBool::new(false);
    let mutex = Mutex::new(());
    let _held = mutex.lock().unwrap();
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let result = fail_closed(&poisoned, None, || {
                    lock_until(
                        &mutex,
                        Instant::now() + Duration::from_millis(30),
                        || Ok(()),
                    )
                });
                assert!(result.is_err());
            })
            .join()
            .unwrap();
    });
    assert!(poisoned.load(Ordering::Acquire));
    assert!(fail_closed(&poisoned, None, || Ok(())).is_err());
}

#[test]
fn concurrent_revocation_prevents_successful_completion() {
    let poisoned = AtomicBool::new(false);
    assert!(
        fail_closed(&poisoned, None, || {
            poisoned.store(true, Ordering::Release);
            Ok(())
        })
        .is_err()
    );
}

#[test]
fn poisoning_closes_transport_while_descriptor_owner_remains_alive() {
    let (left, right) = transport::pair().unwrap();
    let left = Endpoint::admit(left).unwrap();
    let right = Endpoint::admit(right).unwrap();
    let poisoned = AtomicBool::new(false);
    assert!(
        fail_closed(&poisoned, Some(left.fd.as_fd()), || Err::<(), _>(invalid(
            "cancel"
        )))
        .is_err()
    );
    assert!(right.revalidate().is_err());
    assert!(fail_closed(&poisoned, Some(left.fd.as_fd()), || Ok(())).is_err());
    assert!(rustix::io::fcntl_getfd(&left.fd).is_ok());
}

#[test]
fn child_descriptor_installation_rejects_occupied_reserved_slots() {
    use std::os::unix::process::CommandExt;
    for occupied in [
        COMPILER_PROOF_ENDPOINT_CHILD_FD_V1,
        COMPILER_PROOF_BROKER_CHILD_FD_V1,
    ] {
        let (left, right) = transport::pair().unwrap();
        let left = rustix::io::fcntl_dupfd_cloexec(left, DESCRIPTOR_FLOOR).unwrap();
        let right = rustix::io::fcntl_dupfd_cloexec(right, DESCRIPTOR_FLOOR).unwrap();
        let blocker = File::open("/dev/null").unwrap();
        let mut command = Command::new("/bin/true");
        // SAFETY: only async-signal-safe dup3 runs in the fork child.
        unsafe {
            command.pre_exec(move || {
                if libc::dup3(blocker.as_raw_fd(), occupied, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        configure_child_descriptors(&mut command, left, right);
        assert_eq!(
            crate::executor::spawn_artifact_coordinated_child(&mut command)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EBUSY)
        );
    }
}

#[test]
fn child_command_drop_releases_all_captured_endpoint_aliases() {
    let (server, client) = transport::pair().unwrap();
    let server = Endpoint::admit(server).unwrap();
    let client = rustix::io::fcntl_dupfd_cloexec(client, DESCRIPTOR_FLOOR).unwrap();
    let original =
        rustix::io::fcntl_dupfd_cloexec(File::open("/dev/null").unwrap(), DESCRIPTOR_FLOOR)
            .unwrap();
    let mut command = Command::new("/bin/sh");
    command.args(["-c", &format!("test -e /proc/self/fd/{COMPILER_PROOF_ENDPOINT_CHILD_FD_V1} && test -e /proc/self/fd/{COMPILER_PROOF_BROKER_CHILD_FD_V1}")]);
    configure_child_descriptors(&mut command, client, original);
    let mut child = crate::executor::spawn_artifact_coordinated_child(&mut command).unwrap();
    assert!(child.wait().unwrap().success());
    server.revalidate().unwrap();
    drop(command);
    assert!(server.revalidate().is_err());
}

#[test]
fn child_handoff_fits_protected_launcher_minimum_descriptor_limit() {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "compiler_proof_broker_v1::tests::low_descriptor_limit_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    // SAFETY: only resource-limit syscalls over stack scalars run after fork.
    unsafe {
        command.pre_exec(|| {
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) < 0 {
                return Err(io::Error::last_os_error());
            }
            limit.rlim_cur = 243;
            if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = crate::executor::spawn_artifact_coordinated_child(&mut command)
        .unwrap()
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "subprocess fixture with RLIMIT_NOFILE=243"]
fn low_descriptor_limit_child() {
    // Four capability descriptors and the authenticated invocation stream remain live. Three
    // delegation owners, two Command aliases and the original-child validation duplicate fit
    // below the launcher's 240..242 authority slots, even with this conservative joint roster.
    let source = File::open("/dev/null").unwrap();
    let roster = (0..11)
        .map(|_| rustix::io::fcntl_dupfd_cloexec(&source, DESCRIPTOR_FLOOR).unwrap())
        .collect::<Vec<_>>();
    assert!(roster.last().unwrap().as_raw_fd() < 240);
    drop(roster);
    let _capabilities = (0..5)
        .map(|_| rustix::io::fcntl_dupfd_cloexec(&source, DESCRIPTOR_FLOOR).unwrap())
        .collect::<Vec<_>>();
    child_command_drop_releases_all_captured_endpoint_aliases();
}
