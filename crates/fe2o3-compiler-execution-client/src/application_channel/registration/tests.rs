use super::*;
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendFlags, recvmsg, sendmsg,
};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::{
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
};

use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionServiceLaunchManifestV1,
    CompilerExecutionSupervisorHandoffV1,
};
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationHandoffChallengeV1, WorkerV3ApplicationHandoffExpectationV1,
    WorkerV3ApplicationIdentityV1, WorkerV3ApplicationOccurrenceV1,
    WorkerV3ApplicationRegistrationBindingV1 as Binding,
    WorkerV3ApplicationRegistrationDescriptorsV1, WorkerV3LoadEnvelopeIdentityV1,
};
use rustix::net::SendAncillaryMessage;

const MODE: &str = "FE2O3_TEST_REGISTRATION_MODE";
const CASE: &str = "FE2O3_TEST_REGISTRATION_CASE";
const FD: &str = "FE2O3_TEST_REGISTRATION_FD";
const PACKET: &str = "FE2O3_TEST_REGISTRATION_PACKET";

mod custodian;

fn signal(fd: &OwnedFd, signal: i32) {
    // SAFETY: the original fixture pidfd identifies only this campaign's retained process.
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        },
        0
    );
}

fn stop_app(fd: &OwnedFd, pid: u32) {
    signal(fd, libc::SIGSTOP);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        if stat.rsplit_once(')').unwrap().1.split_whitespace().next() == Some("T") {
            return;
        }
        assert!(Instant::now() < deadline, "fixture app did not stop");
        std::thread::sleep(Duration::from_millis(1));
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl ChildGuard {
    fn wait(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline, "registration helper timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn pidfd(pid: u32) -> OwnedFd {
    // SAFETY: scalar inputs; success yields a new exclusively owned descriptor.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    assert!(fd >= 0, "pidfd_open: {}", io::Error::last_os_error());
    // SAFETY: successful syscall yielded this owned descriptor.
    unsafe { OwnedFd::from_raw_fd(fd as i32) }
}

// A codec fixture only, not the running helper's measured or admitted executable.
fn image_identity() -> WorkerV3ApplicationIdentityV1 {
    let mut image = vec![0; 4097];
    image[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    for (at, value) in [(16, 2_u16), (18, 62), (52, 64), (54, 56), (56, 4)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    image[20..24].copy_from_slice(&1_u32.to_le_bytes());
    image[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    image[32..40].copy_from_slice(&64_u64.to_le_bytes());
    for (i, (kind, flags, offset, address, size, alignment)) in [
        (6_u32, 4_u32, 64_u64, 0x400040_u64, 224_u64, 8_u64),
        (1, 4, 0, 0x400000, 288, 4096),
        (1, 5, 4096, 0x401000, 1, 4096),
        (0x6474e551, 6, 0, 0, 0, 16),
    ]
    .into_iter()
    .enumerate()
    {
        let at = 64 + i * 56;
        image[at..at + 4].copy_from_slice(&kind.to_le_bytes());
        image[at + 4..at + 8].copy_from_slice(&flags.to_le_bytes());
        for (delta, value) in [
            (8, offset),
            (16, address),
            (32, size),
            (40, size),
            (48, alignment),
        ] {
            image[at + delta..at + delta + 8].copy_from_slice(&value.to_le_bytes());
        }
    }
    image[4096] = 0xc3;
    WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&image).unwrap()
}

fn inputs(
    fd: RawFd,
    object: (u64, u64, u32),
) -> (
    WorkerV3ApplicationOccurrenceV1,
    WorkerV3ApplicationRegistrationDescriptorsV1,
    WorkerV3ApplicationHandoffExpectationV1,
    WorkerV3ApplicationHandoffChallengeV1,
) {
    let case = std::env::var(CASE).unwrap_or_default();
    let fd = if case == "wrong_coordinate" {
        fd + 100
    } else {
        fd
    };
    let object = if case == "wrong_object" {
        (object.0, object.1 + 1, object.2)
    } else {
        object
    };
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(
        image_identity(),
        [5; 32],
        &[
            WorkerV3ApplicationInputOccurrenceV1::new(1, [1; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(2, [2; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(3, [3; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
                4, object.0, object.1, object.2,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let descriptors = WorkerV3ApplicationRegistrationDescriptorsV1::new(500, 501, 502, fd).unwrap();
    let expectation = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"fixture only").unwrap(),
        &occurrence,
    );
    (
        occurrence,
        descriptors,
        expectation,
        WorkerV3ApplicationHandoffChallengeV1::from_bytes([6; 32]).unwrap(),
    )
}

fn policy() -> CompilerExecutionIssuerPolicyV1 {
    let key = |hex: &str| {
        std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
    };
    CompilerExecutionIssuerPolicyV1::new(
        1,
        CompilerExecutionIssuerMeasurementV1::new([1; 32], 123).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([2; 32], 456).unwrap(),
        key("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
        key("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
    )
    .unwrap()
}

fn transmit(fd: &OwnedFd, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !rights.is_empty() {
            assert!(control.push(SendAncillaryMessage::ScmRights(rights)));
        }
        match sendmsg(
            fd,
            &[IoSlice::new(bytes)],
            &mut control,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(count) => {
                assert_eq!(count, bytes.len());
                return;
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("test send: {error}"),
        }
    }
}

fn receive_test(fd: &OwnedFd) -> (Vec<u8>, Vec<OwnedFd>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut bytes = [0; 1024];
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(3))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        match recvmsg(
            fd,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut control,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(received) => {
                assert!(received.bytes > 0);
                assert!(
                    !received
                        .flags
                        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                );
                let mut rights = Vec::new();
                for message in control.drain() {
                    if let RecvAncillaryMessage::ScmRights(fds) = message {
                        rights.extend(fds);
                    }
                }
                return (bytes[..received.bytes].to_vec(), rights);
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                assert!(Instant::now() < deadline, "test receive timed out");
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("test receive: {error}"),
        }
    }
}

fn helper(mode: &str, case: &str, raw: RawFd) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "application_channel::registration::tests::registration_helper",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MODE, mode)
        .env(CASE, case)
        .env(FD, raw.to_string());
    command
}

// A focused syscall-closure regression, not the full deployed application sandbox.
#[cfg(target_arch = "x86_64")]
fn deny_process_and_socket_creation() {
    let denied = [
        libc::SYS_pidfd_open,
        libc::SYS_waitid,
        libc::SYS_pidfd_send_signal,
        libc::SYS_shutdown,
        libc::SYS_clone,
        libc::SYS_clone3,
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_connect,
        libc::SYS_bind,
    ];
    let stmt = |code: u16, k| libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    };
    let mut filter = vec![
        stmt((libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16, 4),
        libc::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16,
            jt: 1,
            jf: 0,
            k: 0xc000003e,
        },
        stmt(
            (libc::BPF_RET | libc::BPF_K) as u16,
            libc::SECCOMP_RET_KILL_PROCESS,
        ),
        stmt((libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16, 0),
    ];
    for syscall in denied {
        filter.push(libc::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16,
            jt: 0,
            jf: 1,
            k: syscall as u32,
        });
        filter.push(stmt(
            (libc::BPF_RET | libc::BPF_K) as u16,
            libc::SECCOMP_RET_ERRNO | libc::EPERM as u32,
        ));
    }
    filter.push(stmt(
        (libc::BPF_RET | libc::BPF_K) as u16,
        libc::SECCOMP_RET_ALLOW,
    ));
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: irreversible restriction on this isolated helper's calling thread only;
    // the initialized BPF program remains borrowed during installation.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(
            libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER, &program),
            0
        );
    }
    for syscall in denied {
        // SAFETY: denied before argument evaluation; invalid arguments also preclude effects.
        assert_eq!(
            unsafe { libc::syscall(syscall, -1_i64, 0_i64, 0_i64, 0_i64, 0_i64, 0_i64) },
            -1
        );
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EPERM));
    }
}

#[test]
#[ignore = "subprocess helper, invoked with an exact inherited test descriptor"]
fn registration_helper() {
    let mode = std::env::var(MODE).unwrap();
    let case = std::env::var(CASE).unwrap();
    let raw: RawFd = std::env::var(FD).unwrap().parse().unwrap();
    // SAFETY: the fixture parent transfers exactly one descriptor to this subprocess.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC).unwrap();
    if mode == "custodian_controller" {
        custodian::controller(fd, &case);
        return;
    }
    if mode == "custodian_root" {
        custodian::root_sender(fd);
        return;
    }
    if mode == "root_once" || mode == "foreign_ready" {
        let hex = std::env::var(PACKET).unwrap();
        let bytes: Vec<_> = (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        let challenge = Message::decode(&bytes).unwrap();
        if mode == "foreign_ready" {
            transmit(&fd, &bytes, &[]);
            return;
        }
        let root = pidfd(std::process::id());
        let app_pid = challenge
            .registration()
            .unwrap()
            .compiler_handoff()
            .launch_manifest()
            .client()
            .pid();
        let app = pidfd(app_pid);
        if case == "root_death_challenge" {
            stop_app(&app, app_pid);
        }
        transmit(&fd, &bytes, &[root.as_fd()]);
        if case == "root_death_ready" || case == "separate_root_positive" {
            let (bytes, rights) = receive_test(&fd);
            assert!(rights.is_empty());
            let accept = Message::decode(&bytes).unwrap();
            assert_eq!(accept.kind(), Kind::Accept);
            assert_eq!(accept.transcript(), challenge.transcript());
            if case == "root_death_ready" {
                stop_app(&app, app_pid);
            }
            transmit(
                &fd,
                Message::ready(challenge.transcript().unwrap()).canonical_bytes(),
                &[],
            );
            if case == "separate_root_positive" {
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    let mut byte = [0];
                    match rustix::net::recv(&fd, &mut byte, RecvFlags::DONTWAIT) {
                        Ok((0, _)) => break,
                        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                            assert!(Instant::now() < deadline);
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        other => panic!("unexpected app terminal packet: {other:?}"),
                    }
                }
            }
        }
        return;
    }
    if mode == "cargo" {
        let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
        let setup = prepared.child_setup();
        let (occurrence, descriptors, expectation, challenge) =
            inputs(setup.descriptor(), prepared.setup.snapshot.object);
        let mut command = helper("app", &case, setup.descriptor());
        let report = if case == "custodian_root_after" {
            command.env(custodian::CONTROL, fd.as_raw_fd().to_string());
            Some(fd.as_raw_fd())
        } else {
            None
        };
        // SAFETY: the prepared owner remains retained while the fork child exposes its endpoint.
        unsafe {
            command.pre_exec(move || {
                setup.expose_before_exec()?;
                if let Some(raw) = report
                    && libc::fcntl(raw, libc::F_SETFD, 0) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut app = ChildGuard(command.spawn().unwrap());
        let peer = prepared.after_spawn();
        let handoff = CompilerExecutionSupervisorHandoffV1::new(
            current_creator(std::process::id()).unwrap(),
            CompilerExecutionServiceLaunchManifestV1::new(
                current_creator(app.0.id()).unwrap(),
                CompilerExecutionExternalAnchorServiceIdentityV1::new(6000, 7000).unwrap(),
                &policy(),
            ),
        )
        .unwrap();
        let binding =
            Binding::new(handoff, occurrence, descriptors, expectation, challenge).unwrap();
        transmit(&fd, binding.canonical_bytes(), &[peer.peer.as_fd()]);
        drop(peer);
        app.wait();
        return;
    }
    assert_eq!(mode, "app");
    #[cfg(target_arch = "x86_64")]
    if custodian::strict_allowlist() {
        custodian::install_strict_allowlist();
    }
    let endpoint = RetainedApplicationProofEndpointV1::admit_inherited(fd).unwrap();
    let (occurrence, descriptors, expectation, challenge) =
        inputs(endpoint.peer.as_raw_fd(), endpoint.descriptor_identity());
    let inputs = WorkerV3ApplicationRegistrationInputsV1::new(
        occurrence,
        descriptors,
        expectation,
        challenge,
    )
    .unwrap();
    #[cfg(target_arch = "x86_64")]
    if !custodian::strict_allowlist() {
        deny_process_and_socket_creation();
    }
    if case.starts_with("custodian_") {
        custodian::application(endpoint, inputs, &case);
        return;
    }
    let duration = if case == "timeout" {
        Duration::from_millis(80)
    } else {
        Duration::from_secs(5)
    };
    let result = endpoint.register_pre_ack(inputs, Instant::now() + duration);
    if case == "live_root_closed_session" {
        let owner = result.unwrap();
        owner.revalidate().unwrap();
        transmit(&owner.endpoint.peer, b"fixture-owner-retained", &[]);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match owner.revalidate() {
                Err(ApplicationProofChannelErrorV1::Invalid("registered root session closed")) => {
                    break;
                }
                Ok(()) => {
                    assert!(
                        Instant::now() < deadline,
                        "registered session closure was not detected"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                other => panic!("wrong post-Ready continuity result: {other:?}"),
            }
        }
        owner.root.revalidate().unwrap();
    } else if case == "positive" || case == "separate_root_positive" {
        let owner = result.unwrap();
        owner.revalidate().unwrap();
        assert_ne!(owner.transcript().binding(), [0; 32]);
    } else {
        assert!(result.is_err(), "negative {case} accepted");
        if case.starts_with("root_death_") {
            assert!(
                matches!(&result, Err(ApplicationProofChannelErrorV1::Process(error))
                if error.kind() == fe2o3_process_identity::pidfd::PidfdObservationErrorKindV1::AlreadyDead),
                "dead original root was not the rejection reason: {result:?}"
            );
        }
        let expected_reason = match case.as_str() {
            "unprivileged_sender" | "wrong_nonce" | "phase" => {
                Some("registration Challenge sender or phase mismatch")
            }
            "wrong_inputs" | "wrong_coordinate" | "wrong_object" => {
                Some("Challenge differs from original application and Cargo inputs")
            }
            "missing_pidfd" | "ready_rights" => {
                Some("registration descriptor count differs from phase")
            }
            "extra_pidfd" | "extra_bytes" => {
                Some("application packet or ancillary roster is malformed")
            }
            "short" => Some("noncanonical application registration packet"),
            "wrong_ready" | "duplicate_challenge" | "foreign_ready" | "proof_ready" => {
                Some("registration Ready sender or transcript mismatch")
            }
            _ => None,
        };
        if let Some(expected) = expected_reason {
            assert!(
                matches!(&result, Err(ApplicationProofChannelErrorV1::Invalid(reason)) if *reason == expected),
                "wrong rejection for {case}: {result:?}"
            );
        }
        if case == "wrong_pidfd" || case == "ordinary_fd" {
            let expected = if case == "wrong_pidfd" {
                fe2o3_process_identity::pidfd::PidfdObservationErrorKindV1::TargetMismatch
            } else {
                fe2o3_process_identity::pidfd::PidfdObservationErrorKindV1::InspectPidfd
            };
            assert!(
                matches!(&result, Err(ApplicationProofChannelErrorV1::Process(error)) if error.kind() == expected),
                "wrong pidfd rejection for {case}: {result:?}"
            );
        }
        if case == "timeout" {
            assert!(matches!(
                result,
                Err(ApplicationProofChannelErrorV1::Timeout)
            ));
        }
        eprintln!("registration negative {case}: {}", result.unwrap_err());
    }
}

fn run_case(case: &str, root_campaign: bool) {
    let (root_control, cargo_control) = rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    let raw = cargo_control.as_raw_fd();
    let mut command = helper("cargo", case, raw);
    // SAFETY: only the fork child's descriptor table and credentials change.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(raw, libc::F_SETFD, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if root_campaign
                && (libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setgid(1000) != 0
                    || libc::setuid(1000) != 0)
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut cargo = ChildGuard(command.spawn().unwrap());
    drop(cargo_control);
    let (bytes, mut rights) = receive_test(&root_control);
    assert_eq!(rights.len(), 1);
    let peer = rights.pop().unwrap();
    let mut binding = Binding::decode(&bytes).unwrap();
    let (hello, rights) = receive_test(&peer);
    assert!(rights.is_empty());
    let hello = Message::decode(&hello).unwrap();
    assert_eq!(hello.kind(), Kind::Hello);
    assert!(hello.inputs().unwrap().matches_binding(&binding));
    if case == "timeout" {
        cargo.wait();
        return;
    }
    if case == "eof" {
        drop(peer);
        cargo.wait();
        return;
    }
    if case == "wrong_inputs" {
        binding = Binding::new(
            binding.compiler_handoff().clone(),
            binding.occurrence().clone(),
            binding.descriptors(),
            binding.expectation(),
            WorkerV3ApplicationHandoffChallengeV1::from_bytes([99; 32]).unwrap(),
        )
        .unwrap();
    }
    let nonce = if case == "wrong_nonce" {
        [70; 32]
    } else {
        hello.app_nonce()
    };
    let challenge = Message::challenge(binding.clone(), nonce, [9; 32]).unwrap();
    if case.starts_with("root_death_") || case == "separate_root_positive" {
        let app = pidfd(binding.compiler_handoff().launch_manifest().client().pid());
        let raw = peer.as_raw_fd();
        let mut command = helper("root_once", case, raw);
        command.env(
            PACKET,
            challenge
                .canonical_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        );
        // SAFETY: expose only this fixture endpoint in the fork child's descriptor table.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(raw, libc::F_SETFD, 0) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut root = ChildGuard(command.spawn().unwrap());
        root.wait();
        // The authentic sender is now positively reaped, while its valid packet remains queued.
        if case.starts_with("root_death_") {
            signal(&app, libc::SIGCONT);
        }
        cargo.wait();
        return;
    }
    let root_pidfd = if case == "wrong_pidfd" {
        pidfd(cargo.0.id())
    } else {
        pidfd(std::process::id())
    };
    let ordinary: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    let mut challenge_bytes = challenge.canonical_bytes().to_vec();
    if case == "extra_bytes" {
        challenge_bytes.push(0);
    }
    if case == "short" {
        challenge_bytes.truncate(100);
    }
    if case == "phase" {
        challenge_bytes = Message::ready(challenge.transcript().unwrap())
            .canonical_bytes()
            .to_vec();
    }
    let descriptors: Vec<_> = match case {
        "missing_pidfd" | "phase" => vec![],
        "extra_pidfd" => vec![root_pidfd.as_fd(), root_pidfd.as_fd(), root_pidfd.as_fd()],
        "ordinary_fd" => vec![ordinary.as_fd()],
        _ => vec![root_pidfd.as_fd()],
    };
    transmit(&peer, &challenge_bytes, &descriptors);
    if matches!(
        case,
        "positive"
            | "wrong_ready"
            | "ready_rights"
            | "ready_eof"
            | "duplicate_challenge"
            | "foreign_ready"
            | "live_root_closed_session"
            | "proof_ready"
    ) {
        let (accept, rights) = receive_test(&peer);
        assert!(rights.is_empty());
        let accept = Message::decode(&accept).unwrap();
        assert_eq!(accept.kind(), Kind::Accept);
        assert_eq!(accept.transcript(), challenge.transcript());
        if case == "ready_eof" {
            drop(peer);
            cargo.wait();
            return;
        }
        let transcript = if case == "wrong_ready" {
            Transcript::new(hello.app_nonce(), [10; 32], *binding.identity().as_bytes()).unwrap()
        } else {
            challenge.transcript().unwrap()
        };
        let ready = if case == "duplicate_challenge" {
            challenge
        } else if case == "proof_ready" {
            Message::custodian_ready(
                fe2o3_runtime_protocol::WorkerV3ApplicationProofSessionV1::new(
                    transcript,
                    [20; 32],
                    [21; 32],
                    (std::process::id(), 1001, 1001),
                )
                .unwrap(),
            )
        } else {
            Message::ready(transcript)
        };
        if case == "foreign_ready" {
            let raw = peer.as_raw_fd();
            let mut command = helper("foreign_ready", case, raw);
            command.env(
                PACKET,
                ready
                    .canonical_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            );
            // SAFETY: exposes only this fixture endpoint in the fork child's descriptor table.
            unsafe {
                command.pre_exec(move || {
                    if libc::fcntl(raw, libc::F_SETFD, 0) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            ChildGuard(command.spawn().unwrap()).wait();
            cargo.wait();
            return;
        }
        let rights = if matches!(case, "ready_rights" | "duplicate_challenge" | "proof_ready") {
            vec![root_pidfd.as_fd()]
        } else {
            vec![]
        };
        if case == "live_root_closed_session" {
            transmit(&peer, ready.canonical_bytes(), &rights);
            let (marker, rights) = receive_test(&peer);
            assert_eq!(marker, b"fixture-owner-retained");
            assert!(rights.is_empty());
            drop(peer);
            // This actual Challenge/Ready sender remains alive throughout the rejection.
            cargo.wait();
            return;
        }
        transmit(&peer, ready.canonical_bytes(), &rights);
    }
    cargo.wait();
    let mut byte = [0];
    assert_eq!(
        rustix::net::recv(&peer, &mut byte, RecvFlags::DONTWAIT)
            .unwrap()
            .0,
        0,
        "consuming failure or completed helper must close the original endpoint"
    );
}

#[test]
fn unprivileged_creator_cannot_impersonate_root_registration() {
    if rustix::process::geteuid().as_raw() == 0 {
        return;
    }
    run_case("unprivileged_sender", false);
}

#[test]
#[ignore = "requires root in an isolated namespace for actual root/UID1000 channel credentials"]
fn root_registration_transport_campaign() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    for case in [
        "positive",
        "live_root_closed_session",
        "wrong_pidfd",
        "ordinary_fd",
        "missing_pidfd",
        "extra_pidfd",
        "wrong_inputs",
        "wrong_nonce",
        "extra_bytes",
        "short",
        "phase",
        "wrong_ready",
        "ready_rights",
        "ready_eof",
        "duplicate_challenge",
        "eof",
        "timeout",
        "root_death_challenge",
        "root_death_ready",
        "separate_root_positive",
        "foreign_ready",
        "proof_ready",
        "wrong_coordinate",
        "wrong_object",
    ] {
        eprintln!("ROOT_APPLICATION_REGISTRATION_CASE={case}");
        run_case(case, true);
    }
}
