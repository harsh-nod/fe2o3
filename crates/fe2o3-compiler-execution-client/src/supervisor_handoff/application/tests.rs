use super::*;
use crate::child_channel::{PIDFD_OPEN_CALLS, RESERVED_CHILD_FD_LOCK};
use crate::{
    PendingCompilerExecutionChildChannelV1, PreparedApplicationProofChannelV1,
    RetainedCompilerExecutionChildV1,
};
use fe2o3_runtime_protocol::*;
use sha2::{Digest, Sha256};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().unwrap();
    }
}

fn policy() -> CompilerExecutionIssuerPolicyV1 {
    use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
    CompilerExecutionIssuerPolicyV1::new(
        1,
        CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([2; 32], 1).unwrap(),
        [3; 32],
        ed25519_dalek::SigningKey::from_bytes(&[4; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap()
}

fn anchor() -> CompilerExecutionExternalAnchorServiceIdentityV1 {
    CompilerExecutionExternalAnchorServiceIdentityV1::new(6000, 7000).unwrap()
}

// A canonical descriptive identity is sufficient here: transport never claims image admission.
fn image_identity() -> WorkerV3ApplicationIdentityV1 {
    let mut bytes = Vec::from(&b"F3AIDV1\0"[..]);
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&92u32.to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&[9; 32]);
    bytes.extend_from_slice(&123u64.to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/WORKER-V3/APPLICATION-IDENTITY-CHECKSUM/V1\0");
    digest.update(&bytes);
    bytes.extend_from_slice(&digest.finalize());
    WorkerV3ApplicationIdentityV1::decode_canonical(&bytes).unwrap()
}

struct Fixture {
    child: OwnedChild,
    retained: RetainedCompilerExecutionChildV1,
    launch: RetainedApplicationServiceLaunchV1,
    proof: ApplicationProofTransferPeerV1,
    binding: WorkerV3ApplicationRegistrationBindingV1,
}

fn fixture() -> Fixture {
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    let setup = prepared.child_setup();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let expose = setup.clone();
    // SAFETY: runs only in Command's single-threaded post-fork child with the pair alive.
    unsafe {
        command.pre_exec(move || expose.expose_before_exec());
    }
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let child = OwnedChild(command.spawn().unwrap());
    let proof = prepared.after_spawn();
    let retained = RetainedCompilerExecutionChildV1::capture(&child.0).unwrap();
    let before = PIDFD_OPEN_CALLS.get();
    let launch = pending
        .finish_application_until(&retained, Instant::now() + Duration::from_secs(2))
        .unwrap();
    assert_eq!(before, PIDFD_OPEN_CALLS.get());
    let (dev, ino, mode) = setup.descriptor_identity();
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(
        image_identity(),
        [10; 32],
        &[
            WorkerV3ApplicationInputOccurrenceV1::new(1, [1; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(2, [2; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(3, [3; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(4, dev, ino, mode)
                .unwrap(),
        ],
    )
    .unwrap();
    let expectation = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"inert fixture").unwrap(),
        &occurrence,
    );
    let binding = WorkerV3ApplicationRegistrationBindingV1::new(
        CompilerExecutionSupervisorHandoffV1::new(
            launch.submitter(),
            CompilerExecutionServiceLaunchManifestV1::new(launch.client(), anchor(), &policy()),
        )
        .unwrap(),
        occurrence,
        WorkerV3ApplicationRegistrationDescriptorsV1::new(210, 211, 212, setup.descriptor())
            .unwrap(),
        expectation,
        WorkerV3ApplicationHandoffChallengeV1::from_bytes([11; 32]).unwrap(),
    )
    .unwrap();
    Fixture {
        child,
        retained,
        launch,
        proof,
        binding,
    }
}

fn pair() -> (OwnedFd, OwnedFd) {
    rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}

fn expected() -> CompilerExecutionSupervisorCredentialsV1 {
    CompilerExecutionSupervisorCredentialsV1 {
        uid: rustix::process::geteuid().as_raw(),
        gid: rustix::process::getegid().as_raw(),
    }
}

#[test]
fn application_transfer_sends_exact_binding_and_four_original_rights_without_reopening() {
    let _lock = RESERVED_CHILD_FD_LOCK.lock().unwrap();
    let Fixture {
        child,
        retained,
        launch,
        proof,
        binding,
    } = fixture();
    let before = PIDFD_OPEN_CALLS.get();
    let (control, receiver) = pair();
    let pending = transfer(
        launch,
        proof,
        binding.clone(),
        control,
        expected(),
        anchor(),
        &policy(),
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(PIDFD_OPEN_CALLS.get(), before);
    assert_eq!(pending.binding(), &binding);
    let mut payload = [0; WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1];
    let mut vectors = [IoSliceMut::new(&mut payload)];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let received = recvmsg(
        &receiver,
        &mut vectors,
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(received.bytes, 840);
    assert!(
        !received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    );
    let mut rights = Vec::new();
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(fds) => rights.extend(fds),
            _ => panic!("unexpected ancillary"),
        }
    }
    assert_eq!(payload, *binding.canonical_bytes());
    assert_eq!(rights.len(), 4);
    for fd in &rights {
        assert_eq!(
            rustix::io::fcntl_getfd(fd).unwrap(),
            rustix::io::FdFlags::CLOEXEC
        );
    }
    assert_eq!(
        rustix::net::sockopt::socket_peercred(&rights[0])
            .unwrap()
            .pid
            .as_raw_pid() as u32,
        child.0.id()
    );
    for (index, pid) in [(1, child.0.id()), (3, std::process::id())] {
        let info =
            std::fs::read_to_string(format!("/proc/self/fdinfo/{}", rights[index].as_raw_fd()))
                .unwrap();
        assert!(info.lines().any(|line| line == format!("Pid:\t{pid}")));
    }
    assert!(rustix::net::sockopt::socket_passcred(&rights[2]).unwrap());
    retained.validate_custody().unwrap();
}

#[test]
fn application_transfer_rejects_foreign_pair_changed_binding_and_closed_control() {
    let _lock = RESERVED_CHILD_FD_LOCK.lock().unwrap();
    for mutation in 0..4 {
        let Fixture {
            child: _child,
            retained,
            launch,
            mut proof,
            binding,
        } = fixture();
        let before = PIDFD_OPEN_CALLS.get();
        let (control, receiver) = pair();
        let expected_policy = if mutation == 1 {
            use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
            CompilerExecutionIssuerPolicyV1::new(
                2,
                CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
                CompilerExecutionIssuerMeasurementV1::new([2; 32], 1).unwrap(),
                [3; 32],
                ed25519_dalek::SigningKey::from_bytes(&[4; 32])
                    .verifying_key()
                    .to_bytes(),
            )
            .unwrap()
        } else {
            policy()
        };
        if mutation == 0 {
            proof = PreparedApplicationProofChannelV1::prepare()
                .unwrap()
                .after_spawn();
        }
        if mutation == 2 {
            drop(receiver);
        }
        let deadline = if mutation == 3 {
            Instant::now()
        } else {
            Instant::now() + Duration::from_secs(1)
        };
        let result = transfer(
            launch,
            proof,
            binding,
            control,
            expected(),
            anchor(),
            &expected_policy,
            deadline,
        );
        assert!(result.is_err(), "mutation {mutation}");
        assert_eq!(before, PIDFD_OPEN_CALLS.get());
        retained.validate_custody().unwrap();
    }
}

#[test]
fn exited_original_application_cannot_be_transferred() {
    let _lock = RESERVED_CHILD_FD_LOCK.lock().unwrap();
    let Fixture {
        mut child,
        retained,
        launch,
        proof,
        binding,
    } = fixture();
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let (control, _receiver) = pair();
    assert!(
        transfer(
            launch,
            proof,
            binding,
            control,
            expected(),
            anchor(),
            &policy(),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err()
    );
    retained.validate_custody().unwrap();
}
