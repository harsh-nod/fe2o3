//! Real descriptors and inert comparisons only; no runtime approval is invented.
use super::*;
use fe2o3_protected_service_spawn::compiler_service_channel::encode_transfer as encode;

fn encode_transfer(child: u32, parent: u32) -> [u8; TRANSFER_BYTES] {
    encode(child, parent).unwrap()
}

fn pair(kind: net::SocketType) -> (OwnedFd, OwnedFd) {
    net::socketpair(
        net::AddressFamily::UNIX,
        kind,
        net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap()
}

fn current() -> Identity {
    Identity::new(
        std::process::id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap()
}

fn parent() -> u32 {
    rustix::process::getppid().unwrap().as_raw_pid() as u32
}

#[test]
fn actual_idle_seqpacket_matches_only_its_creator() {
    let (a, _b) = pair(net::SocketType::SEQPACKET);
    let client = current();
    let bytes = encode_transfer(client.pid(), parent());
    validate_transfer(&bytes, a.as_fd(), client, parent()).unwrap();
    for changed in [
        Identity::new(client.pid() + 1, client.uid(), client.gid()).unwrap(),
        Identity::new(client.pid(), client.uid() ^ 1, client.gid()).unwrap(),
        Identity::new(client.pid(), client.uid(), client.gid() ^ 1).unwrap(),
    ] {
        let bytes = encode_transfer(changed.pid(), parent());
        assert!(validate_transfer(&bytes, a.as_fd(), changed, parent()).is_err());
    }
}

#[test]
fn altered_wire_and_parent_refuse() {
    let (a, _b) = pair(net::SocketType::SEQPACKET);
    let client = current();
    let bytes = encode_transfer(client.pid(), parent());
    for index in 0..TRANSFER_BYTES {
        let mut changed = bytes;
        changed[index] ^= 1;
        assert!(
            validate_transfer(&changed, a.as_fd(), client, parent()).is_err(),
            "byte {index}"
        );
    }
    assert!(validate_transfer(&bytes, a.as_fd(), client, parent() + 1).is_err());
}

#[test]
fn streams_unconnected_sockets_missing_cloexec_and_files_refuse() {
    let client = current();
    let bytes = encode_transfer(client.pid(), parent());
    let (stream, _other) = pair(net::SocketType::STREAM);
    let file = tempfile::tempfile().unwrap();
    let unconnected = net::socket_with(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let (no_cloexec, _other) = pair(net::SocketType::SEQPACKET);
    rustix::io::fcntl_setfd(&no_cloexec, FdFlags::empty()).unwrap();
    for fd in [
        stream.as_fd(),
        file.as_fd(),
        unconnected.as_fd(),
        no_cloexec.as_fd(),
    ] {
        assert!(validate_transfer(&bytes, fd, client, parent()).is_err());
    }
}

#[test]
fn prematurely_readable_or_closed_peer_refuses() {
    let client = current();
    let bytes = encode_transfer(client.pid(), parent());
    let (a, b) = pair(net::SocketType::SEQPACKET);
    net::send(&b, b"unexpected", net::SendFlags::NOSIGNAL).unwrap();
    assert!(validate_transfer(&bytes, a.as_fd(), client, parent()).is_err());
    drop(b);
    assert!(validate_transfer(&bytes, a.as_fd(), client, parent()).is_err());
    let (a, b) = pair(net::SocketType::SEQPACKET);
    drop(b);
    assert!(validate_transfer(&bytes, a.as_fd(), client, parent()).is_err());
}

#[test]
#[ignore = "requires isolated root, clone3, CAP_SETUID/SETGID/SETPCAP/SYS_PTRACE/KILL and static exec fixture"]
fn native_child_channel_joins_original_pidfd_before_trace() {
    for case in 0..6 {
        run_native(case);
    }
}

#[allow(unsafe_code)]
fn run_native(case: usize) {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_protected_service_spawn::{
        ProtectedServiceCleanupServiceV2 as Cleanup,
        ProtectedServiceDescriptorBindingV1 as Binding,
        cleanup_bridge::CleanupPollV1 as CleanupPoll,
        native_spawn::StagedProtectedServiceExecV2 as Stage,
    };
    use std::{ffi::CString, fs::File, os::unix::fs::PermissionsExt, time::Duration};

    const LIMIT: usize = 1 << 34;
    struct Drain(Cleanup);
    impl Drop for Drain {
        fn drop(&mut self) {
            for _ in 0..1024 {
                if self.0.shutdown().is_ok() {
                    return;
                }
                self.0
                    .pump(fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2)
                    .unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(
                std::thread::panicking(),
                "native compiler channel cleanup did not drain"
            );
        }
    }
    let mut pool = Drain(Cleanup::admit(Account::new(Work::new(LIMIT), LIMIT)).unwrap());
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    // The isolated container can mount /tmp noexec. Use the explicit executable
    // fixture mount, and keep only the cwd in writable scratch.
    let image = File::open(
        std::env::var_os("FE2O3_NATIVE_COMPILER_EXEC_FIXTURE").expect("static exec fixture"),
    )
    .unwrap();
    let cwd = File::open(dir.path()).unwrap();
    let unused = File::open("/dev/null").unwrap();
    let channels = native::Channels::new().unwrap();
    let argv = [CString::new("native-channel-fixture").unwrap()];
    let bindings = [Binding::new(unused.as_fd(), 198).unwrap()];
    let source = usize::try_from(image.metadata().unwrap().len()).unwrap() + 32768;
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(source).unwrap();
    // SAFETY: isolated diagnostic owns real inputs and exclusive creator/wait
    // custody. This exercises mechanics without constructing a runtime approval.
    let (stage, charge) = unsafe {
        Stage::stage_compiler_with_child_channel(
            &image,
            &argv,
            &[],
            cwd.as_fd(),
            [None; 3],
            &bindings,
            channels.profile_writer.as_fd(),
            channels.gate_reader.as_fd(),
            channels.exec_writer.as_fd(),
            channels.child.as_fd(),
            source,
            &mut b,
        )
    }
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let credentials = Credentials::new(65534, 65534).unwrap();
    // SAFETY: all source backing survives in the original cleanup slot; no
    // process is admitted as a compiler. The test retains this creator thread.
    let (child, growth) = unsafe {
        stage.spawn_retaining(
            credentials,
            (image, cwd, unused),
            source,
            &mut pool.0,
            &mut b,
        )
    }
    .unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    launch_io::await_profile_ready(
        channels.profile_reader.as_fd(),
        channels.exec_reader.as_fd(),
        &mut Observer {
            child: &child,
            budget: &mut b,
        },
        deadline,
    )
    .unwrap();
    fe2o3_protected_service_profile::observations::validate_process(credentials, child.pid())
        .unwrap();
    let native::Channels {
        root,
        child: sender,
        exec_reader,
        exec_writer,
        profile_reader,
        profile_writer,
        gate_reader,
        gate_writer,
    } = channels;
    let receive_deadline = if case == 1 { Instant::now() } else { deadline };
    if case == 2 {
        net::sockopt::set_socket_passcred(&root, false).unwrap();
    }
    let receive_credentials = if case == 3 {
        Credentials::new(65533, 65533).unwrap()
    } else {
        credentials
    };
    if case == 4 {
        // Exactly one byte short of this receiver's outer frame.
        b.reserve_storage(LIMIT - b.storage() - FRAME + 1).unwrap();
    }
    if case == 5 {
        // Exactly one work unit short of this receiver's prepaid local work.
        b.charge_work(LIMIT - b.work() - LOCAL_WORK + 1).unwrap();
    }
    let fd_count = || std::fs::read_dir("/proc/self/fd").unwrap().count();
    let before_fds = fd_count();
    let before_storage = b.storage();
    let before_work = b.work();
    let received =
        CompilerChildChannel::receive(&child, root, receive_credentials, receive_deadline, &mut b);
    assert_eq!(b.storage(), before_storage);
    assert!(b.work() > before_work);
    if case != 0 {
        let error = match received {
            Ok(_) => panic!("refusal case {case} succeeded"),
            Err(error) => error,
        };
        match case {
            1 => assert!(matches!(
                error,
                Error::Transport(launch_io::Failure::Timeout("compiler channel entry"))
            )),
            2 => assert!(matches!(
                error,
                Error::Invalid("compiler transfer credentials disabled")
            )),
            3 => assert!(matches!(
                error,
                Error::Transport(launch_io::Failure::MalformedReadyTransfer)
            )),
            4 => assert_eq!(b.failed_storage(), Some(LIMIT + 1)),
            5 => assert_eq!(b.failed_work(), Some(LIMIT + 1)),
            _ => unreachable!(),
        }
        // Receiver is consumed; neither the cloned pidfd nor a received right leaks.
        assert_eq!(fd_count() + 1, before_fds);
        drop(child);
        return;
    }
    let (channel, full) = received.unwrap();
    b.reserve_storage(full).unwrap();
    assert_eq!(full, channel.retained);
    assert_eq!(channel.client.pid(), child.pid().as_raw_pid() as u32);
    assert_eq!(channel.client.uid(), 65534);
    require_idle(channel.client_pidfd.as_fd()).unwrap();
    require_idle(channel.service_peer.as_fd()).unwrap();
    let trace_charge = child.root_trace_storage().unwrap() - child.retained_storage();
    b.reserve_storage(trace_charge).unwrap();
    let mut trace = child.into_root_trace(&mut b).unwrap();
    drop(stage);
    drop((
        sender,
        exec_writer,
        profile_reader,
        profile_writer,
        gate_reader,
    ));
    rustix::io::write(
        &gate_writer,
        &[fe2o3_protected_service_spawn::PROTECTED_SERVICE_GATE_RELEASE_V1],
    )
    .unwrap();
    let mut observed = false;
    for _ in 0..10000 {
        let event = trace.poll(&mut b).unwrap();
        if !event.is_pending() {
            assert!(
                event.is_exec(),
                "unexpected {event:?}; child status {:?}",
                net::recv(&exec_reader, &mut [0; 1], net::RecvFlags::DONTWAIT)
            );
            observed = true;
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(observed, "missing native exec stop");
    // Held exec stop plus CLOEXEC status EOF; this is not compiler admission.
    assert_eq!(
        net::recv(&exec_reader, &mut [0; 1], net::RecvFlags::DONTWAIT).unwrap(),
        (0, 0)
    );
    require_idle(channel.client_pidfd.as_fd()).unwrap();
    require_idle(channel.service_peer.as_fd()).unwrap();
    assert_ne!(trace.cancel(), CleanupPoll::Quarantined);
    let mut terminal = false;
    for _ in 0..1024 {
        if pool.0.shutdown().is_ok() {
            terminal = true;
            break;
        }
        pool.0
            .pump(fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2)
            .unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(terminal, "compiler child did not terminate");
    assert!(require_idle(channel.client_pidfd.as_fd()).is_err());
    drop((trace, channel));
}
