#![cfg(target_os = "linux")]

use std::fs::File;
use std::mem;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1, CompilerExecutionReceiptReceiverV1,
    CompilerExecutionReceiptReturnErrorV1, CompilerExecutionReceiptSenderV1,
    PendingCompilerExecutionReceiptReturnV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationV1,
};
use sha2::{Digest, Sha256};

const CHILD_MODE_ENV: &str = "FE2O3_TEST_RECEIPT_RETURN_CHILD_MODE";
const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

static RESERVED_FD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct Fixture {
    policy: CompilerExecutionIssuerPolicyV1,
    subject: InertCompilerExecutionSubjectV1,
    carriage: CompilerExecutionReceiptCarriageV1,
}

impl Fixture {
    fn new(seed: u8) -> Self {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
        )
        .unwrap();
        let subject = subject(seed + 3);
        let challenge = CompilerExecutionAttestationChallengeV1::new(
            &policy,
            &subject,
            [seed + 4; 32],
            1,
            [0; 32],
        )
        .unwrap();
        let request =
            CompilerExecutionAttestationRequestV1::new(challenge, subject.clone()).unwrap();
        let receipt =
            CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &key).unwrap();
        let publication =
            CompilerExecutionReceiptPublicationV1::new([seed + 5; 32], [seed + 6; 32], receipt)
                .unwrap();
        let acknowledgment =
            CompilerExecutionReceiptPublicationAckV1::new(&publication, [seed + 7; 32]).unwrap();
        let carriage = CompilerExecutionReceiptCarriageV1::new(
            policy.clone(),
            request,
            publication,
            acknowledgment,
        )
        .unwrap();
        Self {
            policy,
            subject,
            carriage,
        }
    }
}

#[test]
fn exact_child_receipt_is_pid_bound_and_returned() {
    let fixture = Fixture::new(0x20);
    let (mut child, receiver) = spawn_child("exact");
    assert_eq!(receiver.client().pid(), child.id());
    let received = receiver
        .receive_exact(&fixture.policy, &fixture.subject, Duration::from_secs(2))
        .unwrap();
    assert_eq!(received, fixture.carriage);
    assert!(child.wait().unwrap().success());
}

#[test]
fn caller_pinned_policy_and_subject_substitution_fail_closed() {
    let fixture = Fixture::new(0x20);
    let other = Fixture::new(0x40);

    let (mut child, receiver) = spawn_child("exact");
    assert!(matches!(
        receiver.receive_exact(&other.policy, &fixture.subject, Duration::from_secs(2)),
        Err(CompilerExecutionReceiptReturnErrorV1::PolicyMismatch)
    ));
    assert!(child.wait().unwrap().success());

    let (mut child, receiver) = spawn_child("exact");
    assert!(matches!(
        receiver.receive_exact(&fixture.policy, &other.subject, Duration::from_secs(2)),
        Err(CompilerExecutionReceiptReturnErrorV1::SubjectMismatch)
    ));
    assert!(child.wait().unwrap().success());
}

#[test]
fn short_extended_ancillary_and_trailing_packets_fail_closed() {
    let fixture = Fixture::new(0x20);
    for (mode, expected) in [
        ("short", "wrong-length"),
        ("extended", "truncated"),
        ("ancillary", "ancillary"),
        ("trailing", "trailing"),
    ] {
        let (mut child, receiver) = spawn_child(mode);
        let error = receiver
            .receive_exact(&fixture.policy, &fixture.subject, Duration::from_secs(2))
            .unwrap_err();
        match expected {
            "wrong-length" => assert!(matches!(
                error,
                CompilerExecutionReceiptReturnErrorV1::WrongPacketLength { .. }
            )),
            "truncated" => assert!(matches!(
                error,
                CompilerExecutionReceiptReturnErrorV1::TruncatedPacket
            )),
            "ancillary" => assert!(matches!(
                error,
                CompilerExecutionReceiptReturnErrorV1::AncillaryData
            )),
            "trailing" => assert!(matches!(
                error,
                CompilerExecutionReceiptReturnErrorV1::TrailingPacket
            )),
            _ => unreachable!(),
        }
        assert!(child.wait().unwrap().success());
    }
}

#[test]
fn child_exit_without_receipt_and_occupied_target_fail_closed() {
    let fixture = Fixture::new(0x20);
    let (mut child, receiver) = spawn_child("absent");
    assert!(matches!(
        receiver.receive_exact(&fixture.policy, &fixture.subject, Duration::from_secs(2)),
        Err(CompilerExecutionReceiptReturnErrorV1::ChildExitedWithoutReceipt)
    ));
    assert!(child.wait().unwrap().success());

    let _guard = RESERVED_FD_LOCK.lock().unwrap();
    // SAFETY: dup2 returns one owned alias at the fixed descriptor, closed by `occupied`.
    let occupied = unsafe {
        let source = File::open("/dev/null").unwrap();
        let descriptor = libc::dup2(
            source.as_raw_fd(),
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
        );
        assert_eq!(descriptor, COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1);
        File::from_raw_fd(descriptor)
    };
    let mut command = Command::new("/bin/true");
    assert!(PendingCompilerExecutionReceiptReturnV1::prepare(&mut command).is_err());
    drop(occupied);
}

#[test]
fn another_live_child_pid_cannot_claim_the_return_channel() {
    let mut command = child_command("exact");
    let pending = PendingCompilerExecutionReceiptReturnV1::prepare(&mut command).unwrap();
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut actual = command.spawn().unwrap();
    let mut unrelated = Command::new("/bin/sleep").arg("2").spawn().unwrap();
    assert!(matches!(
        pending.finish(unrelated.id(), Duration::from_secs(2)),
        Err(CompilerExecutionReceiptReturnErrorV1::ChildPidMismatch)
    ));
    actual.wait().unwrap();
    unrelated.kill().unwrap();
    unrelated.wait().unwrap();
}

#[test]
fn later_child_callback_cannot_remove_the_fixed_sender() {
    let fixture = Fixture::new(0x20);
    let mut command = child_command("exact");
    let pending = PendingCompilerExecutionReceiptReturnV1::prepare(&mut command).unwrap();
    // SAFETY: close is async-signal-safe and operates only on the preceding callback's target.
    unsafe {
        command.pre_exec(|| {
            if libc::close(COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn().unwrap();
    let receiver = pending.finish(child.id(), Duration::from_secs(2)).unwrap();
    assert!(matches!(
        receiver.receive_exact(&fixture.policy, &fixture.subject, Duration::from_secs(2)),
        Err(CompilerExecutionReceiptReturnErrorV1::ChildExitedWithoutReceipt)
    ));
    assert!(!child.wait().unwrap().success());
}

#[test]
fn receipt_return_child_entry() {
    let Ok(mode) = std::env::var(CHILD_MODE_ENV) else {
        return;
    };
    match mode.as_str() {
        "exact" => {
            let fixture = Fixture::new(0x20);
            let keepalive = duplicate_inherited_sender();
            CompilerExecutionReceiptSenderV1::from_inherited_child()
                .unwrap()
                .send_exact(
                    &fixture.policy,
                    &fixture.subject,
                    fixture.carriage,
                    Duration::from_secs(2),
                )
                .unwrap();
            wait_for_parent_close(&keepalive);
        }
        "short" => {
            raw_send(&[0_u8; 8]);
            wait_for_parent_close(&duplicate_inherited_sender());
        }
        "extended" => {
            raw_send(&vec![
                0_u8;
                COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1 + 1
            ]);
            wait_for_parent_close(&duplicate_inherited_sender());
        }
        "ancillary" => {
            raw_send_with_descriptor();
            wait_for_parent_close(&duplicate_inherited_sender());
        }
        "trailing" => {
            let fixture = Fixture::new(0x20);
            raw_send(fixture.carriage.canonical_bytes());
            raw_send(&[0_u8]);
            // SAFETY: the test child owns the inherited sender and has completed both packets.
            assert_eq!(
                unsafe {
                    libc::shutdown(COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1, libc::SHUT_WR)
                },
                0
            );
            wait_for_parent_close(&duplicate_inherited_sender());
        }
        "absent" => {}
        _ => panic!("unknown child mode"),
    }
}

fn spawn_child(mode: &str) -> (Child, CompilerExecutionReceiptReceiverV1) {
    let mut command = child_command(mode);
    let pending = PendingCompilerExecutionReceiptReturnV1::prepare(&mut command).unwrap();
    let child = command.spawn().unwrap();
    let receiver = pending.finish(child.id(), Duration::from_secs(2)).unwrap();
    (child, receiver)
}

fn child_command(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("receipt_return_child_entry")
        .arg("--nocapture")
        .env(CHILD_MODE_ENV, mode);
    command
}

fn raw_send(bytes: &[u8]) {
    // SAFETY: the fixed descriptor is installed by the immediately preceding pre-exec callback.
    let sent = unsafe {
        libc::send(
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
            bytes.as_ptr().cast(),
            bytes.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    assert_eq!(sent, bytes.len() as isize);
}

fn raw_send_with_descriptor() {
    let fixture = Fixture::new(0x20);
    let pipe = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let bytes = fixture.carriage.canonical_bytes();
    let mut vector = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let mut control = [0_usize; 8];
    // SAFETY: zero initializes a valid empty msghdr populated below with live stack storage.
    let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
    header.msg_iov = &mut vector;
    header.msg_iovlen = 1;
    header.msg_control = control.as_mut_ptr().cast();
    header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<i32>() as u32) } as usize;
    // SAFETY: the aligned buffer is large enough for one SCM_RIGHTS descriptor.
    unsafe {
        let message = libc::CMSG_FIRSTHDR(&header);
        assert!(!message.is_null());
        (*message).cmsg_level = libc::SOL_SOCKET;
        (*message).cmsg_type = libc::SCM_RIGHTS;
        (*message).cmsg_len = libc::CMSG_LEN(mem::size_of::<i32>() as u32) as usize;
        libc::CMSG_DATA(message)
            .cast::<i32>()
            .write_unaligned(pipe.0.as_fd().as_raw_fd());
        let sent = libc::sendmsg(
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
            &header,
            libc::MSG_NOSIGNAL,
        );
        assert_eq!(sent, bytes.len() as isize);
    }
}

fn duplicate_inherited_sender() -> OwnedFd {
    // SAFETY: F_DUPFD_CLOEXEC returns one independently owned test keepalive descriptor.
    let descriptor = unsafe {
        libc::fcntl(
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
            libc::F_DUPFD_CLOEXEC,
            3,
        )
    };
    assert!(descriptor >= 0);
    // SAFETY: successful F_DUPFD_CLOEXEC returned one new owned descriptor.
    unsafe { OwnedFd::from_raw_fd(descriptor) }
}

fn wait_for_parent_close(peer: &OwnedFd) {
    let mut descriptor = libc::pollfd {
        fd: peer.as_raw_fd(),
        events: libc::POLLHUP | libc::POLLERR,
        revents: 0,
    };
    // SAFETY: descriptor names one live test-only pollfd slot.
    assert_eq!(unsafe { libc::poll(&mut descriptor, 1, 3_000) }, 1);
    assert_ne!(descriptor.revents & (libc::POLLHUP | libc::POLLERR), 0);
}

fn subject(seed: u8) -> InertCompilerExecutionSubjectV1 {
    let closure_pins = [
        [seed; 32],
        [seed + 1; 32],
        [seed + 2; 32],
        [seed + 3; 32],
        [seed + 4; 32],
        [seed + 5; 32],
    ];
    let mut closure_digest = Sha256::new();
    closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
    closure_digest.update(1_u16.to_le_bytes());
    for pin in closure_pins {
        closure_digest.update(pin);
    }
    let closure_identity: [u8; 32] = closure_digest.finalize().into();
    let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
    let mut offset = 0;
    put(
        &mut bytes,
        &mut offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    );
    put(
        &mut bytes,
        &mut offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
    );
    put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
    put(
        &mut bytes,
        &mut offset,
        &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
    );
    put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
    put(&mut bytes, &mut offset, &9_u64.to_le_bytes());
    put(&mut bytes, &mut offset, &[seed + 6; 16]);
    put(&mut bytes, &mut offset, &[seed + 7; 32]);
    bytes[offset] = 0;
    offset += 8;
    put(&mut bytes, &mut offset, &[seed + 8; 32]);
    put(&mut bytes, &mut offset, &[seed + 9; 32]);
    for pin in closure_pins {
        put(&mut bytes, &mut offset, &pin);
    }
    put(&mut bytes, &mut offset, &1_u16.to_le_bytes());
    put(&mut bytes, &mut offset, &closure_identity);
    for axis in 0_u8..7 {
        put(&mut bytes, &mut offset, &[seed + 10 + axis; 32]);
        put(
            &mut bytes,
            &mut offset,
            &(1_000_u64 + u64::from(axis)).to_le_bytes(),
        );
    }
    let identity = digest(SUBJECT_IDENTITY_DOMAIN, &bytes[..offset]);
    put(&mut bytes, &mut offset, &identity);
    assert_eq!(offset, bytes.len());
    InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
}

fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}
