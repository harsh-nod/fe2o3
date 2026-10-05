//! Real descriptors and inert comparisons only; no runtime approval is invented.
use super::*;
use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
use fe2o3_protected_service_spawn::compiler_service_channel::encode_transfer as encode;

const LIMIT: usize = 1 << 34;

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
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::time::Duration;
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
    for case in 0..10 {
        run_native(case, &mut pool.0);
    }
}

#[allow(unsafe_code)]
fn run_native(case: usize, pool: &mut Cleanup) {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use fe2o3_protected_service_spawn::{
        ProtectedServiceDescriptorBindingV1 as Binding,
        cleanup_bridge::CleanupPollV1 as CleanupPoll,
        native_spawn::StagedProtectedServiceExecV2 as Stage,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::{ffi::CString, fs::File, os::unix::fs::PermissionsExt, time::Duration};
    struct DropWitness(Arc<AtomicUsize>);
    impl Drop for DropWitness {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
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
            (image, cwd, unused, DropWitness(Arc::clone(&drops))),
            source,
            pool,
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
    let (exit_observer, charge) = child.try_clone_pidfd(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
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
    if (1..=5).contains(&case) {
        let received = CompilerChildChannel::receive(
            &child,
            root,
            receive_credentials,
            receive_deadline,
            &mut b,
        );
        assert_eq!(b.storage(), before_storage);
        assert!(b.work() > before_work);
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
        drain_child(pool, exit_observer.as_fd());
        return;
    }
    let child_pid = child.pid().as_raw_pid() as u32;
    let consumed = child.retained_storage() + native::FILE_STORAGE;
    let (mut trace, full) =
        CompilerTrace::receive(child, root, credentials, deadline, &mut b).unwrap();
    assert_eq!(b.storage(), before_storage);
    b.release_storage(consumed).unwrap();
    b.reserve_storage(full).unwrap();
    assert_eq!(full, trace.retained_storage());
    assert_eq!(trace.pid().as_raw_pid() as u32, child_pid);
    trace
        .with_backing(&mut b, |(_, cwd, _, _), same| -> Result<()> {
            assert!(same.storage() >= full);
            assert!(cwd.metadata().unwrap().is_dir());
            Ok(())
        })
        .unwrap();
    assert!(
        trace
            .with_issuer_inputs::<()>(&mut b, |_, _, _, _, _| panic!("gated child exposed inputs"))
            .is_err()
    );
    assert!(trace.resume(&mut b).is_err());
    assert!(!trace.needs_foreground_cancellation());
    assert!(
        trace
            .with_runtime_backing(&mut b, |_, _, _| -> Result<()> {
                panic!("unarmed trace exposed runtime backing")
            })
            .is_err()
    );
    if case == 9 {
        trace.interrupt_for_runtime(&mut b).unwrap();
        loop {
            let event = trace.poll(&mut b).unwrap();
            if event.is_original_interrupt() {
                break;
            }
            assert!(event.is_pending());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let quota = CompilerTrace::<()>::runtime_takeover_quota().unwrap();
        let initial_work = b.work();
        let initial_peak = b.peak_storage();
        let floor = b.storage();
        // SAFETY: this explicitly isolated diagnostic owns the original gated
        // child, original account and waits; no gate is released. The outer
        // isolated test custodian retains its pidfd/deadline and cleanup pool.
        let (mut trace, runtime_full) = unsafe { trace.arm_runtime(&mut b) }.unwrap();
        assert!(b.work() - initial_work <= quota.work);
        assert!(b.peak_storage() <= initial_peak.max(floor + quota.scratch));
        assert_eq!(b.storage(), floor);
        b.release_storage(full).unwrap();
        b.reserve_storage(runtime_full).unwrap();
        assert!(runtime_full > full);
        assert!(trace.needs_foreground_cancellation());
        assert_eq!(trace.pid().as_raw_pid() as u32, child_pid);
        let original_account = b.work_ledger_identity_v1();
        let original_address = &b as *const _ as usize;
        let work = b.work();
        let peak = b.peak_storage();
        let floor = b.storage();
        trace
            .with_runtime_backing(&mut b, |runtime, (_, cwd, _, _), same| -> Result<()> {
                assert!(same.work_ledger_identity_v1() == original_account);
                assert_eq!(same as *const _ as usize, original_address);
                assert_eq!(runtime.pid().as_raw_pid() as u32, child_pid);
                assert!(cwd.metadata().unwrap().is_dir());
                assert!(!runtime.is_trace_retired());
                Ok(())
            })
            .unwrap();
        let quota = CompilerTrace::<()>::runtime_backing_quota().unwrap();
        assert!(b.work() - work <= quota.work());
        assert!(b.peak_storage() <= peak.max(floor + quota.scratch()));
        assert_eq!(b.storage(), floor);
        trace
            .with_backing(&mut b, |(_, cwd, _, _), _| -> Result<()> {
                assert!(cwd.metadata().unwrap().is_dir());
                Ok(())
            })
            .unwrap();
        assert!(trace.resume(&mut b).is_err());
        assert!(matches!(
            trace.cancel(),
            fe2o3_protected_service_spawn::cleanup_bridge::CleanupPollV1::Pending
        ));
        let cancellation = CompilerTrace::<()>::cancellation_quota().unwrap();
        while trace.needs_foreground_cancellation() {
            let work = b.work();
            let peak = b.peak_storage();
            let floor = b.storage();
            let _domain_disposition = trace.cancel_step(&mut b).unwrap();
            assert!(b.work() - work <= cancellation.work());
            assert!(b.peak_storage() <= peak.max(floor + cancellation.scratch()));
            assert_eq!(b.storage(), floor);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            trace
                .with_runtime(&mut b, |_, _| -> Result<()> {
                    panic!("cancelled compiler exposed runtime execution")
                })
                .is_err()
        );
        assert!(
            trace
                .with_runtime_backing(&mut b, |_, _, _| -> Result<()> {
                    panic!("cancelled trace exposed runtime backing")
                })
                .is_err()
        );
        drop(trace);
        b.release_storage(runtime_full).unwrap();
        drop(stage);
        drop((
            sender,
            exec_reader,
            exec_writer,
            profile_reader,
            profile_writer,
            gate_reader,
            gate_writer,
        ));
        drain_child(pool, exit_observer.as_fd());
        return;
    }
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
    assert!(
        trace
            .with_issuer_inputs::<()>(&mut b, |_, _, _, _, _| panic!(
                "unconfirmed exec exposed inputs"
            ))
            .is_err()
    );
    // SAFETY: the owned raw child has run only the audited non-forking native
    // stage. Exact held exec and CLOEXEC status EOF were observed; all parent
    // writer aliases are closed. No fixture instruction has executed yet.
    unsafe {
        trace.confirm_exec(&mut b).unwrap();
        assert!(trace.confirm_exec(&mut b).is_err());
    }
    let mut other_work = Work::new(LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    other.reserve_storage(full).unwrap();
    assert!(
        trace
            .with_backing::<(), Error>(&mut other, |_, _| panic!("foreign account exposed backing"))
            .is_err()
    );
    assert!(
        trace
            .with_issuer_inputs::<()>(&mut other, |_, _, _, _, _| panic!(
                "foreign account exposed inputs"
            ))
            .is_err()
    );
    let transfer_floor = b.storage();
    let transfer_work = b.work();
    let transfer_peak = b.peak_storage();
    let quota = trace.issuer_inputs_quota().unwrap();
    let dependency_storage = trace.issuer_dependency_storage().unwrap();
    assert_eq!((b.storage(), b.work()), (transfer_floor, transfer_work));
    let transfer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        trace.with_issuer_inputs(&mut b, |client, peer, pidfd, dependency, same| {
            assert_eq!(client.pid(), child_pid);
            assert_eq!((client.uid(), client.gid()), (65534, 65534));
            assert!(same.storage() >= full + dependency.retained_storage());
            assert_eq!(dependency.retained_storage(), dependency_storage);
            require_idle(peer)?;
            require_idle(pidfd)?;
            if case == 6 {
                return Err(Error::Invalid("diagnostic transfer refusal"));
            }
            if case == 7 {
                panic!("diagnostic transfer unwind");
            }
            if case == 8 {
                let charge = dependency.retained_storage();
                drop(dependency);
                // Callback success cannot commit a transfer whose enclosing
                // scope subsequently rejects an under-retired reservation.
                same.release_storage(charge + 1)?;
                return Ok(None);
            }
            Ok(Some(dependency))
        })
    }));
    let dependency = if case == 7 {
        assert!(transfer.is_err());
        None
    } else {
        let transferred = transfer.unwrap();
        assert_eq!(transferred.is_ok(), case == 0);
        if case == 8 {
            assert!(matches!(
                transferred,
                Err(Error::Resource(Resource::Accounting))
            ));
        }
        transferred.ok().flatten()
    };
    assert_eq!(b.storage(), transfer_floor);
    assert_eq!(b.work(), transfer_work + quota.work());
    assert!(b.peak_storage() <= transfer_peak.max(transfer_floor + quota.scratch()));
    if let Some(dependency) = &dependency {
        b.reserve_storage(dependency.retained_storage()).unwrap();
    }
    assert!(
        trace
            .with_issuer_inputs::<()>(&mut b, |_, _, _, _, _| panic!("issuer inputs replayed"))
            .is_err()
    );
    if case == 0 {
        trace.resume(&mut b).unwrap();
        assert!(
            trace
                .with_issuer_inputs::<()>(&mut b, |_, _, _, _, _| panic!(
                    "resumed compiler exposed inputs"
                ))
                .is_err()
        );
    } else {
        assert!(trace.resume(&mut b).is_err());
    }
    if case == 8 {
        // Rejection itself must cancel, before an explicit caller cancellation.
        drain_child(pool, exit_observer.as_fd());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
    assert_ne!(trace.cancel(), CleanupPoll::Quarantined);
    drain_child(pool, exit_observer.as_fd());
    assert!(require_idle(exit_observer.as_fd()).is_err());
    drop(trace);
    // The actual compiler is reaped and foreground trace is gone. Independent
    // issuer payload retention still owns the same transitive resources.
    assert_eq!(
        drops.load(Ordering::SeqCst),
        usize::from(dependency.is_none())
    );
    drop(dependency);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

fn drain_child(pool: &mut Cleanup, pidfd: BorrowedFd<'_>) {
    for _ in 0..1024 {
        pool.pump(fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2)
            .unwrap();
        let mut fds = [event::PollFd::new(&pidfd, event::PollFlags::IN)];
        if event::poll(
            &mut fds,
            Some(&event::Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        )
        .unwrap()
            == 1
        {
            assert!(fds[0].revents().contains(event::PollFlags::IN));
            // A full final turn consumes the exited root record before the next
            // attempt. Empty shutdown of the SAME pool is checked after all cases.
            pool.pump(fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2)
                .unwrap();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("compiler child did not terminate");
}
