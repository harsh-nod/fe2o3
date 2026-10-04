use super::*;
use std::io::Write;

// Descriptive protocol data only. Real sealed-image/sandbox admission is covered by the
// static host-consumer integration tests, not these startup ownership tests.
pub(super) fn protocol() -> ApplicationHandoffExpectationV1 {
    let mut elf = vec![0_u8; 0x1001];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16..18].copy_from_slice(&2_u16.to_le_bytes());
    elf[18..20].copy_from_slice(&62_u16.to_le_bytes());
    elf[20..24].copy_from_slice(&1_u32.to_le_bytes());
    elf[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    elf[32..40].copy_from_slice(&64_u64.to_le_bytes());
    elf[52..54].copy_from_slice(&64_u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56_u16.to_le_bytes());
    elf[56..58].copy_from_slice(&4_u16.to_le_bytes());
    for (index, (kind, flags, offset, address, size, alignment)) in [
        (6_u32, 4_u32, 64_u64, 0x400040_u64, 224_u64, 8_u64),
        (1, 4, 0, 0x400000, 288, 0x1000),
        (1, 5, 0x1000, 0x401000, 1, 0x1000),
        (0x6474e551, 6, 0, 0, 0, 16),
    ]
    .into_iter()
    .enumerate()
    {
        let start = 64 + index * 56;
        elf[start..start + 4].copy_from_slice(&kind.to_le_bytes());
        elf[start + 4..start + 8].copy_from_slice(&flags.to_le_bytes());
        elf[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
        elf[start + 16..start + 24].copy_from_slice(&address.to_le_bytes());
        elf[start + 32..start + 40].copy_from_slice(&size.to_le_bytes());
        elf[start + 40..start + 48].copy_from_slice(&size.to_le_bytes());
        elf[start + 48..start + 56].copy_from_slice(&alignment.to_le_bytes());
    }
    elf[0x1000] = 0xc3;
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&elf).unwrap(),
        [9; 32],
        &[
            WorkerV3ApplicationInputOccurrenceV1::new(1, [1; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(2, [2; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(3, [3; 32]).unwrap(),
        ],
    )
    .unwrap();
    ApplicationHandoffExpectationV1 {
        expectation: WorkerV3ApplicationHandoffExpectationV1::new(
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"opaque envelope").unwrap(),
            &occurrence,
        ),
        occurrence,
        descriptors: [10, 11, 12],
        proof_descriptor: None,
        challenge: WorkerV3ApplicationHandoffChallengeV1::from_bytes([7; 32]).unwrap(),
    }
}

fn exited_child() -> Child {
    let mut command = Command::new("/bin/true");
    // SAFETY: only child-side session syscalls run before exec.
    unsafe { command.pre_exec(establish_fresh_application_session) };
    let child = crate::process_execution::spawn(&mut command).unwrap();
    wait_for_application_exit_without_reaping(&child).unwrap();
    child
}

fn pending(child: &Child) -> PendingApplicationAck {
    let (read, write) = cloexec_pipe().unwrap();
    PendingApplicationAck {
        read,
        parent_write: Some(write),
        custody: ApplicationHandoffCustodyV1::prepare(
            protocol(),
            Some(PreparedApplicationProofChannelV1::prepare().unwrap()),
        ),
        sandbox: Some(PendingApplicationSandbox::test_reported_admission(Ok(
            child.id(),
        ))),
        reaper: Some(application_reaper().reserve().unwrap()),
        timeouts: ApplicationTimeouts::TEST_SCHEDULER_TOLERANT,
        test_ready_read: None,
        test_ready_parent_write: None,
    }
}

fn queue_ack(pending: &mut PendingApplicationAck) {
    let bytes = pending
        .custody
        .protocol
        .expectation
        .acknowledgment(pending.custody.protocol.challenge)
        .encode_canonical()
        .unwrap();
    pending
        .parent_write
        .as_mut()
        .unwrap()
        .write_all(&bytes)
        .unwrap();
}

fn spawned(pending: PendingApplicationAck, child: &Child) -> SpawnedApplicationAck {
    match pending.after_spawn(child) {
        Ok(spawned) => spawned,
        Err(failure) => panic!("startup failed: {}", failure.message),
    }
}

fn assert_original(cleanup: &ApplicationCleanup, child: &Child, descriptor: RawFd) {
    let original = cleanup.application.as_ref().unwrap();
    assert!(original.custody.prepared_proof.is_none());
    original
        .custody
        .proof
        .as_ref()
        .unwrap()
        .revalidate()
        .unwrap();
    assert_eq!(original.child().test_child_pidfd().as_raw_fd(), descriptor);
    assert_eq!(original.child().child_pid(), child.id());
    assert!(
        rustix::io::fcntl_getfd(original.child().test_child_pidfd())
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    let fdinfo = std::fs::read_to_string(format!("/proc/self/fdinfo/{descriptor}")).unwrap();
    assert!(
        fdinfo
            .lines()
            .any(|line| line == format!("Pid:\t{}", child.id()))
    );
    let expected = protocol();
    assert_eq!(original.protocol().occurrence, expected.occurrence);
    assert_eq!(original.protocol().descriptors, expected.descriptors);
    assert_eq!(original.protocol().expectation, expected.expectation);
    assert_eq!(original.protocol().challenge, expected.challenge);
}

#[test]
fn startup_releases_ack_writers_before_service_wait() {
    let child = exited_child();
    let mut pending = pending(&child);
    queue_ack(&mut pending);
    let (ready_read, mut ready_write) = cloexec_pipe().unwrap();
    ready_write.write_all(&[1]).unwrap();
    pending.test_ready_read = Some(ready_read);
    pending.test_ready_parent_write = Some(ready_write);
    let allocation = std::ptr::from_ref(pending.custody.as_ref());
    assert!(pending.custody.child.is_none());
    let proof_setup = pending
        .custody
        .prepared_proof
        .as_ref()
        .unwrap()
        .child_setup();
    let mut spawned = spawned(pending, &child);
    // Original pidfd capture may reuse the closed number, but never the socket object.
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: scalar fstat observation writes only its initialized result on success.
    if unsafe { libc::fstat(proof_setup.descriptor(), stat.as_mut_ptr()) } == 0 {
        // SAFETY: successful fstat initialized the structure.
        let stat = unsafe { stat.assume_init() };
        assert_ne!(
            (stat.st_dev, stat.st_ino, stat.st_mode),
            proof_setup.descriptor_identity()
        );
    } else {
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    }
    assert_eq!(
        allocation,
        std::ptr::from_ref(
            spawned
                .cleanup
                .application
                .as_ref()
                .unwrap()
                .custody
                .as_ref()
        )
    );
    let original_fd = spawned
        .cleanup
        .application
        .as_ref()
        .unwrap()
        .child()
        .test_child_pidfd()
        .as_raw_fd();
    assert_original(&spawned.cleanup, &child, original_fd);
    let mut bytes = Vec::new();
    // Nonblocking read_to_end must reach EOF, not WouldBlock from a parent writer alias.
    spawned.read.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes.len(), WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1);
    spawned
        .cleanup
        .application
        .as_ref()
        .unwrap()
        .protocol()
        .validate_ack(&bytes)
        .unwrap();
    bytes.clear();
    spawned
        .test_ready_read
        .as_mut()
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, [1]);
    assert!(
        wait_and_contain_application_group(child, spawned.into_cleanup())
            .unwrap()
            .success()
    );
}

#[test]
fn fast_ack_and_exit_retains_original_pidfd_through_active_handoff() {
    let child = exited_child();
    let mut pending = pending(&child);
    queue_ack(&mut pending);
    let spawned = spawned(pending, &child);
    let original_fd = spawned
        .cleanup
        .application
        .as_ref()
        .unwrap()
        .child()
        .test_child_pidfd()
        .as_raw_fd();
    let active = match spawned.await_ack(&child) {
        Ok(active) => active,
        Err(failure) => panic!("fast ACK failed: {}", failure.message),
    };
    assert_original(active.cleanup.as_ref().unwrap(), &child, original_fd);
    assert_eq!(
        observe_leader_exit_without_reaping(child.id() as libc::pid_t).unwrap(),
        LeaderExitObservation::Exited
    );
    assert!(
        wait_and_contain_application_group(child, active.into_cleanup())
            .unwrap()
            .success()
    );
}

#[test]
fn fast_exit_before_required_service_retains_original_cleanup() {
    use crate::compiler_execution_boundary::{
        CompilerExecutionBoundaryErrorV1, PreparedCompilerExecutionBoundaryV1,
        tests::{client_profile, run_in_isolated_boundary_test_process},
    };
    use fe2o3_compiler_execution_client::CompilerExecutionChildChannelErrorV1;

    if run_in_isolated_boundary_test_process(
        "application_handoff::startup_tests::fast_exit_before_required_service_retains_original_cleanup",
    ) {
        return;
    }
    let profile = client_profile(8, 1_234);
    let mut command = Command::new("/bin/true");
    // SAFETY: only child-side session syscalls run before exec.
    unsafe { command.pre_exec(establish_fresh_application_session) };
    let boundary =
        PreparedCompilerExecutionBoundaryV1::prepare_application_verifier(&profile, &mut command)
            .unwrap();
    let child = crate::process_execution::spawn(&mut command).unwrap();
    wait_for_application_exit_without_reaping(&child).unwrap();
    let mut pending = pending(&child);
    queue_ack(&mut pending);
    let spawned = spawned(pending, &child);
    let original_fd = spawned.retained_child().test_child_pidfd().as_raw_fd();
    assert!(matches!(
        boundary.finish_application(spawned.retained_child()),
        Err(CompilerExecutionBoundaryErrorV1::ChildChannel(
            CompilerExecutionChildChannelErrorV1::ChildExited
        ))
    ));
    assert_original(&spawned.cleanup, &child, original_fd);
    terminate_application_group(child, spawned.into_cleanup()).unwrap();
}

#[test]
fn missing_or_substituted_ack_keeps_original_cleanup_custody() {
    for changed in [false, true] {
        let child = exited_child();
        let mut pending = pending(&child);
        if changed {
            queue_ack(&mut pending);
            pending.custody.protocol.challenge =
                WorkerV3ApplicationHandoffChallengeV1::from_bytes([8; 32]).unwrap();
        }
        let spawned = spawned(pending, &child);
        let original_fd = spawned
            .cleanup
            .application
            .as_ref()
            .unwrap()
            .child()
            .test_child_pidfd()
            .as_raw_fd();
        let failure = match spawned.await_ack(&child) {
            Ok(_) => panic!("invalid ACK accepted"),
            Err(failure) => failure,
        };
        let (message, cleanup) = failure.into_parts();
        assert!(message.contains("acknowledgment"), "{message}");
        assert_eq!(
            cleanup
                .application
                .as_ref()
                .unwrap()
                .child()
                .test_child_pidfd()
                .as_raw_fd(),
            original_fd
        );
        assert_eq!(
            cleanup.application.as_ref().unwrap().child().child_pid(),
            child.id()
        );
        terminate_application_group(child, cleanup).unwrap();
    }
}

#[test]
fn substituted_child_rejects_without_losing_original_owner() {
    let child = exited_child();
    let other = exited_child();
    let spawned = spawned(pending(&child), &child);
    let original_fd = spawned
        .cleanup
        .application
        .as_ref()
        .unwrap()
        .child()
        .test_child_pidfd()
        .as_raw_fd();
    let failure = match spawned.await_ack(&other) {
        Ok(_) => panic!("substituted child accepted"),
        Err(failure) => failure,
    };
    let (message, cleanup) = failure.into_parts();
    assert!(message.contains("substituted"));
    assert_original(&cleanup, &child, original_fd);
    terminate_application_group(child, cleanup).unwrap();
    let other_cleanup = spawned_for_cleanup(&other);
    terminate_application_group(other, other_cleanup).unwrap();
}

fn spawned_for_cleanup(child: &Child) -> ApplicationCleanup {
    spawned(pending(child), child).into_cleanup()
}

#[test]
fn sandbox_admission_failure_retains_captured_original_process() {
    let child = exited_child();
    let mut pending = pending(&child);
    pending.sandbox = Some(PendingApplicationSandbox::test_reported_admission(Err(
        "injected sandbox admission failure".to_string(),
    )));
    let failure = match pending.after_spawn(&child) {
        Ok(_) => panic!("failed sandbox admitted"),
        Err(failure) => failure,
    };
    let (message, cleanup) = failure.into_parts();
    assert!(message.contains("injected sandbox admission failure"));
    assert!(cleanup.sandbox.is_some());
    let original_fd = cleanup
        .application
        .as_ref()
        .unwrap()
        .child()
        .test_child_pidfd()
        .as_raw_fd();
    assert_original(&cleanup, &child, original_fd);
    terminate_application_group(child, cleanup).unwrap();
}
