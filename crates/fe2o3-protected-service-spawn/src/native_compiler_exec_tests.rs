//! Explicit isolated-root diagnostic through the production clone/exec mechanics.
#[path = "native_compiler_restrictions_exec_tests.rs"]
mod restrictions;

use super::*;
use crate::{
    PROTECTED_SERVICE_GATE_RELEASE_V1 as GO, PROTECTED_SERVICE_PROFILE_READY_V1 as READY,
    ProtectedServiceCleanupServiceV2 as Cleanup,
    compiler_service_channel::{COMPILER_SERVICE_FD, TRANSFER_BYTES, transfer_matches},
    launch_io::{self, Boundary, MessageSender, Observer},
    native_spawn::{RootOwnedProtectedServiceChildV2 as Child, RootTaskTraceV2 as Trace},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials, observations,
};
use std::{
    io::{Read, Seek},
    os::{
        fd::{AsFd, OwnedFd},
        unix::fs::PermissionsExt,
    },
    time::{Duration, Instant},
};

const LIMIT: usize = 1 << 32;
const CLEANUP_TURNS: usize = 1024;

struct Drain(Cleanup);
impl Drop for Drain {
    fn drop(&mut self) {
        let pool = &mut self.0;
        for _ in 0..CLEANUP_TURNS {
            if pool.shutdown().is_ok() {
                return;
            }
            if let Err(error) = pool.pump(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2) {
                assert!(
                    std::thread::panicking(),
                    "compiler diagnostic cleanup refused: {error}"
                );
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            std::thread::panicking(),
            "compiler diagnostic cleanup did not drain"
        );
    }
}

fn pipe() -> (OwnedFd, OwnedFd) {
    rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap()
}

fn next(
    trace: &mut Trace<'_>,
    b: &mut Budget<'_>,
    deadline: Instant,
) -> crate::native_spawn::RootTaskTraceEventV2 {
    loop {
        let event = trace.poll(b).unwrap();
        if !event.is_pending() {
            return event;
        }
        assert!(Instant::now() < deadline, "compiler trace event deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
#[ignore = "requires isolated root, CAP_SYS_PTRACE, clone3 and static native_compiler_exec.c fixture"]
fn exact_compiler_inputs_reach_native_exec_and_owned_terminal_wait() {
    assert!(syscall::has_exact_root_identity());
    let image =
        File::open(std::env::var_os("FE2O3_NATIVE_COMPILER_EXEC_FIXTURE").expect("static fixture"))
            .unwrap();
    let cleanup_work = LIMIT
        + CLEANUP_TURNS
            * (Cleanup::pump_work(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2).unwrap()
                + Cleanup::shutdown_work());
    let mut pool = Drain(Cleanup::admit(Account::new(Work::new(cleanup_work), LIMIT)).unwrap());
    for searchable in [true, false] {
        run(&image, searchable, Channel::None, &mut pool.0);
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Channel {
    None,
    Enabled,
    Backpressure,
    WrongGate,
}

#[test]
#[ignore = "requires isolated root, CAP_SYS_PTRACE, clone3, pidfd_getfd and static native_compiler_exec.c fixture"]
fn child_created_channel_survives_gate_and_exec_and_refuses_failed_send_or_gate() {
    assert!(syscall::has_exact_root_identity());
    let image =
        File::open(std::env::var_os("FE2O3_NATIVE_COMPILER_EXEC_FIXTURE").expect("static fixture"))
            .unwrap();
    let cleanup_work = LIMIT
        + CLEANUP_TURNS
            * (Cleanup::pump_work(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2).unwrap()
                + Cleanup::shutdown_work());
    let mut pool = Drain(Cleanup::admit(Account::new(Work::new(cleanup_work), LIMIT)).unwrap());
    for mode in [Channel::Enabled, Channel::Backpressure, Channel::WrongGate] {
        run(&image, true, mode, &mut pool.0);
    }
}

struct Watch<'a, 'b> {
    child: &'a Child,
    budget: &'a mut Budget<'b>,
}
impl Observer for Watch<'_, '_> {
    type Error = Error;
    fn before_attempt(&mut self, boundary: Boundary) -> Result<()> {
        self.budget.charge_work(boundary.work()).map_err(Into::into)
    }
    fn is_live(&mut self) -> Result<bool> {
        self.child.is_live(self.budget)
    }
}

fn run(image: &File, searchable: bool, channel: Channel, pool: &mut Cleanup) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(
        directory.path(),
        std::fs::Permissions::from_mode(if searchable { 0o755 } else { 0o744 }),
    )
    .unwrap();
    std::fs::write(directory.path().join("cwd-marker"), b"cwd-data").unwrap();
    std::fs::set_permissions(
        directory.path().join("cwd-marker"),
        std::fs::Permissions::from_mode(0o444),
    )
    .unwrap();
    let cwd = File::open(directory.path()).unwrap();
    let mut output = tempfile::tempfile().unwrap();
    let (input, input_writer) = pipe();
    rustix::io::write(&input_writer, b"stdin-data").unwrap();
    drop(input_writer);
    let (binding, binding_writer) = pipe();
    rustix::io::write(&binding_writer, b"binding-data").unwrap();
    drop(binding_writer);
    let (ready, ready_writer) = pipe();
    let (gate, gate_writer) = pipe();
    let (status, status_writer) = rustix::net::socketpair(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        rustix::net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let control = (channel != Channel::None).then(|| {
        let pair = rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            rustix::net::SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        rustix::net::sockopt::set_socket_passcred(&pair.0, true).unwrap();
        pair
    });
    let arguments: Vec<_> = [
        if channel == Channel::None {
            b"captured-rustc".as_slice()
        } else {
            b"captured-rustc-channel".as_slice()
        },
        b"repeat",
        b"",
        b"repeat",
        b"two words",
        b"\xff",
    ]
    .into_iter()
    .map(|v| CString::new(v).unwrap())
    .collect();
    let environment = [CString::new("A=").unwrap(), CString::new("B=x=y").unwrap()];
    let bindings = [Binding::new(binding.as_fd(), 198).unwrap()];
    let source = usize::try_from(image.metadata().unwrap().len()).unwrap() + 16384;
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(source).unwrap();
    // An isolated diagnostic owns all sources and channels; this is not image,
    // deployment, compiler or proof admission. The actual staging/clone is shared.
    let (stage, charge) = unsafe {
        if let Some((_, transfer)) = &control {
            Stage::stage_compiler_with_child_channel(
                image,
                &arguments,
                &environment,
                cwd.as_fd(),
                [Some(input.as_fd()), Some(output.as_fd()), None],
                &bindings,
                ready_writer.as_fd(),
                gate.as_fd(),
                status_writer.as_fd(),
                transfer.as_fd(),
                source,
                &mut b,
            )
        } else {
            Stage::stage_compiler(
                image,
                &arguments,
                &environment,
                cwd.as_fd(),
                [Some(input.as_fd()), Some(output.as_fd()), None],
                &bindings,
                ready_writer.as_fd(),
                gate.as_fd(),
                status_writer.as_fd(),
                source,
                &mut b,
            )
        }
    }
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(stage.compiler_arguments(), Some(arguments.as_slice()));
    assert_eq!(stage.compiler_environment(), Some(environment.as_slice()));
    drop(arguments);
    drop(environment);
    let stage = Box::new(stage);
    if channel == Channel::Backpressure {
        let transfer = &control.as_ref().unwrap().1;
        let mut full = false;
        for _ in 0..4096 {
            match rustix::net::send(
                transfer,
                &[0; TRANSFER_BYTES],
                rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
            ) {
                Ok(TRANSFER_BYTES) => {}
                Err(Errno::AGAIN) => {
                    full = true;
                    break;
                }
                other => panic!("fill transfer queue: {other:?}"),
            }
        }
        assert!(full, "bounded transfer queue fill");
    }
    let credentials = Credentials::new(65534, 65534).unwrap();
    // This thread exclusively owns waits, inputs, bounded gate release and cleanup.
    let (mut child, child_charge) = unsafe { stage.spawn(credentials, pool, &mut b) }.unwrap();
    b.reserve_storage(child_charge.additional_storage() + Child::ROOT_TRACE_GROWTH)
        .unwrap();
    drop(stage);
    b.release_storage(charge.additional_storage()).unwrap();
    drop(ready_writer);
    drop(status_writer);
    drop(gate);
    let receiver = control.map(|(receiver, transfer)| {
        drop(transfer);
        receiver
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    rustix::fs::fcntl_setfl(&ready, rustix::fs::OFlags::NONBLOCK).unwrap();
    let mut byte = [0];
    if channel == Channel::Backpressure {
        while child.is_live(&mut b).unwrap() {
            assert!(
                Instant::now() < deadline,
                "nonblocking transfer failure deadline"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(rustix::io::read(&ready, &mut byte).unwrap(), 0);
        assert_eq!(rustix::io::read(&status, &mut byte).unwrap(), 1);
        assert_eq!(byte, [0xcb]);
        assert_eq!(child.cancel(), crate::cleanup_bridge::CleanupPollV1::Reaped);
        assert_eq!(output.stream_position().unwrap(), 0);
        return;
    }
    loop {
        match rustix::io::read(&ready, &mut byte) {
            Ok(1) => break,
            Err(Errno::AGAIN | Errno::INTR) => {
                assert!(
                    Instant::now() < deadline,
                    "compiler profile readiness deadline"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            other => panic!("compiler profile readiness: {other:?}"),
        }
    }
    assert_eq!(byte, [READY]);
    // The untraced-service profile deliberately rejects any tracer. Check it
    // before our exclusive SEIZE while the same child is still behind the gate.
    observations::validate_process(credentials, child.pid()).unwrap();
    let service = receiver.as_ref().map(|receiver| {
        b.reserve_storage(launch_io::ATTEMPT_SCRATCH).unwrap();
        let sender = MessageSender::new(
            child.pid().as_raw_pid(),
            credentials.uid(),
            credentials.gid(),
        );
        let (wire, peer) = launch_io::receive_ready_from::<TRANSFER_BYTES, true, _>(
            receiver.as_fd(),
            sender,
            &mut Watch {
                child: &child,
                budget: &mut b,
            },
            deadline,
        )
        .unwrap();
        b.release_storage(launch_io::ATTEMPT_SCRATCH).unwrap();
        let peer = peer.unwrap();
        assert!(transfer_matches(
            &wire,
            child.pid().as_raw_pid() as u32,
            std::process::id()
        ));
        let actual = rustix::net::sockopt::socket_peercred(&peer).unwrap();
        assert_eq!(actual.pid, child.pid());
        assert_eq!(actual.uid.as_raw(), credentials.uid());
        assert_eq!(actual.gid.as_raw(), credentials.gid());
        assert!(
            rustix::io::fcntl_getfd(&peer)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        inspect_gated_client(&child, &mut b);
        if channel == Channel::Enabled {
            assert_eq!(
                rustix::net::send(
                    &peer,
                    b"channel-request",
                    rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL
                )
                .unwrap(),
                15
            );
        }
        peer
    });
    let mut trace = child.into_root_trace(&mut b).unwrap();
    let release = if channel == Channel::WrongGate {
        GO ^ 1
    } else {
        GO
    };
    assert_eq!(rustix::io::write(&gate_writer, &[release]).unwrap(), 1);
    let first = next(&mut trace, &mut b, deadline);
    rustix::fs::fcntl_setfl(&status, rustix::fs::OFlags::NONBLOCK).unwrap();
    let executed = searchable && channel != Channel::WrongGate;
    let terminal = if executed {
        assert!(first.is_exec());
        assert_eq!(rustix::io::read(&status, &mut byte).unwrap(), 0);
        trace.resume(&mut b).unwrap();
        let terminal = next(&mut trace, &mut b, deadline);
        assert_eq!(terminal.exit_code(), Some(7));
        terminal
    } else {
        assert_eq!(first.exit_code(), Some(126));
        assert_eq!(rustix::io::read(&status, &mut byte).unwrap(), 1);
        assert_eq!(
            byte,
            [if channel == Channel::WrongGate {
                0xc6
            } else {
                0xca
            }]
        );
        first
    };
    assert!(terminal.is_terminal());
    assert_eq!(trace.poll(&mut b).unwrap(), terminal);
    if let Some(peer) = service {
        let mut response = [0; 32];
        if executed {
            assert_eq!(
                rustix::net::recv(&peer, &mut response, rustix::net::RecvFlags::DONTWAIT)
                    .unwrap()
                    .0,
                16
            );
            assert_eq!(&response[..16], b"channel-response");
        }
        // No child endpoint or high-FD alias survives terminal disposal.
        assert_eq!(
            rustix::net::recv(&peer, &mut response, rustix::net::RecvFlags::DONTWAIT)
                .unwrap()
                .0,
            0
        );
    }
    let expected = if executed {
        b"native compiler exec complete\n".as_slice()
    } else {
        b""
    };
    // The output shares the original open-file description, including its offset.
    assert_eq!(output.stream_position().unwrap(), expected.len() as u64);
    output.rewind().unwrap();
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(trace.cancel(), crate::cleanup_bridge::CleanupPollV1::Reaped);
    drop(trace);
}

fn inspect_gated_client(child: &Child, b: &mut Budget<'_>) {
    let (pidfd, charge) = child.try_clone_pidfd(b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert!(matches!(
        rustix::process::pidfd_getfd(
            &pidfd,
            COMPILER_SERVICE_FD,
            rustix::process::PidfdGetfdFlags::empty()
        ),
        Err(Errno::BADF)
    ));
    let directory = format!("/proc/{}/fd", child.pid().as_raw_pid());
    let mut clients = 0;
    let mut entries = std::fs::read_dir(directory).unwrap();
    for _ in 0..1024 {
        let Some(entry) = entries.next() else {
            break;
        };
        let fd: i32 = entry
            .unwrap()
            .file_name()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        if fd < FLOOR {
            continue;
        }
        let copy =
            rustix::process::pidfd_getfd(&pidfd, fd, rustix::process::PidfdGetfdFlags::empty())
                .unwrap();
        if let Ok(peer) = rustix::net::sockopt::socket_peercred(&copy) {
            if peer.pid == child.pid() {
                clients += 1;
                let info = std::fs::read_to_string(format!(
                    "/proc/{}/fdinfo/{fd}",
                    child.pid().as_raw_pid()
                ))
                .unwrap();
                let flags = info
                    .lines()
                    .find_map(|line| line.strip_prefix("flags:\t"))
                    .unwrap();
                let flags = u32::from_str_radix(flags, 8).unwrap();
                assert_ne!(flags & libc::O_CLOEXEC as u32, 0);
            }
        }
    }
    assert!(entries.next().is_none(), "bounded diagnostic FD inventory");
    assert_eq!(
        clients, 1,
        "exactly one child-created high-FD client behind gate"
    );
    drop(pidfd);
    b.release_storage(charge.additional_storage()).unwrap();
}
